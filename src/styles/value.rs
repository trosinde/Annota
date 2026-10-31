//! What a quick style holds: a tool's style properties (spec style-system: Quick styles), and
//! how they are written to JSON.
//!
//! A quick style is a named bundle of the same `properties`/`set_property` values the sidebar
//! edits — the tool defaults of `editing::tool::Toolbox`, minus what is not *style*: a text's
//! words and a marker's own value ([`Toolbox::INTERNAL`]), the kind of shape (a polygon is
//! drawn vertex by vertex, so a style that switched to polygons would change how the tool
//! draws) and the way a step sequence counts (it belongs to the whole sequence, not to one
//! marker). Applying a style is therefore just setting these properties, and a style is only
//! ever made of values a type accepted.
//!
//! In JSON a value is written by its kind: numbers and booleans as such, colours as
//! `"#RRGGBBAA"`, texts and choices as strings, a shadow as the document format's shadow object.
//! Reading needs to know the kind, which the tool's prototype tells ([`decode`]).

use serde_json::{Map, Value};

use crate::document::annotation::{Property, PropertyValue};
use crate::document::color::Rgba;
use crate::document::style::Shadow;
use crate::editing::tool::{Tool, Toolbox};

/// Properties of the tool defaults that are not part of a quick style (module documentation).
pub const NOT_STYLE: [(Tool, &str); 2] = [(Tool::Shape, "shape"), (Tool::Step, "counting")];

/// The tools that have quick styles: every drawing tool.
pub const TOOLS: [Tool; 9] = [
    Tool::Arrow,
    Tool::Line,
    Tool::Pen,
    Tool::Shape,
    Tool::Text,
    Tool::Callout,
    Tool::Step,
    Tool::Highlighter,
    Tool::Redact,
];

/// The tool that makes objects of `type_name` (`"arrow"` → Arrow, `"redaction"` → Redact).
pub fn tool_of_type(type_name: &str) -> Option<Tool> {
    TOOLS
        .into_iter()
        .find(|t| t.object_type() == Some(type_name))
}

/// The keys a quick style of `tool` sets, in the sidebar's order.
pub fn style_keys(tool: Tool) -> Vec<&'static str> {
    Toolbox::default()
        .properties(tool)
        .into_iter()
        .map(|p| p.key)
        .filter(|key| !NOT_STYLE.contains(&(tool, *key)))
        .collect()
}

/// The style part of `values` (a panel's values, a prototype's properties).
pub fn style_of(tool: Tool, values: &[Property]) -> Vec<Property> {
    let keys = style_keys(tool);
    values
        .iter()
        .filter(|p| keys.contains(&p.key))
        .cloned()
        .collect()
}

/// Whether `current` has every value of `style` (the style is the one in use: its tile is
/// selected, "Modified" is not shown, there is nothing to save).
pub fn matches(style: &[Property], current: &[Property]) -> bool {
    !style.is_empty()
        && style
            .iter()
            .all(|s| current.iter().any(|c| c.key == s.key && c.value == s.value))
}

/// A value as written to JSON.
pub fn encode(value: &PropertyValue) -> Value {
    match value {
        PropertyValue::Bool(b) => Value::Bool(*b),
        PropertyValue::Number(n) => {
            serde_json::Number::from_f64(*n).map_or(Value::Null, Value::Number)
        }
        PropertyValue::Color(c) => Value::String(c.to_string()),
        PropertyValue::Text(t) | PropertyValue::Choice(t) => Value::String(t.clone()),
        PropertyValue::Shadow(s) => serde_json::to_value(s).unwrap_or(Value::Null),
    }
}

/// Reads `json` as a value of the same kind as `like`; `None` if it is not one.
pub fn decode(like: &PropertyValue, json: &Value) -> Option<PropertyValue> {
    Some(match like {
        PropertyValue::Bool(_) => PropertyValue::Bool(json.as_bool()?),
        PropertyValue::Number(_) => PropertyValue::Number(json.as_f64().filter(|n| n.is_finite())?),
        PropertyValue::Color(_) => PropertyValue::Color(Rgba::parse(json.as_str()?)?),
        PropertyValue::Text(_) => PropertyValue::Text(json.as_str()?.to_string()),
        PropertyValue::Choice(_) => PropertyValue::Choice(json.as_str()?.to_string()),
        PropertyValue::Shadow(_) => {
            PropertyValue::Shadow(serde_json::from_value::<Shadow>(json.clone()).ok()?)
        }
    })
}

