//! `.annota` files (spec document-format; CLAUDE.md §2): a ZIP archive with
//!
//! * `document.json` — the versioned scene graph, schema `data/schema/document-1.schema.json`
//! * `original.png` — the original image
//! * `thumbnail.png` — the flattened canvas, at most [`THUMBNAIL_SIZE`] px on the longer side
//! * `images/<n>.png` — embedded images used by image objects (only if there are any)
//!
//! PNG entries are stored uncompressed (PNG is compressed already), `document.json` is
//! deflated. Files are written to a temporary file and renamed, so a crash never leaves a
//! half-written document behind.
//!
//! **Versioning.** `document.json` carries `"format": "MAJOR.MINOR"` and `"requires"`, the
//! oldest Annota release that reads that format. A newer *minor* version only adds data: it
//! opens silently, and whatever this version does not know is kept and written back — keys at
//! any depth (`super::residual`), object types, unknown canvas background types
//! ([`Background::Unknown`], drawn as transparent), objects of known types this version cannot
//! read (kept as `UnknownObject`s), and the embedded images such objects use. A newer *major* version is refused with [`ArchiveError::NewerFormat`], which names the
//! format and the required Annota version, before anything else in the file is interpreted.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::bitmap::{Bitmap, BitmapError};
use super::geometry::IRect;
use super::registry::{node_from_json, node_to_json, UnknownObject};
use super::residual;
use super::scene::Scene;
use super::{Background, BaseImage, Canvas, Document, Metadata, Resources};

/// The format this version writes.
pub const FORMAT: FormatVersion = FormatVersion { major: 1, minor: 0 };
/// First Annota release that reads format 1.x.
pub const REQUIRES: &str = "0.1";

pub const DOCUMENT_JSON: &str = "document.json";
pub const ORIGINAL_PNG: &str = "original.png";
pub const THUMBNAIL_PNG: &str = "thumbnail.png";
/// Longer side of `thumbnail.png` (large library tiles are 230 px wide at 2× scale).
pub const THUMBNAIL_SIZE: i32 = 512;

/// Refuse `document.json` above this size.
const MAX_JSON: u64 = 64 << 20;
/// Refuse a single PNG entry above this size.
const MAX_PNG: u64 = 512 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FormatVersion {
    pub major: u32,
    pub minor: u32,
}

impl FormatVersion {
    pub fn parse(text: &str) -> Option<Self> {
        let (major, minor) = text.split_once('.')?;
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(major) || !digits(minor) {
            return None;
        }
        Some(Self {
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
        })
    }
}

impl fmt::Display for FormatVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not an Annota document: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("damaged document.json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("the document has no {0}")]
    Missing(String),
    #[error("damaged document: {0}")]
    Invalid(String),
    #[error("{0}")]
    Bitmap(#[from] BitmapError),
    /// The file needs a newer Annota (mockup 15, "Newer Annota Version Needed").
    #[error("document format {found} needs Annota {} or newer; this version reads format {}.x",
        requires.as_deref().unwrap_or("?"), FORMAT.major)]
    NewerFormat {
        found: FormatVersion,
        /// The `requires` field of the file, if it has one.
        requires: Option<String>,
    },
}

// ---- JSON shape (see data/schema/document-1.schema.json) ----

#[derive(Serialize, Deserialize)]
struct FileJson {
    format: String,
    #[serde(default)]
    requires: Option<String>,
    #[serde(default)]
    generator: Option<String>,
    image: ImageJson,
    canvas: CanvasJson,
    #[serde(default)]
    metadata: MetadataJson,
    objects: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    autosave: Option<AutosaveJson>,
}

#[derive(Serialize, Deserialize)]
struct ImageJson {
    file: String,
    width: i32,
    height: i32,
    #[serde(default)]
    corner_radius: f64,
}

#[derive(Serialize, Deserialize)]
struct CanvasJson {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    background: Background,
}

#[derive(Serialize, Deserialize)]
struct MetadataJson {
    #[serde(default)]
    title: Option<String>,
    #[serde(default = "one")]
    scale_factor: f64,
}

fn one() -> f64 {
    1.0
}

impl Default for MetadataJson {
    fn default() -> Self {
        Self {
            title: None,
            scale_factor: 1.0,
        }
    }
}

/// Only present in autosave copies (`super::autosave`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct AutosaveJson {
    /// The file the document was opened from or last saved to, if any.
    #[serde(default)]
    pub source: Option<String>,
}

/// Keys of `document.json` that are not sections of the typed document: objects are kept per
/// node, the autosave key belongs to the autosave copy only.
const NOT_SECTIONS: [&str; 2] = ["objects", "autosave"];

