//! Object types known to this Annota and the JSON form of scene nodes.
//!
//! A node is stored as the object's own fields plus `"id"` and `"type"`:
//! `{"id": 3, "type": "image", "source": "images/1.png", "x": 940, …}`.
//! New object types are added to [`TYPES`] (one line per type).
//!
//! Forward compatibility (spec document-format): keys of a known type that this version does
//! not write are kept in [`Node::extra`] and written back; objects of an unknown type become
//! [`UnknownObject`]s that are neither drawn nor selectable but are saved unchanged.

use serde::de::DeserializeOwned;
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};

use super::annotation::{Annotation, Property, PropertyError, PropertyValue, RenderContext};
use super::geometry::{Arrange, Point, Rect};
use super::scene::{Node, ObjectId};
use crate::tools::image::Image;

type Decoder = fn(Value) -> Result<Box<dyn Annotation>, serde_json::Error>;

fn decode<T: Annotation + DeserializeOwned + 'static>(
    value: Value,
) -> Result<Box<dyn Annotation>, serde_json::Error> {
    Ok(Box::new(serde_json::from_value::<T>(value)?))
}

/// Every object type this version reads, by its serialisation tag.
const TYPES: &[(&str, Decoder)] = &[(Image::TYPE, decode::<Image>)];

/// The tags of all known object types.
pub fn types() -> impl Iterator<Item = &'static str> {
    TYPES.iter().map(|(name, _)| *name)
}

/// Keys of a node that belong to the scene, not to the object.
pub const RESERVED_KEYS: [&str; 2] = ["id", "type"];

/// An object of a type this version does not know (written by a newer minor version).
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownObject {
    pub type_tag: String,
    pub fields: Map<String, Value>,
}

impl UnknownObject {
    pub const TYPE: &'static str = "unknown";
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
}

fn invalid(message: impl std::fmt::Display) -> serde_json::Error {
    <serde_json::Error as serde::de::Error>::custom(message)
}

/// Reads one node of `document.json`.
pub fn node_from_json(value: Value) -> Result<Node, serde_json::Error> {
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
    let Some((_, decoder)) = TYPES.iter().find(|(name, _)| *name == type_tag) else {
        let object = UnknownObject { type_tag, fields };
        return Ok(Node::new(ObjectId(id), Box::new(object)));
    };
    let object = decoder(Value::Object(fields.clone()))
        .map_err(|err| invalid(format!("object {id} ({type_tag}): {err}")))?;
    // Whatever the type does not write back is unknown to this version: keep it aside.
    let written = match object.to_json()? {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let extra = fields
        .into_iter()
        .filter(|(key, _)| !written.contains_key(key))
        .collect();
    let mut node = Node::new(ObjectId(id), object);
    node.extra = extra;
    Ok(node)
}

/// Writes one node for `document.json`.
pub fn node_to_json(node: &Node) -> Result<Value, serde_json::Error> {
    let (type_tag, own) = match node.object.as_any().downcast_ref::<UnknownObject>() {
        Some(unknown) => (unknown.type_tag.clone(), unknown.fields.clone()),
        None => match node.object.to_json()? {
            Value::Object(map) => (node.object.type_name().to_string(), map),
            _ => return Err(invalid("an object must serialise to a JSON object")),
        },
    };
    let mut map = Map::new();
    map.insert("id".into(), Value::from(node.id.0));
    map.insert("type".into(), Value::String(type_tag));
    for (key, value) in own {
        if !RESERVED_KEYS.contains(&key.as_str()) {
            map.insert(key, value);
        }
    }
    for (key, value) in &node.extra {
        if !map.contains_key(key) {
            map.insert(key.clone(), value.clone());
        }
    }
    Ok(Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::geometry::Rect;
    use serde_json::json;

    #[test]
    fn every_registered_type_round_trips_with_its_tag() {
        for name in types() {
            let sample = match name {
                Image::TYPE => Image::new("images/1.png", Rect::new(1.0, 2.0, 30.0, 40.0)),
                other => panic!("add a sample for new type '{other}'"),
            };
            assert_eq!(sample.type_name(), name);
            let node = Node::new(ObjectId(7), Box::new(sample));
            let value = node_to_json(&node).unwrap();
            assert_eq!(value["type"], json!(name));
            assert_eq!(value["id"], json!(7));
            let own = node.object.to_json().unwrap();
            for key in RESERVED_KEYS {
                assert!(own.get(key).is_none(), "{name} must not write '{key}'");
            }
            let back = node_from_json(value.clone()).unwrap();
            assert!(back.extra.is_empty());
            assert_eq!(node_to_json(&back).unwrap(), value);
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
        let node = node_from_json(value.clone()).unwrap();
        assert_eq!(node.extra.get("glow"), Some(&json!({"radius": 4})));
        assert_eq!(node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn unknown_types_are_kept_verbatim() {
        let value = json!({"id": 4, "type": "hologram", "x": 1, "colour": "#FF0000FF"});
        let node = node_from_json(value.clone()).unwrap();
        assert_eq!(node.object.type_name(), UnknownObject::TYPE);
        assert!(node.object.bounds().is_none());
        assert!(!node.object.hit_test(Point::new(1.0, 0.0), 10.0));
        assert_eq!(node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn malformed_nodes_are_rejected() {
        assert!(node_from_json(json!([1, 2])).is_err());
        assert!(node_from_json(json!({"type": "image"})).is_err());
        assert!(node_from_json(json!({"id": 0, "type": "image"})).is_err());
        assert!(node_from_json(json!({"id": 1})).is_err());
        assert!(node_from_json(json!({"id": 1, "type": "image", "x": "left"})).is_err());
    }
}
