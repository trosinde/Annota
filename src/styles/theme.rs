//! Themes (spec style-system: Themes, Theme exchange): 2–8 colours, a heading font and a body
//! font. The built-in themes, the `.annota-theme` file format and its validation.
//!
//! # The `.annota-theme` format
//!
//! A UTF-8 JSON object (schema: `data/schema/annota-theme-1.schema.json`):
//!
//! ```json
//! {
//!   "format": "1.0",
//!   "requires": "0.1",
//!   "name": "Northwind Ferries",
//!   "colours": ["#0B5CAD", "#00A0B0", "#F2A900", "#E4572E", "#2D2D34"],
//!   "fonts": { "heading": "Inter Display", "body": "Inter" }
//! }
//! ```
//!
//! * `format` is `"MAJOR.MINOR"` and `requires` the oldest Annota release that reads it — the
//!   same scheme as `document.json` (spec document-format: Format versions). A newer *minor*
//!   version only adds keys: they are read past, kept, and written back when the theme is saved
//!   or exported again, so a theme passed through an older Annota loses nothing. A newer
//!   *major* version is refused ("unknown schema version"), as is a file without a version.
//! * `colours` are `"#RRGGBB"` (or `"#RRGGBBAA"`); the first is the default colour of every
//!   tool. Fewer than 2 or more than 8 are refused.
//! * `fonts.heading` is used by the text tool, `fonts.body` by callouts. Fonts are names of
//!   font families; a family that is not installed falls back like any text in Annota.
//! * A theme file has no id: themes are told apart by where they are (built in, the system
//!   directory, the user's directory) and their file name, so two people's files never clash.

use std::fmt;

use serde_json::{Map, Value};

use crate::document::archive::FormatVersion;
use crate::document::color::{palette, Rgba};
use crate::tools::text::DEFAULT_FAMILY;

/// The format this version writes.
pub const FORMAT: FormatVersion = FormatVersion { major: 1, minor: 0 };
/// The oldest Annota release that reads [`FORMAT`].
pub const REQUIRES: &str = "0.1";
/// File name extension, without the dot.
pub const EXTENSION: &str = "annota-theme";
/// How many colours a theme has at least and at most.
pub const MIN_COLOURS: usize = 2;
pub const MAX_COLOURS: usize = 8;

/// Id of the default theme ("Annota").
pub const DEFAULT_ID: &str = "annota";
pub const MONOCHROME_ID: &str = "monochrome";

/// Where a theme comes from (the sections of the theme list, mockup 06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Annota and Monochrome.
    BuiltIn,
    /// A file in the system theme directory (`/usr/share/annota/themes`): read-only.
    Managed,
    /// A file in the user's theme directory ("My Themes"); the file's name without extension.
    User(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// `annota`, `monochrome`, `system/<file stem>`, `user/<file stem>`.
    pub id: String,
    pub origin: Origin,
    pub name: String,
    pub colours: Vec<Rgba>,
    pub heading_font: String,
    pub body_font: String,
    /// The minor version of the file it was read from, if newer than [`FORMAT`]'s.
    pub format_minor: u32,
    /// Keys of a newer minor version, written back unchanged.
    pub unknown: Map<String, Value>,
}

/// Why a file is not a theme (the reasons of the "Can't Import Theme" alert, mockup 10).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    #[error("not a JSON file")]
    NotJson,
    #[error("no schema version")]
    NoVersion,
    #[error("unknown schema version {0}")]
    UnknownVersion(String),
    #[error("{0} colours; a theme needs 2–8")]
    ColourCount(usize),
    #[error("invalid colour {0:?}")]
    InvalidColour(String),
    #[error("no name")]
    NoName,
    #[error("no heading and body font")]
    NoFonts,
}

