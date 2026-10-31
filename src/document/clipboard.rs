//! Annota's object clipboard (add-export-share task 2.4a; spec export-share "Object copy and
//! paste"): the selected objects as a self-contained payload of their own clipboard type,
//! [`MIME_TYPE`], so they paste into any document — another one, or one opened after the source
//! document was closed — without reaching back into the document they came from.
//!
//! **Shape.** The payload is a cut-down `.annota` (`super::archive`): a ZIP archive with
//!
//! * `objects.json` — `{"format", "requires", "generator", "objects": [...]}`: the versioning
//!   keys of `document.json` and its `objects` list, each node written exactly as a saved
//!   document writes it (`registry::node_to_json`: `"id"`, `"type"` and the object's fields,
//!   unknown data of a newer version merged back in);
//! * `images/<n>.png` — the embedded images those objects use, under the source document's
//!   ids, PNG-encoded and stored like in `.annota`.
//!
//! There is no `original.png`, canvas or metadata: objects only.
//!
//! **Forward compatibility** follows `.annota`'s rules, through the same code: a newer *major*
//! format is refused ([`ArchiveError::NewerFormat`]) before anything else is read; a newer
//! *minor* one reads with `registry::node_from_json`, so keys this version does not know (at any
//! depth) stay with each node as its residual, unknown object types and unreadable objects of
//! known types become `UnknownObject`s with the embedded images they appear to use. Pasting
//! them (`editing::ops::paste`) raises the target document's minor version as well, so saving
//! it writes all of that back and the file reopens. Unknown keys at the payload's top level
//! (next to `objects`) have no counterpart in a document and are not kept.

use std::collections::BTreeSet;
use std::io::{Cursor, Write};

use serde_json::{Map, Value};

use super::archive::{self, ArchiveError, MAX_JSON, MAX_PNG, REQUIRES};
use super::bitmap::Bitmap;
use super::registry::{node_from_json, node_to_json, UnknownObject};
use super::scene::{Node, ObjectId};
use super::Document;

/// The clipboard type of copied objects. Private to Annota (`x-`): no other app reads it.
pub const MIME_TYPE: &str = "application/x-annota-objects";
/// The payload's JSON entry.
pub const OBJECTS_JSON: &str = "objects.json";

/// Copied objects, read back from a payload: ready to paste into any document.
#[derive(Clone, Debug)]
pub struct Clip {
    /// The payload's minor format version (a newer Annota's may be higher than ours).
    pub(crate) format_minor: u32,
    /// Top-level objects, bottom to top, with the ids they had in their document.
    pub(crate) nodes: Vec<Node>,
    /// The embedded images they use, under the source document's ids.
    pub(crate) resources: Vec<(String, Bitmap)>,
}

impl Clip {
    /// Number of top-level objects.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The objects, bottom to top.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}

/// The payload for the objects `ids` of `doc` (top-level ids; others are skipped), in stacking
/// order; `None` if none of them is in the document.
pub fn encode(doc: &Document, ids: &[ObjectId]) -> Result<Option<Vec<u8>>, ArchiveError> {
    let wanted: BTreeSet<ObjectId> = ids.iter().copied().collect();
    let nodes: Vec<&Node> = doc
        .scene()
        .nodes()
        .iter()
        .filter(|n| wanted.contains(&n.id))
        .collect();
    if nodes.is_empty() {
        return Ok(None);
    }
    let objects = nodes
        .iter()
        .map(|n| node_to_json(n))
        .collect::<Result<Vec<_>, _>>()?;
    let mut root = Map::new();
    root.insert(
        "format".into(),
        Value::String(archive::written_format(doc).to_string()),
    );
    root.insert("requires".into(), Value::String(REQUIRES.to_string()));
    root.insert(
        "generator".into(),
        Value::String(format!("Annota {}", env!("CARGO_PKG_VERSION"))),
    );
    root.insert("objects".into(), Value::Array(objects));
    let json = serde_json::to_vec(&Value::Object(root))?;

    let used: BTreeSet<&str> = nodes.iter().flat_map(|n| n.object.resources()).collect();
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(OBJECTS_JSON, deflated)?;
    zip.write_all(&json)?;
    for id in used {
        let bitmap = doc
            .resources()
            .get(id)
            .ok_or_else(|| ArchiveError::Missing(id.to_string()))?;
        zip.start_file(id, stored)?;
        zip.write_all(&bitmap.png()?)?;
    }
    Ok(Some(zip.finish()?.into_inner()))
}

/// Reads a payload made by [`encode`] (by this or another Annota version).
pub fn decode(bytes: &[u8]) -> Result<Clip, ArchiveError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    let json = archive::read_entry(&mut zip, OBJECTS_JSON, MAX_JSON)?;
    let mut root: Value = serde_json::from_slice(&json)?;
    let format = archive::check_format(&root)?;
    let newer_minor = format.minor > archive::FORMAT.minor;
    let objects = match root.get_mut("objects").map(Value::take) {
        Some(Value::Array(objects)) => objects,
        _ => {
            return Err(ArchiveError::Invalid(
                "the copied objects are missing".into(),
            ))
        }
    };
    let mut nodes = objects
        .into_iter()
        .map(|v| node_from_json(v, newer_minor))
        .collect::<Result<Vec<_>, _>>()?;

