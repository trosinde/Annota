//! Object types known to this Annota and the JSON form of scene nodes.
//!
//! A node is stored as the object's own fields plus `"id"` and `"type"`:
//! `{"id": 3, "type": "image", "source": "images/1.png", "x": 940, …}`.
//! New object types are added to [`TYPES`] (one line per type).
//!
//! Forward compatibility (spec document-format): whatever a node contains that its type does
//! not write back — keys at any depth, e.g. `shadow.spread` — is kept as the node's residual
//! (`super::residual`) and merged back on write. Objects of an unknown type become
//! [`UnknownObject`]s that are neither drawn nor selectable but are saved unchanged. In a file
//! of a newer minor version, an object of a *known* type that this version cannot read (e.g. a
//! new value of one of its enums) is kept the same way instead of failing the whole file. A
//! container (group) with such an object anywhere inside is kept the same way as a whole, so
//! it is never moved or resized without the parts this version cannot move.

use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

use super::annotation::{Annotation, Property, PropertyError, PropertyValue, RenderContext};
use super::geometry::{Arrange, Point, Rect};
use super::residual;
use super::scene::{Node, ObjectId};
use crate::tools::arrow::Arrow;
use crate::tools::callout::Callout;
use crate::tools::group::Group;
use crate::tools::highlighter::Highlighter;
use crate::tools::image::Image;
use crate::tools::pen::Pen;
use crate::tools::redaction::Redaction;
use crate::tools::shape::Shape;
use crate::tools::step::Step;
use crate::tools::text::Text;

type Decoder = fn(Value) -> Result<Box<dyn Annotation>, serde_json::Error>;

fn decode<T: Annotation + DeserializeOwned + 'static>(
    value: Value,
) -> Result<Box<dyn Annotation>, serde_json::Error> {
    Ok(Box::new(serde_json::from_value::<T>(value)?))
}

/// Every object type this version reads, by its serialisation tag.
const TYPES: &[(&str, Decoder)] = &[
    (Image::TYPE, decode::<Image>),
    (Group::TYPE, decode::<Group>),
    (Arrow::TYPE, decode::<Arrow>),
    (Arrow::LINE_TYPE, Arrow::decode_line),
    (Pen::TYPE, decode::<Pen>),
    (Shape::TYPE, decode::<Shape>),
    (Text::TYPE, decode::<Text>),
    (Callout::TYPE, decode::<Callout>),
    (Step::TYPE, decode::<Step>),
    (Highlighter::TYPE, decode::<Highlighter>),
    (Redaction::TYPE, decode::<Redaction>),
];

/// The tags of all known object types.
pub fn types() -> impl Iterator<Item = &'static str> {
    TYPES.iter().map(|(name, _)| *name)
}

fn decoder(type_tag: &str) -> Option<Decoder> {
    TYPES
        .iter()
        .find(|(name, _)| *name == type_tag)
        .map(|(_, d)| *d)
}

/// Keys of a node that belong to the scene, not to the object.
pub const RESERVED_KEYS: [&str; 2] = ["id", "type"];

/// An object this version cannot interpret: a type it does not know, or (in a newer minor
/// file) a known type it cannot read. Kept verbatim, not drawn, not selectable.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownObject {
    pub type_tag: String,
    pub fields: Map<String, Value>,
    /// Embedded images it appears to use (`images/<n>.png` strings anywhere in its fields)
    /// that the file actually contains; they are kept and saved with it. Set when the
    /// document is read.
    pub(crate) resources: Vec<String>,
}

impl UnknownObject {
    pub const TYPE: &'static str = "unknown";

    pub fn new(type_tag: impl Into<String>, fields: Map<String, Value>) -> Self {
        Self {
            type_tag: type_tag.into(),
            fields,
            resources: Vec::new(),
        }
    }