/// `document.json` of `doc` as a JSON value (the file's content, minus autosave data).
pub fn document_json(doc: &Document) -> Result<Value, ArchiveError> {
    file_json(doc, None)
}

/// Everything but `objects` and `autosave`, as this version writes it (no residual).
fn sections_json(doc: &Document) -> Result<Map<String, Value>, ArchiveError> {
    let canvas = doc.canvas();
    let image = doc.image();
    let metadata = doc.metadata();
    let format = FormatVersion {
        major: FORMAT.major,
        // Keep a newer minor version: the file still carries that version's data.
        minor: if doc.format_minor > FORMAT.minor {
            doc.format_minor
        } else {
            FORMAT.minor
        },
    };
    let json = FileJson {
        format: format.to_string(),
        requires: Some(REQUIRES.to_string()),
        generator: Some(format!("Annota {}", env!("CARGO_PKG_VERSION"))),
        image: ImageJson {
            file: ORIGINAL_PNG.to_string(),
            width: image.bitmap.width(),
            height: image.bitmap.height(),
            corner_radius: image.corner_radius,
        },
        canvas: CanvasJson {
            x: canvas.rect.x,
            y: canvas.rect.y,
            width: canvas.rect.width,
            height: canvas.rect.height,
            background: canvas.background,
        },
        metadata: MetadataJson {
            title: metadata.title.clone(),
            scale_factor: metadata.scale_factor,
        },
        objects: Vec::new(),
        autosave: None,
    };
    let Value::Object(mut map) = serde_json::to_value(json)? else {
        return Err(ArchiveError::Invalid(
            "document.json is not an object".into(),
        ));
    };
    for key in NOT_SECTIONS {
        map.remove(key);
    }
    Ok(map)
}

fn file_json(doc: &Document, autosave: Option<AutosaveJson>) -> Result<Value, ArchiveError> {
    let mut root = Value::Object(sections_json(doc)?);
    if let Some(r) = &doc.residual {
        residual::merge(&mut root, r);
    }
    // An unknown background writes nothing of its own; its keys come from the residual. If
    // there are none (it can only come from a file, so this is defensive), fall back to the
    // background it is drawn as.
    if let Some(background) = root.pointer_mut("/canvas/background") {
        if background.get("type").is_none() {
            *background = serde_json::json!({"type": "transparent"});
        }
    }
    let objects = doc
        .scene()
        .nodes()
        .iter()
        .map(node_to_json)
        .collect::<Result<Vec<_>, _>>()?;
    if let Value::Object(map) = &mut root {
        map.insert("objects".into(), Value::Array(objects));
        if let Some(autosave) = autosave {
            map.insert("autosave".into(), serde_json::to_value(autosave)?);
        }
    }
    Ok(root)
}

/// Resource ids the objects use, each once, in a stable order.
fn used_resources(doc: &Document) -> BTreeSet<String> {
    doc.scene()
        .nodes()
        .iter()
        .flat_map(|n| n.object.resources().into_iter().map(str::to_string))
        .collect()
}

/// The bytes of a `.annota` file for `doc`.
pub fn encode(doc: &Document) -> Result<Vec<u8>, ArchiveError> {
    encode_with(doc, None)
}

pub(crate) fn encode_with(
    doc: &Document,
    autosave: Option<AutosaveJson>,
) -> Result<Vec<u8>, ArchiveError> {
    let json = serde_json::to_vec_pretty(&file_json(doc, autosave)?)?;
    let thumbnail = crate::render::thumbnail(doc, THUMBNAIL_SIZE)?.png()?;
    let original = doc.image().bitmap.png()?;

    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(DOCUMENT_JSON, deflated)?;
    zip.write_all(&json)?;
    zip.start_file(ORIGINAL_PNG, stored)?;
    zip.write_all(&original)?;
    zip.start_file(THUMBNAIL_PNG, stored)?;
    zip.write_all(&thumbnail)?;
    for id in used_resources(doc) {
        let bitmap = doc
            .resources()
            .get(&id)
            .ok_or_else(|| ArchiveError::Missing(id.clone()))?;
        zip.start_file(id.as_str(), stored)?;
        zip.write_all(&bitmap.png()?)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// Saves `doc` to `path`, replacing the file only once the new one is complete.
pub fn save(doc: &Document, path: &Path) -> Result<(), ArchiveError> {
    let bytes = encode(doc)?;
    write_atomically(path, &bytes)?;
    Ok(())
}

/// Writes `bytes` to a temporary file next to `path`, syncs it and renames it over `path`.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no file name"))?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    written
}

fn read_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
) -> Result<Vec<u8>, ArchiveError> {
    let entry = match zip.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(ArchiveError::Missing(name.to_string()))
        }
        Err(err) => return Err(err.into()),
    };
    if entry.size() > limit {
        return Err(ArchiveError::Invalid(format!("{name} is too large")));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(ArchiveError::Invalid(format!("{name} is too large")));
    }
    Ok(bytes)
}