    // The images the objects use must be there, as in a document.
    let ids: BTreeSet<String> = nodes
        .iter()
        .flat_map(|n| n.object.resources().into_iter().map(str::to_string))
        .collect();
    let mut resources: Vec<(String, Bitmap)> = Vec::with_capacity(ids.len());
    for id in ids {
        if super::parse_resource_id(&id).is_none() {
            return Err(ArchiveError::Invalid(format!(
                "invalid image reference '{id}'"
            )));
        }
        let png = archive::read_entry(&mut zip, &id, MAX_PNG)?;
        resources.push((id, Bitmap::from_png(png)?));
    }
    // Objects this version cannot interpret keep the images they appear to use (as when a
    // document is read); anything that is not a readable image entry is ignored.
    for node in &mut nodes {
        let Some(unknown) = node.object.as_any_mut().downcast_mut::<UnknownObject>() else {
            continue;
        };
        for id in unknown.resource_candidates() {
            if !resources.iter().any(|(r, _)| *r == id) {
                let Ok(png) = archive::read_entry(&mut zip, &id, MAX_PNG) else {
                    continue;
                };
                let Ok(bitmap) = Bitmap::from_png(png) else {
                    continue;
                };
                resources.push((id.clone(), bitmap));
            }
            unknown.resources.push(id);
        }
    }
    for node in &nodes {
        super::command::check_geometry(node.id, node.object.as_ref())
            .map_err(|e| ArchiveError::Invalid(e.to_string()))?;
    }
    Ok(Clip {
        format_minor: format.minor,
        nodes,
        resources,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::archive::tests::sample;
    use crate::document::color::palette;
    use crate::document::command::{Command, History};
    use crate::document::geometry::Point;
    use crate::tools::arrow::Arrow;
    use crate::tools::group::Group;
    use serde_json::json;
    use std::io::Read;

    /// `objects.json` of a payload.
    fn objects_json(bytes: &[u8]) -> Value {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        serde_json::from_slice(&archive::read_entry(&mut zip, OBJECTS_JSON, MAX_JSON).unwrap())
            .unwrap()
    }

    /// Rewrites `objects.json` inside a payload.
    fn with_json(bytes: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
        let mut src = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let mut edit = Some(edit);
        let opts = zip::write::SimpleFileOptions::default();
        for i in 0..src.len() {
            let mut entry = src.by_index(i).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == OBJECTS_JSON {
                let mut v: Value = serde_json::from_slice(&data).unwrap();
                if let Some(edit) = edit.take() {
                    edit(&mut v);
                }
                data = serde_json::to_vec(&v).unwrap();
            }
            out.start_file(name, opts).unwrap();
            out.write_all(&data).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    fn with_arrow(mut doc: Document) -> (Document, ObjectId) {
        let id = doc.scene().fresh_ids(1)[0];
        let at = doc.scene().len();
        let mut arrow = Arrow::new(Point::new(10.0, 20.0), Point::new(90.0, 60.0));
        arrow.color = palette::CORAL;
        History::new()
            .execute(
                &mut doc,
                "Add",
                Command::Insert {
                    nodes: vec![(at, Node::new(id, Box::new(arrow)))],
                    resources: Vec::new(),
                },
            )
            .unwrap();
        (doc, id)
    }

    #[test]
    fn objects_round_trip_like_a_saved_document() {
        // The same JSON a saved and reopened document gives, embedded images included.
        let doc = sample();
        let ids = doc.scene().ids();
        let bytes = encode(&doc, &ids).unwrap().unwrap();
        let clip = decode(&bytes).unwrap();
        assert_eq!(clip.len(), 2);
        let saved = archive::document_json(&doc).unwrap();
        let pasted: Vec<Value> = clip
            .nodes
            .iter()
            .map(|n| node_to_json(n).unwrap())
            .collect();
        assert_eq!(Value::Array(pasted), saved["objects"]);
        // Both image objects share one embedded image: it travels once, pixel for pixel.
        assert_eq!(clip.resources.len(), 1);
        let (id, bitmap) = &clip.resources[0];
        let original = doc.resources().get(id).unwrap();
        assert_eq!(bitmap.png().unwrap(), original.png().unwrap());
        // The versioning keys of document.json.
        let json = objects_json(&bytes);
        assert_eq!(json["format"], json!("1.0"));
        assert_eq!(json["requires"], json!(REQUIRES));
        assert!(json.get("image").is_none() && json.get("canvas").is_none());
        let zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut names: Vec<&str> = zip.file_names().collect();
        names.sort_unstable();
        assert_eq!(names, vec!["images/1.png", "objects.json"]);
    }

    #[test]
    fn only_the_selected_objects_in_stacking_order() {
        let (doc, arrow) = with_arrow(sample());
        let ids = doc.scene().ids();
        // Asked top-first: written bottom to top, as they are stacked.
        let bytes = encode(&doc, &[arrow, ids[0]]).unwrap().unwrap();
        let clip = decode(&bytes).unwrap();
        let tags: Vec<&str> = clip.nodes.iter().map(|n| n.object.type_name()).collect();
        assert_eq!(tags, vec!["image", "arrow"]);
        // Just the arrow: no embedded image travels with it.
        let clip = decode(&encode(&doc, &[arrow]).unwrap().unwrap()).unwrap();
        assert_eq!(clip.len(), 1);
        assert!(clip.resources.is_empty());
        // Nothing (an id that is not there): no payload.
        assert!(encode(&doc, &[ObjectId(999)]).unwrap().is_none());
        assert!(encode(&doc, &[]).unwrap().is_none());
    }

    #[test]
    fn groups_travel_with_their_members() {
        let mut doc = sample();
        let ids = doc.scene().ids();
        let mut sel = crate::editing::selection::Selection::new();
        sel.set(doc.scene(), ids);
        let edit = crate::editing::ops::group(&doc, &sel).unwrap();
        History::new()
            .execute(&mut doc, "Group", edit.command)
            .unwrap();
        let group = doc.scene().ids()[0];
        let clip = decode(&encode(&doc, &[group]).unwrap().unwrap()).unwrap();
        let back = clip.nodes[0]
            .object
            .as_any()
            .downcast_ref::<Group>()
            .unwrap();
        assert_eq!(back.children.len(), 2);
        assert_eq!(clip.resources.len(), 1, "the members' image");
    }

    #[test]
    fn newer_minor_payload_keeps_what_this_version_does_not_know() {
        // Spec document-format "Forward compatibility", applied to the clipboard: a payload of
        // a newer Annota (format 1.3) with an unknown key on a known object, an unknown object
        // type that uses an image, and a known type this version cannot read.
        let (doc, arrow) = with_arrow(sample());
        let image = doc.scene().ids()[0];
        let bytes = with_json(&encode(&doc, &[image, arrow]).unwrap().unwrap(), |v| {
            v["format"] = json!("1.3");
            v["clipboard_hint"] = json!("dropped: not part of any document");
            v["objects"][1]["glow"] = json!({"radius": 4});
            v["objects"][1]["shadow"]["spread"] = json!(1.5);
            let objects = v["objects"].as_array_mut().unwrap();
            objects.push(json!({"id": 40, "type": "sticker", "art": "images/1.png", "x": 3}));
            objects.push(json!({"id": 41, "type": "arrow", "head": {"kind": "spiral"}}));
        });
        let clip = decode(&bytes).unwrap();
        assert_eq!(clip.format_minor, 3);
        assert_eq!(clip.len(), 4);
        assert!(clip.nodes[1].residual.is_some(), "unknown arrow keys kept");
        let written = node_to_json(&clip.nodes[1]).unwrap();
        assert_eq!(written["glow"], json!({"radius": 4}));
        assert_eq!(written["shadow"]["spread"], json!(1.5));
        assert_eq!(clip.nodes[2].object.type_name(), UnknownObject::TYPE);
        assert_eq!(clip.nodes[2].object.resources(), vec!["images/1.png"]);
        assert_eq!(clip.nodes[3].object.type_name(), UnknownObject::TYPE);
        assert_eq!(
            node_to_json(&clip.nodes[3]).unwrap(),
            json!({"id": 41, "type": "arrow", "head": {"kind": "spiral"}})
        );
        // In a payload of this version, the unreadable arrow is damage, not a newer feature.
        let current = with_json(&bytes, |v| v["format"] = json!("1.0"));
        assert!(decode(&current).is_err());
    }

    #[test]
    fn newer_major_payload_is_refused() {
        let doc = sample();
        let bytes = with_json(&encode(&doc, &doc.scene().ids()).unwrap().unwrap(), |v| {
            v["format"] = json!("2.0");
            v["requires"] = json!("2.1");
            v["objects"] = json!("anything may change in a new major version");
        });
        match decode(&bytes) {
            Err(ArchiveError::NewerFormat { found, requires }) => {
                assert_eq!(found.major, 2);
                assert_eq!(requires.as_deref(), Some("2.1"));
            }
            other => panic!("expected NewerFormat, got {other:?}"),
        }
    }

    #[test]
    fn damaged_payloads_fail_cleanly() {
        assert!(matches!(decode(b"not a zip"), Err(ArchiveError::Zip(_))));
        let doc = sample();
        let good = encode(&doc, &doc.scene().ids()).unwrap().unwrap();
        type Edit = Box<dyn FnOnce(&mut Value)>;
        let cases: Vec<(&str, Edit)> = vec![
            (
                "no format",
                Box::new(|v: &mut Value| v["format"] = json!(null)),
            ),
            (
                "no objects",
                Box::new(|v: &mut Value| v["objects"] = json!({})),
            ),
            (
                "bad ref",
                Box::new(|v: &mut Value| v["objects"][0]["source"] = json!("../x.png")),
            ),
            (
                "missing ref",
                Box::new(|v: &mut Value| v["objects"][0]["source"] = json!("images/7.png")),
            ),
            (
                "bad field",
                Box::new(|v: &mut Value| v["objects"][0]["x"] = json!("left")),
            ),
            (
                "no id",
                Box::new(|v: &mut Value| v["objects"][0]["id"] = json!(0)),
            ),
        ];
        for (name, edit) in cases {
            assert!(
                decode(&with_json(&good, edit)).is_err(),
                "{name} should fail"
            );
        }
        // A document is not a payload.
        assert!(matches!(
            decode(&archive::encode(&doc).unwrap()),
            Err(ArchiveError::Missing(_))
        ));
    }

    #[test]
    fn a_payload_outlives_its_document() {
        // The payload is bytes: nothing in it refers to the document it came from.
        let (doc, arrow) = with_arrow(Document::new(
            Bitmap::filled(50, 40, palette::PAPER).unwrap(),
        ));
        let bytes = encode(&doc, &[arrow]).unwrap().unwrap();
        drop(doc);
        let clip = decode(&bytes).unwrap();
        let back = clip.nodes[0]
            .object
            .as_any()
            .downcast_ref::<Arrow>()
            .unwrap();
        assert_eq!(back.color, palette::CORAL);
        assert_eq!(clip.nodes[0].object.arrange().rect, {
            let a = Arrow::new(Point::new(10.0, 20.0), Point::new(90.0, 60.0));
            crate::document::annotation::Annotation::arrange(&a).rect
        });
    }
}