    /// Every string in its fields that is a valid resource id, each once.
    pub(crate) fn resource_candidates(&self) -> Vec<String> {
        fn walk(v: &Value, out: &mut std::collections::BTreeSet<String>) {
            match v {
                Value::String(s) if super::parse_resource_id(s).is_some() => {
                    out.insert(s.clone());
                }
                Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
                Value::Object(map) => map.values().for_each(|i| walk(i, out)),
                _ => {}
            }
        }
        let mut out = std::collections::BTreeSet::new();
        self.fields.values().for_each(|v| walk(v, &mut out));
        out.into_iter().collect()
    }
}

impl Serialize for UnknownObject {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.fields.serialize(serializer)
    }
}

impl Annotation for UnknownObject {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }
    fn bounds(&self) -> Rect {
        Rect::NONE
    }
    fn hit_test(&self, _: Point, _: f64) -> bool {
        false
    }
    fn render(&self, _: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        Ok(())
    }
    fn arrange(&self) -> Arrange {
        Arrange::default()
    }
    fn set_arrange(&mut self, _: Arrange) {}
    fn properties(&self) -> Vec<Property> {
        Vec::new()
    }
    fn set_property(&mut self, key: &str, _: &PropertyValue) -> Result<(), PropertyError> {
        Err(PropertyError::Unknown(key.to_string()))
    }
    fn handles(&self) -> Vec<super::annotation::Handle> {
        Vec::new()
    }
    fn translate(&mut self, _: f64, _: f64) {}
    fn resources(&self) -> Vec<&str> {
        self.resources.iter().map(String::as_str).collect()
    }
    /// Anything inside that looks like a scene node (`"id"` and `"type"`): a newer version's
    /// container type may hold children, whose ids must stay unique.
    fn nested_ids(&self) -> Vec<ObjectId> {
        fn walk(v: &Value, out: &mut Vec<ObjectId>) {
            match v {
                Value::Object(map) => {
                    if let (Some(id), Some(Value::String(_))) =
                        (map.get("id").and_then(Value::as_u64), map.get("type"))
                    {
                        out.push(ObjectId(id));
                    }
                    map.values().for_each(|i| walk(i, out));
                }
                Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        self.fields.values().for_each(|v| walk(v, &mut out));
        out
    }
}

fn invalid(message: impl std::fmt::Display) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message)
}

/// Reads one node of `document.json`. With `keep_unreadable` (files of a newer minor
/// version), an object of a known type that does not parse is kept as an [`UnknownObject`]
/// instead of failing.
pub fn node_from_json(value: Value, keep_unreadable: bool) -> Result<Node, serde_json::Error> {
    let Value::Object(mut fields) = value else {
        return Err(invalid("an object must be a JSON object"));
    };
    let id = fields
        .remove("id")
        .and_then(|v| v.as_u64())
        .filter(|&id| id > 0)
        .ok_or_else(|| invalid("an object needs a positive integer \"id\""))?;
    let type_tag = match fields.remove("type") {
        Some(Value::String(t)) => t,
        _ => return Err(invalid("an object needs a string \"type\"")),
    };
    let Some(decoder) = decoder(&type_tag) else {
        let object = UnknownObject::new(type_tag, fields);
        return Ok(Node::new(ObjectId(id), Box::new(object)));
    };
    let original = Value::Object(fields);
    let object = match decoder(original.clone()) {
        Ok(object) => object,
        Err(_) if keep_unreadable => {
            let Value::Object(fields) = original else {
                return Err(invalid("an object must be a JSON object"));
            };
            let object = UnknownObject::new(type_tag, fields);
            return Ok(Node::new(ObjectId(id), Box::new(object)));
        }
        Err(err) => return Err(invalid(format!("object {id} ({type_tag}): {err}"))),
    };
    if contains_unknown(object.as_ref()) {
        let Value::Object(fields) = original else {
            return Err(invalid("an object must be a JSON object"));
        };
        let object = UnknownObject::new(type_tag, fields);
        return Ok(Node::new(ObjectId(id), Box::new(object)));
    }
    // Whatever the type does not write back is unknown to this version: keep it aside.
    let written = object.to_json()?;
    let mut node = Node::new(ObjectId(id), object);
    node.residual = residual::diff(&original, &written);
    Ok(node)
}

