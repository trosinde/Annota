//! The quick styles and themes of the editor window (add-style-system): the library and the
//! themes on disk (`annota::styles`), shared by the sidebar's quick styles and the theme
//! editor, and the words for them (descriptions, colour names, suggested names, import errors).
//!
//! Every change to the library is saved at once and announced to the listeners
//! ([`Styles::connect_changed`]), which rebuild what they show.

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};

use annota::document::annotation::{Property, PropertyValue};
use annota::document::color::{palette, Rgba};
use annota::editing::tool::Tool;
use annota::styles::generate;
use annota::styles::library::{Library, StyleRef};
use annota::styles::store::{ImportError, StylePaths};
use annota::styles::theme::{Theme, ThemeError, DEFAULT_ID};

use super::colour::{self, Hsva};
use super::panel::swatch_name;
use crate::ui::labels;

type Listener = Rc<dyn Fn()>;
/// The generated styles of a (theme id, tool).
type Generated = HashMap<(String, Tool), Rc<Vec<Vec<Property>>>>;

/// A style resolved for showing and applying.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub tool: Tool,
    pub values: Vec<Property>,
    /// A user style's name.
    pub name: Option<String>,
}

pub struct Styles {
    paths: StylePaths,
    library: RefCell<Library>,
    themes: RefCell<Vec<Theme>>,
    /// Generated styles per (theme id, tool), dropped when themes change.
    generated: RefCell<Generated>,
    /// Counts changes, so views know when to rebuild.
    generation: Cell<u64>,
    listeners: RefCell<Vec<Listener>>,
}

impl Styles {
    pub fn load(paths: StylePaths) -> Rc<Self> {
        let library = paths.load_library();
        let themes = paths.load_themes();
        Rc::new(Styles {
            paths,
            library: RefCell::new(library),
            themes: RefCell::new(themes),
            generated: RefCell::new(HashMap::new()),
            generation: Cell::new(0),
            listeners: RefCell::new(Vec::new()),
        })
    }

    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Rc::new(f));
    }

    fn notify(&self) {
        self.generation.set(self.generation.get() + 1);
        let listeners: Vec<Listener> = self.listeners.borrow().clone();
        for f in listeners {
            f();
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation.get()
    }

    pub fn library(&self) -> Ref<'_, Library> {
        self.library.borrow()
    }

    pub fn themes(&self) -> Ref<'_, Vec<Theme>> {
        self.themes.borrow()
    }

    pub fn theme(&self, id: &str) -> Option<Theme> {
        self.themes.borrow().iter().find(|t| t.id == id).cloned()
    }

    /// The theme in use; Annota if the library names one that is gone.
    pub fn current_theme(&self) -> Theme {
        let id = self.library.borrow().theme.clone();
        self.theme(&id).unwrap_or_else(Theme::annota)
    }

    /// The eight generated styles of `tool` for `theme`.
    pub fn generated(&self, theme: &Theme, tool: Tool) -> Rc<Vec<Vec<Property>>> {
        let key = (theme.id.clone(), tool);
        if let Some(styles) = self.generated.borrow().get(&key) {
            return styles.clone();
        }
        let styles = Rc::new(generate::styles(theme, tool));
        self.generated.borrow_mut().insert(key, styles.clone());
        styles
    }

    /// What a reference stands for: a user style, or a slot of a theme that still exists.
    pub fn resolve(&self, style: &StyleRef) -> Option<Resolved> {
        match style {
            StyleRef::User(id) => {
                let library = self.library.borrow();
                let s = library.get(id)?;
                Some(Resolved {
                    tool: s.tool,
                    values: s.values.clone(),
                    name: Some(s.name.clone()),
                })
            }
            StyleRef::Theme { theme, tool, slot } => {
                let theme = self.theme(theme)?;
                let values = self.generated(&theme, *tool).get(*slot)?.clone();
                Some(Resolved {
                    tool: *tool,
                    values,
                    name: None,
                })
            }
        }
    }

    /// Changes the library, saves it and tells the listeners.
    pub fn change<R>(&self, f: impl FnOnce(&mut Library) -> R) -> R {
        let result = f(&mut self.library.borrow_mut());
        if let Err(err) = self.paths.save_library(&self.library.borrow()) {
            eprintln!("annota: cannot save quick styles: {err}");
        }
        self.notify();
        result
    }

    /// Uses theme `id` (the switcher, "Use Theme").
    pub fn set_theme(&self, id: &str) {
        if self.theme(id).is_some() && self.library.borrow().theme != id {
            self.change(|l| l.theme = id.to_string());
        }
    }

    /// Reads the themes again (after one was saved, imported or deleted).
    pub fn reload_themes(&self) {
        self.themes.replace(self.paths.load_themes());
        self.generated.borrow_mut().clear();
        // A deleted current theme: back to Annota.
        let current = self.library.borrow().theme.clone();
        if self.theme(&current).is_none() {
            self.change(|l| l.theme = DEFAULT_ID.to_string());
        } else {
            self.notify();
        }
    }

    /// Saves a user theme (a new one gets its file) and reads the themes again.
    pub fn save_theme(&self, theme: &mut Theme) -> std::io::Result<()> {
        let result = self.paths.save_theme(theme);
        self.reload_themes();
        result
    }

    pub fn delete_theme(&self, theme: &Theme) -> std::io::Result<()> {
        let result = self.paths.delete_theme(theme);
        self.reload_themes();
        result
    }

    pub fn import(&self, file: &Path) -> Result<Theme, ImportError> {
        let result = self.paths.import(file);
        if result.is_ok() {
            self.reload_themes();
        }
        result
    }
}

