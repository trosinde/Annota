//! The user's quick styles, favourites and current theme (spec style-system: Quick styles,
//! Favourites), and their file `styles.json`.
//!
//! The library is application state, not part of a document: saving, renaming, moving,
//! starring and deleting styles are not undo steps of the document's history (they would be
//! undone with the document's edits and lost with it). The sidebar offers a toast with Undo
//! instead, made from the inverse operation ([`Library::remove`] → [`Library::restore`]).
//!
//! # `styles.json`
//!
//! ```json
//! {
//!   "format": "1.0",
//!   "theme": "annota",
//!   "styles": [
//!     {"id": "s1a2b3", "tool": "arrow", "name": "Teal bold",
//!      "properties": {"color": "#14B8A6FF", "width": 8, "shadow": {…}, …}}
//!   ],
//!   "favourites": [
//!     {"style": "s1a2b3"},
//!     {"theme": "annota", "tool": "callout", "slot": 0}
//!   ]
//! }
//! ```
//!
//! `tool` is the type tag of the objects the tool makes (`document.json`'s `"type"`), the
//! order of `styles` is the order of each tool's "My Styles". Forward compatibility follows
//! `document.json`'s rules: unknown top-level keys, styles of unknown tools, unknown style
//! properties and favourites of an unknown kind are kept and written back; a newer *major*
//! version is not read and — so it is not destroyed — never overwritten
//! ([`Library::read_only`]). A file that is not a library at all is moved aside by the store
//! (`styles.json.broken`) rather than lost.

use serde_json::{Map, Value};

use crate::document::annotation::Property;
use crate::document::archive::FormatVersion;
use crate::editing::tool::Tool;

use super::theme::DEFAULT_ID;
use super::value;

pub const FORMAT: FormatVersion = FormatVersion { major: 1, minor: 0 };
pub const FILE_NAME: &str = "styles.json";

/// One user style ("My Styles").
#[derive(Clone, Debug, PartialEq)]
pub struct QuickStyle {
    pub id: String,
    pub tool: Tool,
    pub name: String,
    pub values: Vec<Property>,
    /// Properties this version does not know or would refuse, written back unchanged.
    pub unknown: Map<String, Value>,
}

/// A style the user can star: one of theirs, or a slot of a theme's generated styles.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StyleRef {
    User(String),
    Theme {
        theme: String,
        tool: Tool,
        slot: usize,
    },
}

/// A favourite as stored: one this version understands, or one it keeps as it was.
#[derive(Clone, Debug, PartialEq)]
enum Favourite {
    Known(StyleRef),
    Unknown(Value),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Library {
    styles: Vec<QuickStyle>,
    /// Styles of tools this version does not know, in file order (written after the others).
    unknown_styles: Vec<Value>,
    favourites: Vec<Favourite>,
    /// The current theme's id.
    pub theme: String,
    unknown: Map<String, Value>,
    format_minor: u32,
    /// Read from a newer major version: shown empty and never saved.
    pub read_only: bool,
    next_id: u64,
}

impl Default for Library {
    fn default() -> Self {
        Library {
            styles: Vec::new(),
            unknown_styles: Vec::new(),
            favourites: Vec::new(),
            theme: DEFAULT_ID.to_string(),
            unknown: Map::new(),
            format_minor: FORMAT.minor,
            read_only: false,
            next_id: 1,
        }
    }
}

/// Why `styles.json` could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unreadable {
    /// Not JSON, or not a library (no format version).
    #[error("not a quick style library")]
    Broken,
    #[error("written by a newer Annota (format {0})")]
    Newer(FormatVersion),
}

/// What [`Library::remove`] took out, to put it back.
#[derive(Clone, Debug, PartialEq)]
pub struct Removed {
    pub index: usize,
    pub style: QuickStyle,
    pub favourite: bool,
}

impl Library {
    /// The user styles of `tool`, in their order.
    pub fn styles(&self, tool: Tool) -> Vec<&QuickStyle> {
        self.styles.iter().filter(|s| s.tool == tool).collect()
    }

    pub fn get(&self, id: &str) -> Option<&QuickStyle> {
        self.styles.iter().find(|s| s.id == id)
    }