/// Whether an object this version cannot interpret is nested anywhere inside `object`.
fn contains_unknown(object: &dyn Annotation) -> bool {
    object.children().iter().any(|child| {
        child.object.as_any().is::<UnknownObject>() || contains_unknown(child.object.as_ref())
    })
}

/// Writes one node for `document.json`.
pub fn node_to_json(node: &Node) -> Result<Value, serde_json::Error> {
    let (type_tag, own) = match node.object.as_any().downcast_ref::<UnknownObject>() {
        Some(unknown) => (
            unknown.type_tag.clone(),
            Value::Object(unknown.fields.clone()),
        ),
        None => (node.object.type_name().to_string(), node.object.to_json()?),
    };
    let mut own = match own {
        Value::Object(_) => own,
        _ => return Err(invalid("an object must serialise to a JSON object")),
    };
    if let Some(r) = &node.residual {
        residual::merge(&mut own, r);
    }
    let mut map = Map::new();
    map.insert("id".into(), Value::from(node.id.0));
    map.insert("type".into(), Value::String(type_tag));
    if let Value::Object(own) = own {
        for (key, value) in own {
            if !RESERVED_KEYS.contains(&key.as_str()) {
                map.insert(key, value);
            }
        }
    }
    Ok(Value::Object(map))
}

