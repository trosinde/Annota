//! Forward compatibility at every depth (spec document-format: Forward compatibility).
//!
//! A file of a newer minor format version may contain keys this version does not know — at the
//! top level, inside a section (`canvas.background.pattern`) or inside an object
//! (`objects[0].shadow.spread`). When a node or section is read, its original JSON is compared
//! with what this version writes for the typed value it read ([`diff`]); whatever this version
//! would not write is kept as a [`Residual`]. When the document is written, the residual is
//! merged back into the freshly serialised JSON ([`merge`]): known keys always reflect the
//! current state, unknown keys survive untouched.
//!
//! Rules:
//! * **Objects** are compared key by key, recursively.
//! * **Arrays** are compared element by element if both have the same length; on write, the
//!   residual is only applied while the array still has that length (otherwise the array was
//!   rewritten by this version and the old elements' extra keys no longer belong anywhere).
//! * **Tagged values.** If an object has a `"type"` key, its residual remembers that tag. If the
//!   value is later written with a *different* `"type"` (e.g. the canvas background changed from
//!   an unknown `"gradient"` to `"color"`), the residual of that object is dropped: the old
//!   variant's extra keys do not apply to the new variant.
//! * Scalars are owned by this version: a known key always gets the current value.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// What this version did not write back of one JSON value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Residual {
    Object {
        /// The value's `"type"` when it was read, if it had one.
        tag: Option<Value>,
        /// Keys this version does not write, with their original values.
        keys: Map<String, Value>,
        /// Residuals inside keys this version does write.
        nested: BTreeMap<String, Residual>,
    },
    /// Per element, for an array of unchanged length.
    Array(Vec<Option<Residual>>),
}

/// The part of `original` (as read) that `written` (what this version writes for the same
/// data) does not reproduce; `None` if nothing is lost.
pub(crate) fn diff(original: &Value, written: &Value) -> Option<Residual> {
    match (original, written) {
        (Value::Object(o), Value::Object(w)) => {
            let mut keys = Map::new();
            let mut nested = BTreeMap::new();
            for (key, value) in o {
                match w.get(key) {
                    None => {
                        keys.insert(key.clone(), value.clone());
                    }
                    Some(w_value) => {
                        if let Some(r) = diff(value, w_value) {
                            nested.insert(key.clone(), r);
                        }
                    }
                }
            }
            if keys.is_empty() && nested.is_empty() {
                return None;
            }
            Some(Residual::Object {
                tag: o.get("type").cloned(),
                keys,
                nested,
            })
        }
        (Value::Array(o), Value::Array(w)) if o.len() == w.len() => {
            let items: Vec<Option<Residual>> = o.iter().zip(w).map(|(o, w)| diff(o, w)).collect();
            items
                .iter()
                .any(Option::is_some)
                .then_some(Residual::Array(items))
        }
        _ => None,
    }
}

/// Puts the keys of `residual` back into `target` where `target` does not have them.
pub(crate) fn merge(target: &mut Value, residual: &Residual) {
    match (target, residual) {
        (Value::Object(t), Residual::Object { tag, keys, nested }) => {
            if let (Some(old), Some(new)) = (tag, t.get("type")) {
                if old != new {
                    return;
                }
            }
            for (key, value) in keys {
                if !t.contains_key(key) {
                    t.insert(key.clone(), value.clone());
                }
            }
            for (key, r) in nested {
                if let Some(value) = t.get_mut(key) {
                    merge(value, r);
                }
            }
        }
        (Value::Array(t), Residual::Array(items)) if t.len() == items.len() => {
            for (value, r) in t.iter_mut().zip(items) {
                if let Some(r) = r {
                    merge(value, r);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn round_trip(original: Value, written: Value) -> Value {
        let mut out = written.clone();
        if let Some(r) = diff(&original, &written) {
            merge(&mut out, &r);
        }
        out
    }

    #[test]
    fn unknown_keys_survive_at_every_depth() {
        let original = json!({
            "a": 1, "new": true,
            "shadow": {"blur": 3, "spread": 2, "deep": {"x": 1, "y": 2}},
            "list": [{"p": 1, "q": 9}, {"p": 2}]
        });
        let written = json!({
            "a": 1,
            "shadow": {"blur": 3, "deep": {"x": 1}},
            "list": [{"p": 1}, {"p": 2}]
        });
        assert_eq!(round_trip(original.clone(), written), original);
    }

    #[test]
    fn known_keys_take_the_current_value() {
        let original = json!({"x": 1, "shadow": {"blur": 3, "spread": 2}});
        let r = diff(&original, &json!({"x": 1, "shadow": {"blur": 3}})).unwrap();
        // Later the object moved and its blur changed.
        let mut now = json!({"x": 50, "shadow": {"blur": 8}});
        merge(&mut now, &r);
        assert_eq!(now, json!({"x": 50, "shadow": {"blur": 8, "spread": 2}}));
    }

    #[test]
    fn nothing_lost_means_no_residual() {
        let v = json!({"a": [1, 2], "b": {"c": "d"}});
        assert_eq!(diff(&v, &v), None);
        // Scalars that differ in spelling only ("#fff" read, "#FFFFFFFF" written) are known.
        assert_eq!(
            diff(&json!({"c": "#fff"}), &json!({"c": "#FFFFFFFF"})),
            None
        );
    }

    #[test]
    fn changed_tag_drops_the_old_variants_keys() {
        // Unknown variant: this version writes nothing for it, so everything is residual.
        let original = json!({"type": "gradient", "stops": ["#000", "#fff"]});
        let r = diff(&original, &json!({})).unwrap();
        let mut unchanged = json!({});
        merge(&mut unchanged, &r);
        assert_eq!(unchanged, original);
        let mut changed = json!({"type": "color", "color": "#FFFFFFFF"});
        merge(&mut changed, &r);
        assert_eq!(changed, json!({"type": "color", "color": "#FFFFFFFF"}));
        // Same tag: extra keys of the variant stay.
        let original = json!({"type": "color", "color": "#fff", "pattern": "dots"});
        let r = diff(&original, &json!({"type": "color", "color": "#FFFFFFFF"})).unwrap();
        let mut same = json!({"type": "color", "color": "#000000FF"});
        merge(&mut same, &r);
        assert_eq!(same["pattern"], json!("dots"));
    }

    #[test]
    fn rewritten_arrays_drop_element_residuals() {
        let original = json!({"pts": [{"x": 1, "w": 2}, {"x": 3, "w": 4}]});
        let r = diff(&original, &json!({"pts": [{"x": 1}, {"x": 3}]})).unwrap();
        let mut longer = json!({"pts": [{"x": 1}, {"x": 3}, {"x": 5}]});
        merge(&mut longer, &r);
        assert_eq!(longer, json!({"pts": [{"x": 1}, {"x": 3}, {"x": 5}]}));
    }
}