/// `values` as a JSON object (`{"color": "#FF5A4EFF", "width": 8, …}`).
pub fn encode_all(values: &[Property]) -> Map<String, Value> {
    values
        .iter()
        .map(|p| (p.key.to_string(), encode(&p.value)))
        .collect()
}

/// Reads the style properties of `tool` from `json`. Values a type would refuse are left out
/// (applying the style leaves those properties as they are); keys that are not style
/// properties of the tool — from a newer Annota, say — are returned separately so they can be
/// written back unchanged.
pub fn decode_all(tool: Tool, json: &Map<String, Value>) -> (Vec<Property>, Map<String, Value>) {
    let mut prototype = Toolbox::default();
    let defaults = prototype.properties(tool);
    let keys = style_keys(tool);
    let mut values = Vec::new();
    let mut unknown = Map::new();
    for (key, raw) in json {
        let known = defaults
            .iter()
            .find(|p| p.key == key.as_str() && keys.contains(&p.key));
        let value = known.and_then(|p| decode(&p.value, raw).map(|v| (p.key, v)));
        match value {
            // Only values the type accepts.
            Some((key, v)) if prototype.set_property(tool, key, &v).is_ok() => {
                values.push(Property::new(key, v));
            }
            _ => {
                unknown.insert(key.clone(), raw.clone());
            }
        }
    }
    // The sidebar's order, whatever order the file had.
    values.sort_by_key(|p| keys.iter().position(|k| *k == p.key));
    (values, unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::color::palette;

    #[test]
    fn style_keys_leave_out_what_is_not_style() {
        assert!(!style_keys(Tool::Text).contains(&"text"));
        assert!(!style_keys(Tool::Step).contains(&"value"));
        assert!(!style_keys(Tool::Step).contains(&"counting"));
        assert!(!style_keys(Tool::Shape).contains(&"shape"));
        assert!(
            style_keys(Tool::Callout).contains(&"shape"),
            "callout shape is style"
        );
        assert_eq!(style_keys(Tool::Highlighter), vec!["color", "opacity"]);
        for tool in TOOLS {
            assert!(!style_keys(tool).is_empty(), "{tool:?}");
            assert_eq!(tool_of_type(tool.object_type().unwrap()), Some(tool));
        }
        assert_eq!(tool_of_type("image"), None);
    }

    #[test]
    fn every_value_kind_round_trips() {
        for tool in TOOLS {
            let values = style_of(tool, &Toolbox::default().properties(tool));
            let json = encode_all(&values);
            let (back, unknown) = decode_all(tool, &json);
            assert_eq!(back, values, "{tool:?}");
            assert!(unknown.is_empty());
        }
    }

    #[test]
    fn unknown_and_refused_values_are_kept_aside() {
        let json: Map<String, Value> = serde_json::from_value(serde_json::json!({
            "width": 400,             // out of range: refused by the arrow
            "color": "#14B8A6FF",
            "glow": {"radius": 3},    // a property of a newer Annota
            "end_style": "triangle",
        }))
        .unwrap();
        let (values, unknown) = decode_all(Tool::Arrow, &json);
        assert_eq!(
            values,
            vec![
                Property::new("color", PropertyValue::Color(palette::TEAL)),
                Property::new("end_style", PropertyValue::Choice("triangle".into())),
            ]
        );
        assert_eq!(unknown.len(), 2);
        assert!(unknown.contains_key("glow") && unknown.contains_key("width"));
    }

    #[test]
    fn a_style_matches_values_that_contain_it() {
        let current = style_of(Tool::Arrow, &Toolbox::default().properties(Tool::Arrow));
        assert!(matches(&current, &current));
        let mut teal = current.clone();
        teal[0].value = PropertyValue::Color(palette::TEAL);
        assert!(!matches(&teal, &current));
        assert!(!matches(&[], &current), "an empty style matches nothing");
    }
}