/// Reads `document.json` and refuses newer major versions. Also returns the parsed JSON as
/// it was, for forward compatibility.
fn read_document_json<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
) -> Result<(FileJson, FormatVersion, Value), ArchiveError> {
    let bytes = read_entry(zip, DOCUMENT_JSON, MAX_JSON)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .and_then(FormatVersion::parse)
        .ok_or_else(|| ArchiveError::Invalid("missing or malformed \"format\"".into()))?;
    if format.major > FORMAT.major {
        return Err(ArchiveError::NewerFormat {
            found: format,
            requires: value
                .get("requires")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
    if format.major < FORMAT.major {
        return Err(ArchiveError::Invalid(format!("unknown format {format}")));
    }
    Ok((serde_json::from_value(value.clone())?, format, value))
}

/// A document read from a file, plus what an autosave copy remembers.
pub(crate) struct Opened {
    pub document: Document,
    pub autosave: Option<AutosaveJson>,
}

pub(crate) fn decode_from<R: Read + Seek>(reader: R) -> Result<Opened, ArchiveError> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let (json, format, original) = read_document_json(&mut zip)?;
    // A newer minor version may add values this version cannot read: keep those objects.
    let newer_minor = format.minor > FORMAT.minor;

    if json.image.file != ORIGINAL_PNG {
        return Err(ArchiveError::Invalid(format!(
            "unexpected image file '{}'",
            json.image.file
        )));
    }
    let bitmap = Bitmap::from_png(read_entry(&mut zip, ORIGINAL_PNG, MAX_PNG)?)?;
    if (bitmap.width(), bitmap.height()) != (json.image.width, json.image.height) {
        return Err(ArchiveError::Invalid(
            "original.png does not have the recorded size".into(),
        ));
    }
    if !(json.image.corner_radius.is_finite() && json.image.corner_radius >= 0.0) {
        return Err(ArchiveError::Invalid("invalid corner radius".into()));
    }
    if json.canvas.width <= 0 || json.canvas.height <= 0 {
        return Err(ArchiveError::Invalid("invalid canvas size".into()));
    }
    if !(json.metadata.scale_factor.is_finite() && json.metadata.scale_factor > 0.0) {
        return Err(ArchiveError::Invalid("invalid scale factor".into()));
    }

    let mut nodes = json
        .objects
        .into_iter()
        .map(|v| node_from_json(v, newer_minor))
        .collect::<Result<Vec<_>, _>>()?;

    let mut resources = Resources::default();
    let ids: BTreeSet<String> = nodes
        .iter()
        .flat_map(|n| n.object.resources().into_iter().map(str::to_string))
        .collect();
    for id in ids {
        if super::parse_resource_id(&id).is_none() {
            return Err(ArchiveError::Invalid(format!(
                "invalid image reference '{id}'"
            )));
        }
        let png = read_entry(&mut zip, &id, MAX_PNG)?;
        resources.insert(id, Bitmap::from_png(png)?);
    }
    // Objects this version cannot interpret keep the embedded images they appear to use, so
    // saving does not drop them. Anything that is not a readable image entry is ignored.
    for node in &mut nodes {
        let Some(unknown) = node.object.as_any_mut().downcast_mut::<UnknownObject>() else {
            continue;
        };
        for id in unknown.resource_candidates() {
            if !resources.contains(&id) {
                let Ok(png) = read_entry(&mut zip, &id, MAX_PNG) else {
                    continue;
                };
                let Ok(bitmap) = Bitmap::from_png(png) else {
                    continue;
                };
                resources.insert(id.clone(), bitmap);
            }
            unknown.resources.push(id);
        }
    }
    for node in &nodes {
        super::command::check_geometry(node.id, node.object.as_ref())
            .map_err(|e| ArchiveError::Invalid(e.to_string()))?;
    }
    let scene = Scene::try_from_nodes(nodes).map_err(|e| ArchiveError::Invalid(e.to_string()))?;

    let mut document = Document::from_parts(
        BaseImage {
            bitmap,
            corner_radius: json.image.corner_radius,
        },
        Canvas {
            rect: IRect::new(
                json.canvas.x,
                json.canvas.y,
                json.canvas.width,
                json.canvas.height,
            ),
            background: json.canvas.background,
        },
        Metadata {
            title: json.metadata.title,
            scale_factor: json.metadata.scale_factor,
        },
        scene,
        resources,
    );
    document.format_minor = format.minor;
    // Whatever this version does not write back of the sections (unknown keys at any depth,
    // an unknown background type) stays with the document.
    let mut original = original;
    if let Value::Object(map) = &mut original {
        for key in NOT_SECTIONS {
            map.remove(key);
        }
    }
    document.residual = residual::diff(&original, &Value::Object(sections_json(&document)?));
    Ok(Opened {
        document,
        autosave: json.autosave,
    })
}

/// Reads a document from `.annota` bytes.
pub fn decode(bytes: &[u8]) -> Result<Document, ArchiveError> {
    Ok(decode_from(Cursor::new(bytes))?.document)
}

/// Opens a `.annota` file. The file is only read, never changed.
pub fn load(path: &Path) -> Result<Document, ArchiveError> {
    Ok(decode_from(BufReader::new(File::open(path)?))?.document)
}

/// What can be shown about a file without decoding its images (recovery dialog, library).
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub format: FormatVersion,
    pub title: Option<String>,
    pub object_count: usize,
    pub canvas: IRect,
    /// `thumbnail.png`, if the file has one.
    pub thumbnail_png: Option<Vec<u8>>,
    /// Autosave copies: the file the document belongs to.
    pub source: Option<PathBuf>,
}

pub fn read_summary(path: &Path) -> Result<Summary, ArchiveError> {
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
    let (json, format, _) = read_document_json(&mut zip)?;
    let thumbnail_png = match read_entry(&mut zip, THUMBNAIL_PNG, MAX_PNG) {
        Ok(bytes) => Some(bytes),
        Err(ArchiveError::Missing(_)) => None,
        Err(err) => return Err(err),
    };
    Ok(Summary {
        format,
        title: json.metadata.title,
        object_count: json.objects.len(),
        canvas: IRect::new(
            json.canvas.x,
            json.canvas.y,
            json.canvas.width,
            json.canvas.height,
        ),
        thumbnail_png,
        source: json.autosave.and_then(|a| a.source).map(PathBuf::from),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::document::color::palette;
    use crate::document::command::{Command, History};
    use crate::document::geometry::Point;
    use crate::tools::image::Image;
    use serde_json::json;

    pub(crate) fn sample() -> Document {
        let base = Bitmap::draw(160, 100, |cr| {
            cr.set_source_rgb(0.9, 0.9, 0.85);
            cr.paint()?;
            cr.set_source_rgb(0.1, 0.4, 0.3);
            cr.rectangle(20.0, 20.0, 60.0, 30.0);
            cr.fill()
        })
        .unwrap();
        let mut doc = Document::with_title(base, "Checkout flow");
        let mut h = History::new();
        let pip = Bitmap::filled(80, 50, palette::AZURE).unwrap();
        let cmd = Image::insert_command(&doc, pip.clone(), Point::new(110.0, 70.0));
        h.execute(&mut doc, "Insert", cmd).unwrap();
        // A second object using the same embedded image, rotated and restyled.
        let ids = doc.scene().ids();
        let mut second = doc.scene().get(ids[0]).unwrap().object.clone();
        second.translate(-60.0, -30.0);
        let mut a = second.arrange();
        a.rotation = 15.0;
        second.set_arrange(a);
        second
            .set_property(
                "border_color",
                &crate::document::annotation::PropertyValue::Color(palette::CORAL),
            )
            .unwrap();
        let id = doc.scene().fresh_ids(1)[0];
        h.execute(
            &mut doc,
            "Duplicate",
            Command::Insert {
                nodes: vec![(1, crate::document::scene::Node::new(id, second))],
                resources: Vec::new(),
            },
        )
        .unwrap();
        h.execute(
            &mut doc,
            "Extend Canvas",
            Command::SetCanvas(Canvas {
                rect: IRect::new(-20, 0, 200, 100),
                background: Background::Color {
                    color: palette::PAPER,
                },
            }),
        )
        .unwrap();
        doc
    }

    fn pixels_equal(a: &Bitmap, b: &Bitmap) -> bool {
        a.width() == b.width()
            && a.height() == b.height()
            && (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)))
    }

    #[test]
    fn round_trip_keeps_every_object_and_property() {
        // Spec document-format "Round trip" (with every object type this version has).
        let doc = sample();
        let bytes = encode(&doc).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(document_json(&back).unwrap(), document_json(&doc).unwrap());
        assert_eq!(back.object_count(), 2);
        assert_eq!(back.metadata().title.as_deref(), Some("Checkout flow"));
        assert_eq!(back.canvas(), doc.canvas());
        assert!(pixels_equal(&back.image().bitmap, &doc.image().bitmap));
        // Both objects share one embedded image, stored once.
        assert_eq!(back.resources().len(), 1);
        let a = crate::render::flatten(&doc, 1.0).unwrap();
        let b = crate::render::flatten(&back, 1.0).unwrap();
        assert!(pixels_equal(&a, &b));
        // Saving the reopened document again gives the same JSON.
        let again = decode(&encode(&back).unwrap()).unwrap();
        assert_eq!(document_json(&again).unwrap(), document_json(&doc).unwrap());
    }

    #[test]
    fn archive_layout() {
        let bytes = encode(&sample()).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut names: Vec<&str> = zip.file_names().collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![
                "document.json",
                "images/1.png",
                "original.png",
                "thumbnail.png"
            ]
        );
        let thumb =
            Bitmap::from_png(read_entry(&mut zip, THUMBNAIL_PNG, MAX_PNG).unwrap()).unwrap();
        assert_eq!(
            (thumb.width(), thumb.height()),
            (200, 100),
            "small canvases are not enlarged"
        );
        let json: Value =
            serde_json::from_slice(&read_entry(&mut zip, DOCUMENT_JSON, MAX_JSON).unwrap())
                .unwrap();
        assert_eq!(json["format"], json!("1.0"));
        assert_eq!(json["requires"], json!(REQUIRES));
        assert_eq!(json["objects"][0]["type"], json!("image"));
        assert!(json.get("autosave").is_none());
    }

    #[test]
    fn unused_embedded_images_are_not_saved() {
        let mut doc = sample();
        let mut h = History::new();
        let ids = doc.scene().ids();
        h.execute(&mut doc, "Delete", Command::Remove { ids })
            .unwrap();
        let bytes = encode(&doc).unwrap();
        let zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(!zip.file_names().any(|n| n.starts_with("images/")));
    }

    /// Rewrites `document.json` inside an archive.
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
            if name == DOCUMENT_JSON {
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

    #[test]
    fn newer_major_names_the_required_version() {
        // Spec document-format "Newer major" (mockup 15: format 2.0, Annota 2.1).
        let bytes = with_json(&encode(&sample()).unwrap(), |v| {
            v["format"] = json!("2.0");
            v["requires"] = json!("2.1");
            // Anything may have changed in a new major version.
            v["objects"] = json!("not a list any more");
        });
        match decode(&bytes) {
            Err(ArchiveError::NewerFormat { found, requires }) => {
                assert_eq!(found, FormatVersion { major: 2, minor: 0 });
                assert_eq!(requires.as_deref(), Some("2.1"));
            }
            other => panic!("expected NewerFormat, got {other:?}"),
        }
        let err = decode(&bytes).unwrap_err().to_string();
        assert!(err.contains("2.0") && err.contains("2.1"), "{err}");
    }

    #[test]
    fn newer_major_file_is_not_changed() {
        let dir = std::env::temp_dir().join(format!("annota-archive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Release notes v2.annota");
        let bytes = with_json(&encode(&sample()).unwrap(), |v| v["format"] = json!("2.0"));
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            load(&path),
            Err(ArchiveError::NewerFormat {
                requires: Some(_),
                ..
            })
        ));
        assert!(matches!(
            read_summary(&path),
            Err(ArchiveError::NewerFormat { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn newer_minor_opens_and_keeps_unknown_data() {
        let bytes = with_json(&encode(&sample()).unwrap(), |v| {
            v["format"] = json!("1.4");
            v["tags"] = json!(["checkout", "bug"]);
            v["canvas"]["grid"] = json!({"size": 8});
            v["metadata"]["application"] = json!("Firefox");
            v["image"]["dpi"] = json!(144);
            v["objects"][0]["glow"] = json!(true);
            v["objects"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id": 99, "type": "sticky-note", "x": 5, "text": "later"}));
        });
        let doc = decode(&bytes).unwrap();
        assert_eq!(doc.object_count(), 3);
        let saved: Value = document_json(&doc).unwrap();
        assert_eq!(
            saved["format"],
            json!("1.4"),
            "keeps the newer minor version"
        );
        assert_eq!(saved["tags"], json!(["checkout", "bug"]));
        assert_eq!(saved["canvas"]["grid"], json!({"size": 8}));
        assert_eq!(saved["metadata"]["application"], json!("Firefox"));
        assert_eq!(saved["image"]["dpi"], json!(144));
        assert_eq!(saved["objects"][0]["glow"], json!(true));
        assert_eq!(
            saved["objects"][2],
            json!({"id": 99, "type": "sticky-note", "x": 5, "text": "later"})
        );
        // And through a full save/load cycle.
        let again = document_json(&decode(&encode(&doc).unwrap()).unwrap()).unwrap();
        assert_eq!(again, saved);
    }

    /// A `.annota` file from a hand-written `document.json`, a 40 × 20 original and the
    /// given embedded images.
    fn hand_written(json: &str, images: &[&str]) -> Vec<u8> {
        let original = Bitmap::filled(40, 20, palette::CORAL).unwrap();
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        out.start_file(DOCUMENT_JSON, opts).unwrap();
        out.write_all(json.as_bytes()).unwrap();
        out.start_file(ORIGINAL_PNG, opts).unwrap();
        out.write_all(&original.png().unwrap()).unwrap();
        for name in images {
            out.start_file(*name, opts).unwrap();
            out.write_all(&Bitmap::filled(8, 8, palette::AZURE).unwrap().png().unwrap())
                .unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    /// `document.json` as saved after opening `bytes` (save, then read the JSON back out).
    fn resaved(doc: &Document) -> Value {
        let mut zip = zip::ZipArchive::new(Cursor::new(encode(doc).unwrap())).unwrap();
        serde_json::from_slice(&read_entry(&mut zip, DOCUMENT_JSON, MAX_JSON).unwrap()).unwrap()
    }

    const IMAGE_FIELDS: &str = r##""source": "images/1.png", "x": 4, "y": 2, "width": 10, "height": 6,
        "rotation": 0, "opacity": 1, "border_color": "#FFFFFFFF", "border_width": 2,
        "corner_radius": 0"##;

    #[test]
    fn unknown_nested_keys_survive_a_round_trip() {
        // Reviewer repro (B4): unknown keys *inside* known sections and objects were dropped.
        let json = format!(
            r##"{{
            "format": "1.3", "requires": "0.1", "generator": "Annota 0.4.0",
            "image": {{"file": "original.png", "width": 40, "height": 20, "corner_radius": 0,
                       "exif": {{"camera": "none", "dpi": [144, 144]}}}},
            "canvas": {{"x": 0, "y": 0, "width": 40, "height": 20,
                        "background": {{"type": "color", "color": "#FFFFFFFF",
                                        "pattern": {{"kind": "dots", "spacing": 8}}}}}},
            "metadata": {{"title": "Nested", "scale_factor": 2, "source_app": {{"name": "Firefox"}}}},
            "objects": [{{"id": 1, "type": "image", {IMAGE_FIELDS},
                "shadow": {{"enabled": true, "color": "#000006FF", "opacity": 0.4, "angle": 45,
                            "distance": 3, "blur": 2, "spread": 1.5, "inset": {{"on": false}}}},
                "filters": [{{"kind": "sepia", "amount": 0.3}}]}}]
        }}"##
        );
        let original: Value = serde_json::from_str(&json).unwrap();
        let doc = decode(&hand_written(&json, &["images/1.png"])).unwrap();
        let saved = resaved(&doc);
        for pointer in [
            "/image/exif",
            "/canvas/background/pattern",
            "/metadata/source_app",
            "/objects/0/shadow/spread",
            "/objects/0/shadow/inset",
            "/objects/0/filters",
        ] {
            assert_eq!(
                saved.pointer(pointer),
                original.pointer(pointer),
                "{pointer}"
            );
        }
        assert_eq!(saved["format"], json!("1.3"));

        // Editing keeps them, and known keys show the edit: move the object and change its
        // shadow blur through commands.
        let mut doc = doc;
        let mut h = History::new();
        let id = doc.scene().ids()[0];
        let mut object = doc.scene().get(id).unwrap().object.clone();
        object.translate(5.0, 0.0);
        let mut shadow = Image::DEFAULT_SHADOW;
        shadow.blur = 9.0;
        object
            .set_property(
                "shadow",
                &crate::document::annotation::PropertyValue::Shadow(shadow),
            )
            .unwrap();
        h.execute(
            &mut doc,
            "Edit",
            Command::Replace {
                objects: vec![(id, object)],
            },
        )
        .unwrap();
        let mut canvas = *doc.canvas();
        canvas.rect.width = 60;
        h.execute(&mut doc, "Extend", Command::SetCanvas(canvas))
            .unwrap();
        let saved = resaved(&doc);
        assert_eq!(saved["objects"][0]["x"], json!(9.0));
        assert_eq!(saved["objects"][0]["shadow"]["blur"], json!(9.0));
        assert_eq!(saved["objects"][0]["shadow"]["spread"], json!(1.5));
        assert_eq!(saved["canvas"]["width"], json!(60));
        assert_eq!(
            saved["canvas"]["background"]["pattern"],
            json!({"kind": "dots", "spacing": 8})
        );
        // And the saved file reads back to the same JSON.
        assert_eq!(resaved(&decode(&encode(&doc).unwrap()).unwrap()), saved);
    }

    #[test]
    fn unknown_background_type_opens_and_is_kept() {
        // Reviewer repro (B4): a new background variant made the whole file fail to open.
        let json = r##"{
            "format": "1.2", "requires": "0.1",
            "image": {"file": "original.png", "width": 40, "height": 20},
            "canvas": {"x": 0, "y": 0, "width": 60, "height": 20,
                       "background": {"type": "gradient", "angle": 90,
                                      "stops": [{"at": 0, "color": "#FF0000FF"}, {"at": 1, "color": "#0000FFFF"}]}},
            "objects": []
        }"##;
        let original: Value = serde_json::from_str(json).unwrap();
        let doc = decode(&hand_written(json, &[])).unwrap();
        assert_eq!(doc.canvas().background, Background::Unknown);
        // Drawn as transparent: the extension exports as transparent pixels.
        let flat = crate::render::flatten(&doc, 1.0).unwrap();
        assert_eq!(flat.pixel(50, 10).unwrap().a, 0);
        assert_eq!(flat.pixel(10, 10), Some(palette::CORAL));
        assert_eq!(
            resaved(&doc)["canvas"]["background"],
            original["canvas"]["background"]
        );
        // Picking another background replaces it cleanly; undo brings the original back.
        let mut doc = doc;
        let mut h = History::new();
        let mut canvas = *doc.canvas();
        canvas.background = Background::Color {
            color: palette::PAPER,
        };
        h.execute(&mut doc, "Background", Command::SetCanvas(canvas))
            .unwrap();
        assert_eq!(
            resaved(&doc)["canvas"]["background"],
            json!({"type": "color", "color": "#FFFFFFFF"})
        );
        h.undo(&mut doc).unwrap();
        assert_eq!(
            resaved(&doc)["canvas"]["background"],
            original["canvas"]["background"]
        );
    }

    #[test]
    fn unreadable_object_of_a_known_type_is_kept_in_newer_minor_files() {
        // A newer minor version may give a known type a value this version cannot read (here
        // a new shadow representation). Only that object degrades; it keeps its image.
        let json = format!(
            r##"{{
            "format": "1.5", "requires": "0.1",
            "image": {{"file": "original.png", "width": 40, "height": 20}},
            "canvas": {{"x": 0, "y": 0, "width": 40, "height": 20, "background": {{"type": "transparent"}}}},
            "objects": [
                {{"id": 1, "type": "image", {IMAGE_FIELDS}, "shadow": {{"preset": "soft"}}}},
                {{"id": 2, "type": "image", "source": "images/2.png", "x": 0, "y": 0, "width": 8,
                  "height": 8, "rotation": 0, "opacity": 1, "border_color": "#FFFFFFFF",
                  "border_width": 0, "corner_radius": 0,
                  "shadow": {{"enabled": false, "color": "#000006FF", "opacity": 0.3, "angle": 45,
                              "distance": 3, "blur": 2}}}}
            ]
        }}"##
        );
        let original: Value = serde_json::from_str(&json).unwrap();
        let bytes = hand_written(&json, &["images/1.png", "images/2.png"]);
        let doc = decode(&bytes).unwrap();
        assert_eq!(doc.object_count(), 2);
        assert_eq!(doc.scene().nodes()[0].object.type_name(), "unknown");
        assert_eq!(doc.scene().nodes()[1].object.type_name(), Image::TYPE);
        let saved = resaved(&doc);
        assert_eq!(saved["objects"][0], original["objects"][0]);
        let zip = zip::ZipArchive::new(Cursor::new(encode(&doc).unwrap())).unwrap();
        let mut names: Vec<&str> = zip
            .file_names()
            .filter(|n| n.starts_with("images/"))
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["images/1.png", "images/2.png"]);
        // In a file of this version the same object is damage, not a newer feature.
        let current = json.replace("\"1.5\"", "\"1.0\"");
        assert!(decode(&hand_written(&current, &["images/1.png", "images/2.png"])).is_err());
    }

    #[test]
    fn damaged_files_fail_cleanly() {
        assert!(matches!(decode(b"not a zip"), Err(ArchiveError::Zip(_))));
        let good = encode(&sample()).unwrap();
        type Edit = Box<dyn FnOnce(&mut Value)>;
        let cases: Vec<(&str, Edit)> = vec![
            (
                "no format",
                Box::new(|v: &mut Value| v["format"] = json!(null)),
            ),
            (
                "bad format",
                Box::new(|v: &mut Value| v["format"] = json!("one")),
            ),
            (
                "old major",
                Box::new(|v: &mut Value| v["format"] = json!("0.9")),
            ),
            (
                "bad size",
                Box::new(|v: &mut Value| v["image"]["width"] = json!(1)),
            ),
            (
                "bad canvas",
                Box::new(|v: &mut Value| v["canvas"]["width"] = json!(0)),
            ),
            (
                "dup ids",
                Box::new(|v: &mut Value| {
                    let first = v["objects"][0].clone();
                    v["objects"][1]["id"] = first["id"].clone();
                }),
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
                "bad colour",
                Box::new(|v: &mut Value| {
                    v["canvas"]["background"] = json!({"type": "color", "color": "red"})
                }),
            ),
        ];
        for (name, edit) in cases {
            let bytes = with_json(&good, edit);
            assert!(decode(&bytes).is_err(), "{name} should fail");
        }
    }

    #[test]
    fn schema_matches_what_is_written() {
        // Guards data/schema/document-1.schema.json against drifting from the code. (Full
        // validation needs a JSON-schema engine; see design.md for the manual check.)
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/schema/document-1.schema.json"
        )))
        .unwrap();
        let doc = document_json(&sample()).unwrap();
        let keys =
            |v: &Value| -> BTreeSet<String> { v.as_object().unwrap().keys().cloned().collect() };
        let required = |v: &Value| -> BTreeSet<String> {
            v["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|k| k.as_str().unwrap().to_string())
                .collect()
        };
        let props = |v: &Value| keys(&v["properties"]);
        assert!(required(&schema).is_subset(&keys(&doc)));
        assert!(keys(&doc).is_subset(&props(&schema)));
        for section in ["image", "canvas", "metadata"] {
            let s = &schema["properties"][section];
            assert_eq!(keys(&doc[section]), props(s), "{section}");
        }
        assert!(required(&schema["properties"]["image"]).is_subset(&keys(&doc["image"])));
        assert!(required(&schema["properties"]["canvas"]).is_subset(&keys(&doc["canvas"])));
        // Image objects: the schema lists exactly the fields `Image` writes.
        let mut object = doc["objects"][0].as_object().unwrap().clone();
        object.remove("id");
        object.remove("type");
        let object = Value::Object(object);
        let image = &schema["$defs"]["image_object"];
        assert_eq!(keys(&object), required(image));
        assert_eq!(keys(&object), props(image));
        let shadow = &schema["$defs"]["shadow"];
        assert_eq!(keys(&object["shadow"]), required(shadow));
        assert_eq!(
            schema["properties"]["format"]["pattern"],
            json!(format!("^{}\\.[0-9]+$", FORMAT.major))
        );
    }

    #[test]
    fn failed_save_keeps_the_previous_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("annota-ro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.annota");
        save(&sample(), &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        // A directory the temporary file cannot be created in.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let mut changed = sample();
        let mut h = History::new();
        let ids = changed.scene().ids();
        h.execute(&mut changed, "Delete", Command::Remove { ids })
            .unwrap();
        let result = save(&changed, &path);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Running as root ignores the permission; only check when the save really failed.
        if result.is_err() {
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn format_versions_parse() {
        assert_eq!(FormatVersion::parse("1.0"), Some(FORMAT));
        assert_eq!(FormatVersion::parse("12.34").unwrap().to_string(), "12.34");
        for bad in ["1", "1.", ".1", "1.0.0", "a.b", "-1.0", "+1.0", ""] {
            assert_eq!(FormatVersion::parse(bad), None, "{bad}");
        }
        assert!(FormatVersion::parse("2.0").unwrap() > FORMAT);
    }

    #[test]
    fn save_replaces_atomically_and_reads_summary() {
        let dir = std::env::temp_dir().join(format!("annota-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.annota");
        std::fs::write(&path, b"old").unwrap();
        save(&sample(), &path).unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let summary = read_summary(&path).unwrap();
        assert_eq!(summary.title.as_deref(), Some("Checkout flow"));
        assert_eq!(summary.object_count, 2);
        assert_eq!(summary.canvas, IRect::new(-20, 0, 200, 100));
        assert!(Bitmap::from_png(summary.thumbnail_png.unwrap()).is_ok());
        assert_eq!(summary.source, None);
        let opened = load(&path).unwrap();
        assert_eq!(opened.object_count(), 2);
        // Missing directory: error, nothing created.
        assert!(save(&sample(), &dir.join("missing").join("x.annota")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