// ---- words ----

/// A colour's name: the palette's names, else a hue name ("Blue", "Dark grey").
pub fn colour_name(c: Rgba) -> String {
    match c {
        palette::CORAL
        | palette::AMBER
        | palette::LIME
        | palette::TEAL
        | palette::AZURE
        | palette::VIOLET
        | palette::INK
        | palette::PAPER
        | colour::HIGHLIGHT_YELLOW => swatch_name(c),
        _ => hue_name(c),
    }
}

fn hue_name(c: Rgba) -> String {
    if c.a == 0 {
        return pgettext("colour", "None");
    }
    let hsv = Hsva::from_rgba(c);
    if hsv.s < 0.15 || hsv.v < 0.12 {
        return if hsv.v < 0.2 {
            pgettext("colour", "Black")
        } else if hsv.v > 0.92 {
            pgettext("colour", "White")
        } else {
            pgettext("colour", "Grey")
        };
    }
    match hsv.h {
        h if h < 15.0 => pgettext("colour", "Red"),
        h if h < 40.0 => pgettext("colour", "Orange"),
        h if h < 65.0 => pgettext("colour", "Yellow"),
        h if h < 160.0 => pgettext("colour", "Green"),
        h if h < 195.0 => pgettext("colour", "Cyan"),
        h if h < 255.0 => pgettext("colour", "Blue"),
        h if h < 290.0 => pgettext("colour", "Purple"),
        h if h < 340.0 => pgettext("colour", "Pink"),
        _ => pgettext("colour", "Red"),
    }
}

fn value<'a>(values: &'a [Property], key: &str) -> Option<&'a PropertyValue> {
    values.iter().find(|p| p.key == key).map(|p| &p.value)
}

fn colour_of(values: &[Property], key: &str) -> Option<Rgba> {
    match value(values, key)? {
        PropertyValue::Color(c) => Some(*c),
        _ => None,
    }
}

fn number_of(values: &[Property], key: &str) -> Option<f64> {
    match value(values, key)? {
        PropertyValue::Number(n) => Some(*n),
        _ => None,
    }
}

fn choice_of<'a>(values: &'a [Property], key: &str) -> Option<&'a str> {
    match value(values, key)? {
        PropertyValue::Choice(c) | PropertyValue::Text(c) => Some(c),
        _ => None,
    }
}

fn px(n: f64) -> String {
    gettext("{n} px").replace("{n}", &super::sidebar::format_number(n))
}

fn percent(fraction: f64) -> String {
    gettext("{n} %").replace("{n}", &super::sidebar::format_number(fraction * 100.0))
}