impl Theme {
    /// The default theme: the Annota annotation palette (Coral, Amber, Lime, Teal, Azure,
    /// Violet, Ink — the sidebar's swatches of add-core-annotation-tools) in Adwaita Sans. Its
    /// generated first styles are exactly the tools' built-in defaults.
    pub fn annota() -> Theme {
        Theme::built_in(
            DEFAULT_ID,
            "Annota",
            vec![
                palette::CORAL,
                palette::AMBER,
                palette::LIME,
                palette::TEAL,
                palette::AZURE,
                palette::VIOLET,
                palette::INK,
            ],
        )
    }

    /// Ink, a neutral grey and one signal colour (mockup 06's chips: black, grey, red).
    pub fn monochrome() -> Theme {
        Theme::built_in(
            MONOCHROME_ID,
            "Monochrome",
            vec![palette::INK, Rgba::opaque(0x77, 0x76, 0x7B), palette::CORAL],
        )
    }

    fn built_in(id: &str, name: &str, colours: Vec<Rgba>) -> Theme {
        Theme {
            id: id.to_string(),
            origin: Origin::BuiltIn,
            name: name.to_string(),
            colours,
            heading_font: DEFAULT_FAMILY.to_string(),
            body_font: DEFAULT_FAMILY.to_string(),
            format_minor: FORMAT.minor,
            unknown: Map::new(),
        }
    }

    pub fn built_ins() -> Vec<Theme> {
        vec![Theme::annota(), Theme::monochrome()]
    }

    /// Whether it can be changed and deleted (only the user's own themes can).
    pub fn editable(&self) -> bool {
        matches!(self.origin, Origin::User(_))
    }

    /// Colour `i` of the theme (0-based).
    pub fn colour(&self, i: usize) -> Option<Rgba> {
        self.colours.get(i).copied()
    }