/// Checks that `object` reads back from what it writes, so a document holding it stays
/// saveable *and* reopenable (a NaN field serialises as `null`, which does not read back).
/// Unknown objects and types that are not registered (tests) pass.
pub(crate) fn check_reads_back(object: &dyn Annotation) -> Result<(), serde_json::Error> {
    if object.as_any().is::<UnknownObject>() {
        return Ok(());
    }
    let Some(decoder) = decoder(object.type_name()) else {
        return Ok(());
    };
    let json = object.to_json()?;
    if !json.is_object() {
        return Err(invalid("an object must serialise to a JSON object"));
    }
    decoder(json).map(|_| ())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::document::geometry::Rect;
    use serde_json::json;

    #[test]
    fn every_registered_type_round_trips_with_its_tag() {
        for name in types() {
            let sample = match name {
                Image::TYPE => {
                    Box::new(Image::new("images/1.png", Rect::new(1.0, 2.0, 30.0, 40.0)))
                        as Box<dyn Annotation>
                }
                Group::TYPE => Box::new(Group::new(vec![Node::new(
                    ObjectId(8),
                    Box::new(Image::new("images/1.png", Rect::new(1.0, 2.0, 30.0, 40.0))),
                )])),
                other => sample(other),
            };
            assert_eq!(sample.type_name(), name);
            let node = Node::new(ObjectId(7), sample);
            let value = node_to_json(&node).unwrap();
            assert_eq!(value["type"], json!(name));
            assert_eq!(value["id"], json!(7));
            let own = node.object.to_json().unwrap();
            for key in RESERVED_KEYS {
                assert!(own.get(key).is_none(), "{name} must not write '{key}'");
            }
            let back = node_from_json(value.clone(), false).unwrap();
            assert!(back.residual.is_none());
            assert_eq!(node_to_json(&back).unwrap(), value);
        }
    }

    /// An object of every type added by add-core-annotation-tools, with non-default values
    /// where they matter for the format.
    pub(crate) fn sample(type_name: &str) -> Box<dyn Annotation> {
        use crate::document::geometry::Point;
        let (a, b) = (Point::new(10.0, 20.0), Point::new(200.0, 80.0));
        let rect = Rect::new(10.0, 20.0, 190.0, 60.0);
        match type_name {
            Arrow::TYPE => Box::new(Arrow::new(a, b)),
            Arrow::LINE_TYPE => Box::new(Arrow::line(a, b)),
            Pen::TYPE => {
                let mut pen = Pen::new(a);
                pen.points.push(b);
                Box::new(pen)
            }
            Shape::TYPE => {
                let style = Shape::new(rect);
                Box::new(Shape::polygon(&[a, b, Point::new(10.0, 90.0)], &style).unwrap())
            }
            Text::TYPE => {
                let mut text = Text::new(a);
                text.text = "Runs nightly\non mains power".into();
                Box::new(text)
            }
            Callout::TYPE => {
                let mut callout = Callout::new(rect, Point::new(300.0, 300.0));
                callout.text = "Both paths".into();
                callout.tails.push(Point::new(0.0, 300.0));
                Box::new(callout)
            }
            Step::TYPE => {
                let mut step = Step::new(a, 2, 3);
                step.value = Some("7".into());
                Box::new(step)
            }
            Highlighter::TYPE => Box::new(Highlighter::new(rect)),
            Redaction::TYPE => Box::new(Redaction::new(rect)),
            other => panic!("add a sample for new type '{other}'"),
        }
    }

    #[test]
    fn unknown_keys_of_a_known_type_are_preserved() {
        let mut value = node_to_json(&Node::new(
            ObjectId(1),
            Box::new(Image::new("images/1.png", Rect::new(0.0, 0.0, 10.0, 10.0))),
        ))
        .unwrap();
        value["glow"] = json!({"radius": 4});
        value["shadow"]["spread"] = json!(2.5);
        let node = node_from_json(value.clone(), false).unwrap();
        assert!(node.residual.is_some());
        assert_eq!(node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn unknown_types_are_kept_verbatim() {
        let value = json!({"id": 4, "type": "hologram", "x": 1, "colour": "#FF0000FF"});
        let node = node_from_json(value.clone(), false).unwrap();
        assert_eq!(node.object.type_name(), UnknownObject::TYPE);
        assert!(node.object.bounds().is_none());
        assert!(!node.object.hit_test(Point::new(1.0, 0.0), 10.0));
        assert_eq!(node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn malformed_nodes_are_rejected() {
        assert!(node_from_json(json!([1, 2]), false).is_err());
        assert!(node_from_json(json!({"type": "image"}), false).is_err());
        assert!(node_from_json(json!({"id": 0, "type": "image"}), false).is_err());
        assert!(node_from_json(json!({"id": 1}), false).is_err());
        assert!(node_from_json(json!({"id": 1, "type": "image", "x": "left"}), false).is_err());
        // Only id/type problems stay fatal in a newer minor file.
        assert!(node_from_json(json!({"type": "image"}), true).is_err());
    }

    #[test]
    fn unreadable_known_type_is_kept_in_newer_minor_files() {
        let value = json!({"id": 5, "type": "image", "source": "images/2.png",
                           "x": 1, "border_style": {"kind": "wavy"}, "shadow": "soft"});
        let node = node_from_json(value.clone(), true).unwrap();
        let unknown = node
            .object
            .as_any()
            .downcast_ref::<UnknownObject>()
            .unwrap();
        assert_eq!(unknown.type_tag, "image");
        assert_eq!(
            unknown.resource_candidates(),
            vec!["images/2.png".to_string()]
        );
        assert_eq!(node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn unknown_containers_report_nested_ids() {
        let value = json!({"id": 4, "type": "stack", "children": [
            {"id": 9, "type": "arrow"}, {"id": 12, "type": "stack", "children": [{"id": 30, "type": "text"}]}
        ], "meta": {"id": 99}});
        let node = node_from_json(value, false).unwrap();
        let mut ids = node.object.nested_ids();
        ids.sort();
        assert_eq!(ids, vec![ObjectId(9), ObjectId(12), ObjectId(30)]);
    }

    #[test]
    fn nan_fields_do_not_read_back() {
        let mut img = Image::new("images/1.png", Rect::new(0.0, 0.0, 10.0, 10.0));
        assert!(check_reads_back(&img).is_ok());
        img.opacity = f64::NAN;
        assert!(check_reads_back(&img).is_err());
    }
}