/// The tile's tooltip (redline: "Coral · 7 px · shadow").
pub fn description(tool: Tool, values: &[Property]) -> String {
    let colour = |key: &str| colour_of(values, key).map(colour_name).unwrap_or_default();
    let shadow = match value(values, "shadow") {
        Some(PropertyValue::Shadow(s)) if s.enabled => gettext("shadow"),
        Some(PropertyValue::Shadow(_)) => gettext("no shadow"),
        _ => String::new(),
    };
    let line = match choice_of(values, "line_style") {
        Some("dashed") => gettext("dashed"),
        Some("dotted") => gettext("dotted"),
        _ => String::new(),
    };
    let parts: Vec<String> = match tool {
        Tool::Arrow | Tool::Line | Tool::Pen => vec![
            colour("color"),
            number_of(values, "width").map(px).unwrap_or_default(),
            line,
            shadow,
        ],
        Tool::Shape => {
            let fill = match colour_of(values, "fill") {
                Some(c) if c.a > 0 => gettext("{colour} fill").replace("{colour}", &colour_name(c)),
                _ => gettext("no fill"),
            };
            vec![
                colour("outline_color"),
                number_of(values, "outline_width")
                    .map(px)
                    .unwrap_or_default(),
                fill,
                line,
            ]
        }
        Tool::Text => vec![
            colour("color"),
            number_of(values, "font_size").map(px).unwrap_or_default(),
            choice_of(values, "font_family")
                .unwrap_or_default()
                .to_string(),
        ],
        Tool::Callout => {
            let fill = colour_of(values, "fill").unwrap_or(palette::CORAL);
            let text = colour_of(values, "color").unwrap_or(palette::PAPER);
            let fill_name = if fill.a == 0 {
                pgettext("colour", "None")
            } else {
                colour_name(fill)
            };
            let template = match text {
                palette::PAPER => gettext("{fill}, white text"),
                palette::INK => gettext("{fill}, dark text"),
                _ => gettext("{fill}, {colour} text").replace("{colour}", &colour_name(text)),
            };
            vec![template.replace("{fill}", &fill_name)]
        }
        Tool::Step => vec![
            colour("color"),
            match choice_of(values, "shape") {
                Some("square") => pgettext("step shape", "square"),
                Some("rounded_square") => pgettext("step shape", "rounded square"),
                Some("pin") => pgettext("step shape", "pin"),
                _ => pgettext("step shape", "circle"),
            },
            number_of(values, "radius")
                .map(|r| px(r * 2.0))
                .unwrap_or_default(),
        ],
        Tool::Highlighter => vec![
            colour("color"),
            number_of(values, "opacity")
                .map(percent)
                .unwrap_or_default(),
        ],
        Tool::Redact => match choice_of(values, "redaction") {
            Some("solid") => vec![gettext("Solid"), colour("color")],
            kind => vec![
                if kind == Some("blur") {
                    gettext("Blur")
                } else {
                    gettext("Pixelate")
                },
                number_of(values, "intensity")
                    .map(|n| percent(n / 100.0))
                    .unwrap_or_default(),
                if choice_of(values, "shape") == Some("ellipse") {
                    gettext("ellipse")
                } else {
                    String::new()
                },
            ],
        },
        Tool::Select => Vec::new(),
    };
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Favourites view tooltip (mockup 04: "Callout · Coral, white text").
pub fn favourite_description(tool: Tool, values: &[Property]) -> String {
    format!(
        "{} · {}",
        labels::tool_name(tool),
        description(tool, values)
    )
}

/// The key whose colour names a style, and the key whose size makes it "bold" or "thin".
fn naming_keys(tool: Tool) -> (&'static str, &'static str) {
    match tool {
        Tool::Shape => ("outline_color", "outline_width"),
        Tool::Text => ("color", "font_size"),
        Tool::Callout => ("fill", "font_size"),
        Tool::Step => ("color", "radius"),
        Tool::Highlighter => ("color", "opacity"),
        Tool::Redact => ("color", "intensity"),
        _ => ("color", "width"),
    }
}

/// A name for saving `values` (mockup 02: "Teal bold"): the main colour's name, "bold" or
/// "thin" against the theme's first style of the tool.
pub fn suggested_name(tool: Tool, values: &[Property], base: &[Property]) -> String {
    let (colour_key, size_key) = naming_keys(tool);
    let name = if tool == Tool::Redact && choice_of(values, "redaction") != Some("solid") {
        if choice_of(values, "redaction") == Some("blur") {
            gettext("Blur")
        } else {
            gettext("Pixelate")
        }
    } else {
        colour_of(values, colour_key)
            .map(colour_name)
            .unwrap_or_else(|| labels::tool_name(tool))
    };
    match (number_of(values, size_key), number_of(base, size_key)) {
        (Some(v), Some(b)) if v > b + 1e-9 => gettext("{colour} bold").replace("{colour}", &name),
        (Some(v), Some(b)) if v < b - 1e-9 => gettext("{colour} thin").replace("{colour}", &name),
        _ => name,
    }
}

/// The "Can't Import Theme" body (mockup 10).
pub fn import_error(file: &str, err: &ImportError) -> String {
    let reason = match err {
        ImportError::Read(_) => gettext("The file could not be read."),
        ImportError::Write(_) => gettext("It could not be saved to your themes."),
        ImportError::Invalid(ThemeError::ColourCount(n)) => ngettext(
            "It contains {n} colour; a theme needs 2–8.",
            "It contains {n} colours; a theme needs 2–8.",
            u32::try_from(*n).unwrap_or(u32::MAX),
        )
        .replace("{n}", &n.to_string()),
        ImportError::Invalid(ThemeError::UnknownVersion(v)) => {
            gettext("Its schema version “{version}” is unknown to this version of Annota.")
                .replace("{version}", v)
        }
        ImportError::Invalid(ThemeError::NoVersion) => gettext("It has no schema version."),
        ImportError::Invalid(ThemeError::NotJson) => gettext("It is not a JSON file."),
        ImportError::Invalid(ThemeError::InvalidColour(c)) => {
            gettext("“{colour}” is not a colour.").replace("{colour}", c.trim_matches('"'))
        }
        ImportError::Invalid(ThemeError::NoName) => gettext("It has no name."),
        ImportError::Invalid(ThemeError::NoFonts) => {
            gettext("It does not name a heading font and a body font.")
        }
    };
    match err {
        ImportError::Read(_) | ImportError::Write(_) => {
            gettext("“{file}” could not be imported. {reason}")
        }
        ImportError::Invalid(_) => gettext("“{file}” is not a valid Annota theme. {reason}"),
    }
    .replace("{file}", file)
    .replace("{reason}", &reason)
}

/// Draws up to `n` colour chips (`w` × `h`, radius `r`, 2 px apart) at the origin.
pub fn draw_chips(
    cr: &gtk::cairo::Context,
    colours: &[Rgba],
    n: usize,
    w: f64,
    h: f64,
    r: f64,
) -> Result<(), gtk::cairo::Error> {
    for (i, c) in colours.iter().take(n).enumerate() {
        let x = i as f64 * (w + 2.0);
        cr.new_sub_path();
        cr.arc(x + w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
        cr.arc(x + w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
        cr.arc(
            x + r,
            h - r,
            r,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
        );
        cr.arc(
            x + r,
            r,
            r,
            std::f64::consts::PI,
            1.5 * std::f64::consts::PI,
        );
        cr.close_path();
        let (red, g, b, a) = c.to_f64();
        cr.set_source_rgba(red, g, b, a);
        cr.fill()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::editing::tool::Toolbox;
    use annota::styles::value;
    use annota::tools::highlighter::Highlighter;

    #[test]
    fn colour_names() {
        assert_eq!(colour_name(palette::TEAL), "Teal");
        assert_eq!(colour_name(Rgba::opaque(0x0B, 0x5C, 0xAD)), "Blue");
        assert_eq!(colour_name(Rgba::opaque(0x77, 0x76, 0x7B)), "Grey");
        assert_eq!(colour_name(Rgba::opaque(0x05, 0x05, 0x08)), "Black");
        assert_eq!(colour_name(Highlighter::YELLOW), "Yellow");
    }

    #[test]
    fn descriptions_and_suggested_names() {
        // Mockup 02: Teal, 8 px, no shadow → "Teal bold" (against the 6 px of … the default
        // arrow is 8 px, so here the base is thinner).
        let toolbox = Toolbox::default();
        let mut values = value::style_of(Tool::Arrow, &toolbox.properties(Tool::Arrow));
        let base = values.clone();
        for p in &mut values {
            match p.key {
                "color" => p.value = PropertyValue::Color(palette::TEAL),
                "width" => p.value = PropertyValue::Number(12.0),
                "shadow" => {
                    if let PropertyValue::Shadow(s) = &mut p.value {
                        s.enabled = false;
                    }
                }
                _ => {}
            }
        }
        assert_eq!(
            description(Tool::Arrow, &values),
            "Teal · 12 px · no shadow"
        );
        assert_eq!(suggested_name(Tool::Arrow, &values, &base), "Teal bold");
        assert_eq!(suggested_name(Tool::Arrow, &base, &base), "Coral");
        let callout = value::style_of(Tool::Callout, &toolbox.properties(Tool::Callout));
        assert_eq!(description(Tool::Callout, &callout), "Coral, white text");
        assert_eq!(
            favourite_description(Tool::Callout, &callout),
            "Callout · Coral, white text"
        );
        let redact = value::style_of(Tool::Redact, &toolbox.properties(Tool::Redact));
        assert_eq!(description(Tool::Redact, &redact), "Pixelate · 70 %");
    }

    #[test]
    fn import_errors_name_the_reason() {
        let body = import_error(
            "brand-2024.annota-theme",
            &ImportError::Invalid(ThemeError::ColourCount(11)),
        );
        assert_eq!(
            body,
            "“brand-2024.annota-theme” is not a valid Annota theme. It contains 11 colours; a theme needs 2–8."
        );
        assert!(import_error("x", &ImportError::Invalid(ThemeError::NotJson)).contains("JSON"));
        assert!(import_error(
            "x",
            &ImportError::Invalid(ThemeError::UnknownVersion("2.0".into()))
        )
        .contains("“2.0”"));
    }
}