    fn fresh_id(&mut self) -> String {
        loop {
            let id = format!("s{}", self.next_id);
            self.next_id += 1;
            let taken = self.get(&id).is_some()
                || self
                    .unknown_styles
                    .iter()
                    .any(|raw| raw.get("id").and_then(Value::as_str) == Some(id.as_str()));
            if !taken {
                return id;
            }
        }
    }

    /// `base`, or `base 2`, `base 3` … — a name no style of `tool` has yet.
    pub fn unique_name(&self, tool: Tool, base: &str) -> String {
        let taken = |name: &str| self.styles(tool).iter().any(|s| s.name == name);
        let base = base.trim();
        if !taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !taken(name))
            .unwrap_or_else(|| base.to_string())
    }

    /// Adds a style at the end of `tool`'s styles (scenario "Save style"); returns its id.
    pub fn add(&mut self, tool: Tool, name: &str, values: Vec<Property>) -> String {
        let id = self.fresh_id();
        self.styles.push(QuickStyle {
            id: id.clone(),
            tool,
            name: name.trim().to_string(),
            values: value::style_of(tool, &values),
            unknown: Map::new(),
        });
        id
    }

    pub fn rename(&mut self, id: &str, name: &str) -> bool {
        let name = name.trim();
        match self.styles.iter_mut().find(|s| s.id == id) {
            Some(s) if !name.is_empty() && s.name != name => {
                s.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Replaces a style's properties ("Update from Current Properties").
    pub fn update(&mut self, id: &str, values: &[Property]) -> bool {
        match self.styles.iter_mut().find(|s| s.id == id) {
            Some(s) => {
                let values = value::style_of(s.tool, values);
                let changed = s.values != values;
                s.values = values;
                changed
            }
            None => false,
        }
    }

    /// Where the style is among its tool's styles, and how many that tool has.
    pub fn position(&self, id: &str) -> Option<(usize, usize)> {
        let tool = self.get(id)?.tool;
        let mine = self.styles(tool);
        Some((mine.iter().position(|s| s.id == id)?, mine.len()))
    }

    /// Whether the style can move one place left (`-1`) or right (`1`) among its tool's.
    pub fn can_move(&self, id: &str, delta: isize) -> bool {
        self.position(id)
            .is_some_and(|(at, len)| at.checked_add_signed(delta).is_some_and(|to| to < len))
    }

    /// Moves the style one place left (`-1`) or right (`1`) among its tool's styles (Move
    /// Left/Right, Ctrl+←/→).
    pub fn move_by(&mut self, id: &str, delta: isize) -> bool {
        if !self.can_move(id, delta) {
            return false;
        }
        let Some(tool) = self.get(id).map(|s| s.tool) else {
            return false;
        };
        let indices: Vec<usize> = (0..self.styles.len())
            .filter(|&i| self.styles[i].tool == tool)
            .collect();
        let Some(at) = indices.iter().position(|&i| self.styles[i].id == id) else {
            return false;
        };
        let Some(to) = at.checked_add_signed(delta) else {
            return false;
        };
        self.styles.swap(indices[at], indices[to]);
        true
    }

    /// Deletes a user style (and its star); [`Library::restore`] puts it back.
    pub fn remove(&mut self, id: &str) -> Option<Removed> {
        let index = self.styles.iter().position(|s| s.id == id)?;
        let style = self.styles.remove(index);
        let star = StyleRef::User(id.to_string());
        let favourite = self.is_favourite(&star);
        self.set_favourite(&star, false);
        Some(Removed {
            index,
            style,
            favourite,
        })
    }

    /// Undoes [`Library::remove`] (the toast's Undo).
    pub fn restore(&mut self, removed: Removed) -> bool {
        if self.get(&removed.style.id).is_some() {
            return false;
        }
        let star = StyleRef::User(removed.style.id.clone());
        let index = removed.index.min(self.styles.len());
        self.styles.insert(index, removed.style);
        if removed.favourite {
            self.set_favourite(&star, true);
        }
        true
    }

    pub fn is_favourite(&self, style: &StyleRef) -> bool {
        self.favourites
            .iter()
            .any(|f| matches!(f, Favourite::Known(r) if r == style))
    }

    /// Stars or unstars a style; returns whether that changed anything.
    pub fn set_favourite(&mut self, style: &StyleRef, on: bool) -> bool {
        let was = self.is_favourite(style);
        if on && !was {
            if let StyleRef::User(id) = style {
                if self.get(id).is_none() {
                    return false;
                }
            }
            self.favourites.push(Favourite::Known(style.clone()));
        } else if !on && was {
            self.favourites
                .retain(|f| !matches!(f, Favourite::Known(r) if r == style));
        }
        on != was
    }

    /// The starred styles, in the order they were starred.
    pub fn favourites(&self) -> Vec<&StyleRef> {
        self.favourites
            .iter()
            .filter_map(|f| match f {
                Favourite::Known(r) => Some(r),
                Favourite::Unknown(_) => None,
            })
            .collect()
    }

    // ---- file ----

    /// Reads `styles.json`'s text.
    pub fn parse(text: &str) -> Result<Library, Unreadable> {
        let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(text) else {
            return Err(Unreadable::Broken);
        };
        let format = map
            .remove("format")
            .and_then(|f| f.as_str().and_then(FormatVersion::parse))
            .ok_or(Unreadable::Broken)?;
        if format.major != FORMAT.major {
            return Err(Unreadable::Newer(format));
        }
        let mut library = Library {
            format_minor: newest_minor(format.minor),
            ..Library::default()
        };
        if let Some(Value::String(theme)) = map.remove("theme") {
            library.theme = theme;
        }
        if let Some(Value::Array(styles)) = map.remove("styles") {
            for raw in styles {
                match read_style(&raw) {
                    Some(style) if library.get(&style.id).is_none() => library.styles.push(style),
                    _ => library.unknown_styles.push(raw),
                }
            }
        }
        if let Some(Value::Array(favourites)) = map.remove("favourites") {
            for raw in favourites {
                library.favourites.push(match read_favourite(&raw) {
                    Some(r) => Favourite::Known(r),
                    None => Favourite::Unknown(raw),
                });
            }
        }
        library.unknown = map;
        Ok(library)
    }

    /// The library to use for a file of a newer major version: empty, and never saved, so the
    /// newer file stays as it is.
    pub fn read_only() -> Library {
        Library {
            read_only: true,
            ..Library::default()
        }
    }

    pub fn to_json(&self) -> String {
        let mut map = self.unknown.clone();
        let version = FormatVersion {
            major: FORMAT.major,
            minor: self.format_minor,
        };
        map.insert("format".into(), Value::String(version.to_string()));
        map.insert("theme".into(), Value::String(self.theme.clone()));
        let mut styles: Vec<Value> = self.styles.iter().map(write_style).collect();
        styles.extend(self.unknown_styles.iter().cloned());
        map.insert("styles".into(), Value::Array(styles));
        map.insert(
            "favourites".into(),
            Value::Array(
                self.favourites
                    .iter()
                    .map(|f| match f {
                        Favourite::Known(r) => write_favourite(r),
                        Favourite::Unknown(v) => v.clone(),
                    })
                    .collect(),
            ),
        );
        serde_json::to_string_pretty(&Value::Object(map)).unwrap_or_default()
    }
}

fn tool_tag(tool: Tool) -> &'static str {
    tool.object_type().unwrap_or("select")
}

fn read_style(raw: &Value) -> Option<QuickStyle> {
    let id = raw.get("id")?.as_str()?.to_string();
    let tool = value::tool_of_type(raw.get("tool")?.as_str()?)?;
    let name = raw.get("name")?.as_str()?.to_string();
    let properties = raw.get("properties")?.as_object()?;
    let (values, unknown) = value::decode_all(tool, properties);
    Some(QuickStyle {
        id,
        tool,
        name,
        values,
        unknown,
    })
}

fn write_style(style: &QuickStyle) -> Value {
    let mut properties = style.unknown.clone();
    properties.extend(value::encode_all(&style.values));
    serde_json::json!({
        "id": style.id,
        "tool": tool_tag(style.tool),
        "name": style.name,
        "properties": properties,
    })
}

fn read_favourite(raw: &Value) -> Option<StyleRef> {
    if let Some(id) = raw.get("style") {
        return Some(StyleRef::User(id.as_str()?.to_string()));
    }
    Some(StyleRef::Theme {
        theme: raw.get("theme")?.as_str()?.to_string(),
        tool: value::tool_of_type(raw.get("tool")?.as_str()?)?,
        slot: usize::try_from(raw.get("slot")?.as_u64()?)
            .ok()
            .filter(|s| *s < super::generate::SLOTS)?,
    })
}

fn write_favourite(r: &StyleRef) -> Value {
    match r {
        StyleRef::User(id) => serde_json::json!({ "style": id }),
        StyleRef::Theme { theme, tool, slot } => {
            serde_json::json!({ "theme": theme, "tool": tool_tag(*tool), "slot": slot })
        }
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
    use crate::document::annotation::PropertyValue;
    use crate::document::color::palette;
    use crate::editing::tool::Toolbox;

    fn teal_bold() -> Vec<Property> {
        let mut values = Toolbox::default().properties(Tool::Arrow);
        for p in &mut values {
            match p.key {
                "color" => p.value = PropertyValue::Color(palette::TEAL),
                "width" => p.value = PropertyValue::Number(8.0),
                _ => {}
            }
        }
        values
    }

    fn names(lib: &Library, tool: Tool) -> Vec<String> {
        lib.styles(tool).iter().map(|s| s.name.clone()).collect()
    }

    #[test]
    fn a_saved_style_goes_to_the_end_of_its_tool() {
        // Scenario "Save style".
        let mut lib = Library::default();
        lib.add(
            Tool::Arrow,
            "Coral",
            Toolbox::default().properties(Tool::Arrow),
        );
        lib.add(
            Tool::Callout,
            "Note",
            Toolbox::default().properties(Tool::Callout),
        );
        let id = lib.add(Tool::Arrow, " Teal bold ", teal_bold());
        assert_eq!(names(&lib, Tool::Arrow), ["Coral", "Teal bold"]);
        let style = lib.get(&id).unwrap();
        assert!(value::matches(&style.values, &teal_bold()));
        assert!(
            style.values.iter().all(|p| p.key != "text"),
            "only style keys"
        );
        assert_eq!(lib.unique_name(Tool::Arrow, "Teal bold"), "Teal bold 2");
        assert_eq!(lib.unique_name(Tool::Callout, "Teal bold"), "Teal bold");
    }

    #[test]
    fn rename_move_update_delete_and_undo() {
        let mut lib = Library::default();
        let a = lib.add(Tool::Arrow, "A", teal_bold());
        let n = lib.add(Tool::Pen, "Pen", Toolbox::default().properties(Tool::Pen));
        let b = lib.add(Tool::Arrow, "B", teal_bold());
        let c = lib.add(Tool::Arrow, "C", teal_bold());
        assert!(lib.rename(&b, "Bee"));
        assert!(!lib.rename(&b, "  "), "no empty names");
        assert!(!lib.can_move(&a, -1) && lib.can_move(&a, 1));
        assert!(!lib.can_move(&c, 1) && lib.can_move(&c, -1));
        assert!(lib.move_by(&c, -1));
        assert_eq!(names(&lib, Tool::Arrow), ["A", "C", "Bee"]);
        assert!(!lib.move_by(&a, -1), "disabled at the ends");
        assert_eq!(names(&lib, Tool::Pen), ["Pen"], "other tools stay");
        assert!(lib.update(&a, &Toolbox::default().properties(Tool::Arrow)));
        assert!(!lib.update(&a, &Toolbox::default().properties(Tool::Arrow)));
        let star = StyleRef::User(c.clone());
        assert!(lib.set_favourite(&star, true));
        let removed = lib.remove(&c).unwrap();
        assert!(removed.favourite && !lib.is_favourite(&star));
        assert_eq!(names(&lib, Tool::Arrow), ["A", "Bee"]);
        assert!(lib.restore(removed.clone()));
        assert!(!lib.restore(removed), "only once");
        assert_eq!(names(&lib, Tool::Arrow), ["A", "C", "Bee"]);
        assert!(lib.is_favourite(&star));
        assert!(lib.get(&n).is_some());
    }

    #[test]
    fn favourites_of_user_and_theme_styles() {
        let mut lib = Library::default();
        let id = lib.add(Tool::Arrow, "A", teal_bold());
        let theme = StyleRef::Theme {
            theme: "annota".into(),
            tool: Tool::Callout,
            slot: 0,
        };
        assert!(lib.set_favourite(&StyleRef::User(id.clone()), true));
        assert!(lib.set_favourite(&theme, true));
        assert!(!lib.set_favourite(&theme, true), "already");
        assert!(!lib.set_favourite(&StyleRef::User("nope".into()), true));
        assert_eq!(lib.favourites().len(), 2);
        assert!(lib.set_favourite(&theme, false));
        assert_eq!(lib.favourites(), vec![&StyleRef::User(id)]);
    }

    #[test]
    fn the_file_round_trips_and_keeps_what_it_does_not_know() {
        let mut lib = Library::default();
        let id = lib.add(Tool::Arrow, "Teal bold", teal_bold());
        lib.set_favourite(&StyleRef::User(id), true);
        lib.set_favourite(
            &StyleRef::Theme {
                theme: "user/brand".into(),
                tool: Tool::Step,
                slot: 3,
            },
            true,
        );
        lib.theme = "user/brand".into();
        let back = Library::parse(&lib.to_json()).unwrap();
        assert_eq!(back.styles, lib.styles);
        assert_eq!(back.favourites, lib.favourites);
        assert_eq!(back.theme, "user/brand");
        assert!(!back.read_only);

        let mut newer: Value = serde_json::from_str(&lib.to_json()).unwrap();
        newer["format"] = "1.4".into();
        newer["sync"] = serde_json::json!({"enabled": false});
        newer["styles"][0]["properties"]["glow"] = 3.into();
        newer["styles"].as_array_mut().unwrap().push(
            serde_json::json!({"id": "x", "tool": "magnifier", "name": "Loupe", "properties": {}}),
        );
        newer["favourites"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"template": "t1"}));
        let read = Library::parse(&newer.to_string()).unwrap();
        assert_eq!(read.styles(Tool::Arrow).len(), 1);
        assert_eq!(read.favourites().len(), 2);
        let written: Value = serde_json::from_str(&read.to_json()).unwrap();
        assert_eq!(written["format"], "1.4");
        assert_eq!(written["sync"]["enabled"], false);
        assert_eq!(written["styles"][0]["properties"]["glow"], 3);
        assert_eq!(written["styles"][1]["tool"], "magnifier");
        assert_eq!(written["favourites"][2]["template"], "t1");
    }

    #[test]
    fn fresh_ids_skip_collisions_with_unknown_styles() {
        // Reviewer finding N1: a style this version can't read (e.g. an unknown tool) is kept
        // verbatim in `unknown_styles` and written back out unchanged, so a freshly assigned id
        // must avoid those too, not just known styles'. `next_id` is at 2 right after the first
        // `add`, so without the fix the next one would reuse "s2" below instead of skipping it.
        let mut lib = Library::default();
        lib.add(Tool::Arrow, "Coral", teal_bold());
        lib.unknown_styles.push(
            serde_json::json!({"id": "s2", "tool": "magnifier", "name": "Loupe", "properties": {}}),
        );
        let id = lib.add(Tool::Pen, "Pen", Toolbox::default().properties(Tool::Pen));
        assert_ne!(
            id, "s2",
            "must not collide with the preserved unknown style"
        );
    }

    #[test]
    fn schema_matches_what_is_written() {
        // Guards data/schema/styles-1.schema.json against drifting from the code.
        let schema: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/schema/styles-1.schema.json"
        )))
        .unwrap();
        let mut lib = Library::default();
        let id = lib.add(Tool::Arrow, "Teal bold", teal_bold());
        lib.set_favourite(&StyleRef::User(id), true);
        let written: Value = serde_json::from_str(&lib.to_json()).unwrap();
        let keys = |v: &Value| -> Vec<String> {
            let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
            k.sort();
            k
        };
        assert_eq!(keys(&written), keys(&schema["properties"]));
        assert_eq!(
            keys(&written["styles"][0]),
            keys(&schema["properties"]["styles"]["items"]["properties"])
        );
    }

    #[test]
    fn newer_major_and_broken_files() {
        assert_eq!(
            Library::parse(r#"{"format": "2.0", "styles": []}"#),
            Err(Unreadable::Newer(FormatVersion { major: 2, minor: 0 }))
        );
        assert_eq!(Library::parse("not json"), Err(Unreadable::Broken));
        assert_eq!(Library::parse(r#"{"styles": []}"#), Err(Unreadable::Broken));
        assert!(!Library::parse(r#"{"format": "1.0"}"#).unwrap().read_only);
        assert!(Library::read_only().read_only);
    }
}