    /// Reads a theme file's text. `id` and `origin` say where it was found.
    pub fn parse(text: &str, id: String, origin: Origin) -> Result<Theme, ThemeError> {
        let value: Value = serde_json::from_str(text).map_err(|_| ThemeError::NotJson)?;
        let Value::Object(mut map) = value else {
            return Err(ThemeError::NotJson);
        };
        let format = match map.remove("format") {
            Some(Value::String(f)) => {
                FormatVersion::parse(&f).ok_or(ThemeError::UnknownVersion(f.clone()))?
            }
            Some(other) => return Err(ThemeError::UnknownVersion(other.to_string())),
            None => return Err(ThemeError::NoVersion),
        };
        if format.major != FORMAT.major {
            return Err(ThemeError::UnknownVersion(format.to_string()));
        }
        map.remove("requires");
        let colours = match map.remove("colours") {
            Some(Value::Array(list)) => list,
            Some(_) => return Err(ThemeError::ColourCount(0)),
            None => Vec::new(),
        };
        // Count first: "It contains 11 colours" is the clearer reason even if one is invalid.
        if !(MIN_COLOURS..=MAX_COLOURS).contains(&colours.len()) {
            return Err(ThemeError::ColourCount(colours.len()));
        }
        let colours = colours
            .iter()
            .map(|c| {
                c.as_str()
                    .and_then(Rgba::parse)
                    .ok_or_else(|| ThemeError::InvalidColour(c.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let name = match map.remove("name") {
            Some(Value::String(n)) if !n.trim().is_empty() => n.trim().to_string(),
            _ => return Err(ThemeError::NoName),
        };
        let (heading_font, body_font) = match map.remove("fonts") {
            Some(Value::Object(mut fonts)) => {
                let mut take = |key: &str| match fonts.remove(key) {
                    Some(Value::String(f)) if !f.trim().is_empty() => Some(f.trim().to_string()),
                    _ => None,
                };
                let (Some(heading), Some(body)) = (take("heading"), take("body")) else {
                    return Err(ThemeError::NoFonts);
                };
                if !fonts.is_empty() {
                    map.insert("fonts".into(), Value::Object(fonts));
                }
                (heading, body)
            }
            _ => return Err(ThemeError::NoFonts),
        };
        Ok(Theme {
            id,
            origin,
            name,
            colours,
            heading_font,
            body_font,
            format_minor: newest_minor(format.minor),
            unknown: map,
        })
    }

    /// The file's text: pretty JSON, keys of a newer minor version kept.
    pub fn to_json(&self) -> String {
        let mut map = self.unknown.clone();
        let mut fonts = match map.remove("fonts") {
            Some(Value::Object(f)) => f,
            _ => Map::new(),
        };
        fonts.insert("heading".into(), Value::String(self.heading_font.clone()));
        fonts.insert("body".into(), Value::String(self.body_font.clone()));
        let version = FormatVersion {
            major: FORMAT.major,
            minor: newest_minor(self.format_minor),
        };
        map.insert("format".into(), Value::String(version.to_string()));
        map.insert("requires".into(), Value::String(REQUIRES.to_string()));
        map.insert("name".into(), Value::String(self.name.clone()));
        map.insert(
            "colours".into(),
            Value::Array(
                self.colours
                    .iter()
                    .map(|c| Value::String(hex(*c)))
                    .collect(),
            ),
        );
        map.insert("fonts".into(), Value::Object(fonts));
        // A fixed key order reads better in a shared file; serde_json's map is sorted, so
        // write the known keys first by hand.
        let mut out = String::from("{\n");
        let order = ["format", "requires", "name", "colours", "fonts"];
        let mut entries: Vec<(String, Value)> = Vec::new();
        for key in order {
            if let Some(v) = map.remove(key) {
                entries.push((key.to_string(), v));
            }
        }
        entries.extend(map);
        for (i, (key, value)) in entries.iter().enumerate() {
            let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".into());
            let text = text.replace('\n', "\n  ");
            let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".into());
            out.push_str(&format!("  {key}: {text}"));
            out.push_str(if i + 1 < entries.len() { ",\n" } else { "\n" });
        }
        out.push_str("}\n");
        out
    }
}

/// `"#RRGGBB"`, with `AA` only when not opaque (theme files are hand-edited too).
pub fn hex(c: Rgba) -> String {
    if c.a == 0xFF {
        format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
    } else {
        c.to_string()
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// The minor version to keep: a newer file's (its data is still there), else this version's.
fn newest_minor(minor: u32) -> u32 {
    if minor > FORMAT.minor {
        minor
    } else {
        FORMAT.minor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Theme, ThemeError> {
        Theme::parse(text, "user/t".into(), Origin::User("t".into()))
    }

    const NORTHWIND: &str = r##"{
        "format": "1.0", "requires": "0.1", "name": "Northwind Ferries",
        "colours": ["#0B5CAD", "#00A0B0", "#F2A900", "#E4572E", "#2D2D34"],
        "fonts": {"heading": "Inter Display", "body": "Inter"}
    }"##;

    #[test]
    fn a_theme_file_round_trips() {
        let theme = parse(NORTHWIND).unwrap();
        assert_eq!(theme.name, "Northwind Ferries");
        assert_eq!(theme.colours.len(), 5);
        assert_eq!(theme.colours[0], Rgba::opaque(0x0B, 0x5C, 0xAD));
        assert_eq!(
            (theme.heading_font.as_str(), theme.body_font.as_str()),
            ("Inter Display", "Inter")
        );
        let text = theme.to_json();
        assert!(text.starts_with("{\n  \"format\": \"1.0\",\n  \"requires\": \"0.1\",\n  \"name\""));
        assert!(text.contains("\"#0B5CAD\""), "{text}");
        assert_eq!(parse(&text).unwrap(), theme);
    }

    #[test]
    fn a_newer_minor_version_keeps_what_it_added() {
        let text = r##"{"format": "1.3", "requires": "0.4", "name": "Brand",
            "colours": ["#112233", "#445566"],
            "fonts": {"heading": "A", "body": "B", "mono": "C"},
            "logo": {"svg": "<svg/>"}}"##;
        let theme = parse(text).unwrap();
        assert_eq!(theme.format_minor, 3);
        let written: Value = serde_json::from_str(&theme.to_json()).unwrap();
        assert_eq!(written["format"], "1.3");
        assert_eq!(written["logo"]["svg"], "<svg/>");
        assert_eq!(written["fonts"]["mono"], "C");
        assert_eq!(written["fonts"]["heading"], "A");
    }

    #[test]
    fn every_import_error_has_its_reason() {
        assert_eq!(parse("PNG\u{1}garbage"), Err(ThemeError::NotJson));
        assert_eq!(parse("[1, 2]"), Err(ThemeError::NotJson));
        assert_eq!(parse(r#"{"name": "x"}"#), Err(ThemeError::NoVersion));
        assert_eq!(
            parse(r#"{"format": "2.0", "name": "x"}"#),
            Err(ThemeError::UnknownVersion("2.0".into()))
        );
        assert_eq!(
            parse(r#"{"format": "one", "name": "x"}"#),
            Err(ThemeError::UnknownVersion("one".into()))
        );
        let eleven: Vec<String> = (0..11).map(|i| format!("\"#0000{i:02}\"")).collect();
        let text = format!(
            r#"{{"format": "1.0", "name": "x", "colours": [{}], "fonts": {{"heading": "a", "body": "b"}}}}"#,
            eleven.join(",")
        );
        assert_eq!(parse(&text), Err(ThemeError::ColourCount(11)));
        assert_eq!(
            parse(
                r##"{"format": "1.0", "name": "x", "colours": ["#000000"], "fonts": {"heading": "a", "body": "b"}}"##
            ),
            Err(ThemeError::ColourCount(1))
        );
        assert_eq!(
            parse(
                r##"{"format": "1.0", "name": "x", "colours": ["#000000", "red"], "fonts": {"heading": "a", "body": "b"}}"##
            ),
            Err(ThemeError::InvalidColour("\"red\"".into()))
        );
        assert_eq!(
            parse(
                r##"{"format": "1.0", "name": " ", "colours": ["#000000", "#FFFFFF"], "fonts": {"heading": "a", "body": "b"}}"##
            ),
            Err(ThemeError::NoName)
        );
        assert_eq!(
            parse(
                r##"{"format": "1.0", "name": "x", "colours": ["#000000", "#FFFFFF"], "fonts": {"heading": "a"}}"##
            ),
            Err(ThemeError::NoFonts)
        );
    }

    #[test]
    fn schema_matches_what_is_written() {
        // Guards data/schema/annota-theme-1.schema.json against drifting from the code (full
        // validation with Python `jsonschema`, see design.md).
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/schema/annota-theme-1.schema.json"
        )))
        .unwrap();
        let written: Value = serde_json::from_str(&Theme::annota().to_json()).unwrap();
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&written), keys(&schema["properties"]));
        assert_eq!(
            keys(&written["fonts"]),
            keys(&schema["properties"]["fonts"]["properties"])
        );
        let colours = &schema["properties"]["colours"];
        assert_eq!(colours["minItems"], MIN_COLOURS);
        assert_eq!(colours["maxItems"], MAX_COLOURS);
        for key in schema["required"].as_array().unwrap() {
            assert!(written.get(key.as_str().unwrap()).is_some(), "{key}");
        }
    }

    #[test]
    fn built_in_themes_are_valid_and_read_only() {
        for theme in Theme::built_ins() {
            assert!((MIN_COLOURS..=MAX_COLOURS).contains(&theme.colours.len()));
            assert!(!theme.editable());
            let back = Theme::parse(&theme.to_json(), theme.id.clone(), Origin::BuiltIn).unwrap();
            assert_eq!(back, theme);
        }
        assert_eq!(Theme::annota().colours[0], palette::CORAL);
    }
}
