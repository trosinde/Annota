//! The theme generator (spec style-system: Themes): eight quick styles per tool from a theme's
//! colours and fonts. Deterministic — the same theme always gives the same styles — so a
//! favourite theme style can be stored as "slot 3 of the arrow styles of theme X".
//!
//! Every slot starts from the tool's built-in defaults (the prototype of
//! `editing::tool::Toolbox`) and changes only colours, fonts and one or two style properties,
//! through `set_property`, so a generated style is always valid. Slot 0 is "the default look in
//! colour 1": for the Annota theme it *is* the built-in default of every tool, so nothing is
//! "Modified" in a fresh window, and "Use Theme" makes slot 0 every tool's defaults ("The first
//! colour is the default for every tool", mockup 06).
//!
//! The eight slots, per tool (`c1…c6` the theme's colours 1–6, `ink` its darkest colour after
//! the first — or, where colour 1 already is that dark (Monochrome), colour 5):
//!
//! | slot | arrow, line, pen | shape | text | callout | step | highlighter | redact |
//! |---|---|---|---|---|---|---|---|
//! | 0 | c1 | outline c1 | c1, heading font | fill c1 | c1 | yellow | pixelate 70 |
//! | 1 | c2 | outline c2 | c2 | fill c2 | c2 | c1 | pixelate 100 |
//! | 2 | c3 | outline c3 | c3 | fill c3 | c3 | c2 | blur 70 |
//! | 3 | c4 | outline c4 | c4 | fill c4 | c4 | c3 | blur 100 |
//! | 4 | ink | outline ink | ink | paper, ink outline | ink | c4 | solid ink |
//! | 5 | c1 bold | c1 tinted fill | c1 large | ink fill | c1 square | c5 | solid c1 |
//! | 6 | c1 dashed (pen: fine) | c1 solid fill | paper, c1 outline | paper, c1 outline | c1 large | yellow light | pixelate ellipse |
//! | 7 | c1 no shadow | c1 dashed | ink body text | fill c6 | paper, c1 ring | c1 light | blur ellipse |
//!
//! The rows 0–1 of arrows, callouts (0 and 2), steps and shapes are the theme editor's
//! preview (mockup 06: "8 sample tiles generated from colours 1–3"). Text on a fill (callouts,
//! step labels) is white unless white contrasts less than 3:1 with the fill (WCAG's minimum
//! for large text) and the theme's ink contrasts more — Amber or a yellow get dark text. A
//! theme with fewer colours than a slot needs uses a lighter tint of colour *i mod n*, so two
//! colours still give eight different styles. The highlighter keeps its yellow as slot 0: a
//! highlight is yellow first, and the theme's colours follow.

use crate::document::annotation::{Annotation, Property, PropertyValue};
use crate::document::color::{palette, Rgba};
use crate::editing::tool::{Tool, Toolbox};
use crate::tools::highlighter::Highlighter;

use super::theme::Theme;
use super::value;

/// How many styles a theme generates per tool (two rows of four, redline).
pub const SLOTS: usize = 8;

/// The theme editor's preview: (tool, slot) of its eight tiles (mockup 06).
pub const PREVIEW: [(Tool, usize); 8] = [
    (Tool::Arrow, 0),
    (Tool::Arrow, 1),
    (Tool::Callout, 0),
    (Tool::Callout, 2),
    (Tool::Step, 0),
    (Tool::Step, 1),
    (Tool::Shape, 0),
    (Tool::Shape, 1),
];

/// Relative luminance (WCAG 2), 0 (black) to 1 (white).
pub fn luminance(c: Rgba) -> f64 {
    let lin = |v: u8| {
        let s = f64::from(v) / 255.0;
        if s <= 0.040_45 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio, 1–21.
pub fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `c` mixed with white by `amount` (0–1).
fn tint(c: Rgba, amount: f64) -> Rgba {
    let mix = |v: u8| (f64::from(v) + (255.0 - f64::from(v)) * amount).round() as u8;
    Rgba::new(mix(c.r), mix(c.g), mix(c.b), c.a)
}

/// The colours a theme's slots use.
struct Colours<'a> {
    theme: &'a Theme,
    ink: Rgba,
}

impl<'a> Colours<'a> {
    fn new(theme: &'a Theme) -> Self {
        // The darkest colour other than the first (which has slot 0 already), if it is dark
        // enough to be ink; else the Annota ink.
        let ink = theme
            .colours
            .iter()
            .skip(1)
            .copied()
            .filter(|c| luminance(*c) < 0.05)
            .min_by(|a, b| luminance(*a).total_cmp(&luminance(*b)))
            .unwrap_or(palette::INK);
        Colours { theme, ink }
    }

    /// Colour `i` (0-based); beyond the theme's colours a lighter tint of colour `i mod n`,
    /// lighter with every round, so no two slots share a colour.
    fn c(&self, i: usize) -> Rgba {
        let n = self.theme.colours.len().max(1);
        match self.theme.colour(i) {
            Some(c) => c,
            None => {
                let round = i32::try_from(i / n).unwrap_or(i32::MAX);
                self.theme
                    .colour(i % n)
                    .map_or(palette::CORAL, |c| tint(c, 1.0 - 0.55f64.powi(round)))
            }
        }
    }

    /// The colour of the "ink" slot (4): the ink, or — where colour 1 already is the ink
    /// (Monochrome) — colour 5.
    fn slot4(&self) -> Rgba {
        if contrast(self.ink, self.c(0)) < 1.5 {
            self.c(4)
        } else {
            self.ink
        }
    }

    /// Text on `fill`: white, unless that is hard to read and ink is better.
    fn on(&self, fill: Rgba) -> Rgba {
        let white = contrast(fill, palette::PAPER);
        if white < 3.0 && contrast(fill, self.ink) > white {
            self.ink
        } else {
            palette::PAPER
        }
    }
}

type Change = (&'static str, PropertyValue);

fn colour(key: &'static str, c: Rgba) -> Change {
    (key, PropertyValue::Color(c))
}

fn number(key: &'static str, n: f64) -> Change {
    (key, PropertyValue::Number(n))
}

fn choice(key: &'static str, c: &str) -> Change {
    (key, PropertyValue::Choice(c.into()))
}

fn text(key: &'static str, t: &str) -> Change {
    (key, PropertyValue::Text(t.into()))
}

fn flag(key: &'static str, b: bool) -> Change {
    (key, PropertyValue::Bool(b))
}

fn no_shadow(prototype: &dyn Annotation) -> Option<Change> {
    prototype
        .properties()
        .into_iter()
        .find_map(|p| match p.value {
            PropertyValue::Shadow(mut s) if p.key == "shadow" => {
                s.enabled = false;
                Some(("shadow", PropertyValue::Shadow(s)))
            }
            _ => None,
        })
}

/// What slot `slot` of `tool` changes from the tool's defaults.
fn changes(tool: Tool, slot: usize, k: &Colours<'_>, prototype: &dyn Annotation) -> Vec<Change> {
    let c1 = k.c(0);
    let ink = k.slot4();
    let paper = palette::PAPER;
    // Slots 0–3: colours 1–4; slot 4: ink.
    let main = match slot {
        0..=3 => k.c(slot),
        4 => ink,
        _ => c1,
    };
    let heading = k.theme.heading_font.as_str();
    let body = k.theme.body_font.as_str();
    match tool {
        Tool::Arrow | Tool::Line | Tool::Pen => {
            let mut out = vec![colour("color", main)];
            match (slot, tool) {
                (5, Tool::Arrow) => out.push(number("width", 12.0)),
                (5, Tool::Line) => out.push(number("width", 10.0)),
                (5, _) => out.push(number("width", 16.0)),
                (6, Tool::Pen) => out.push(number("width", 4.0)),
                (6, _) => out.push(choice("line_style", "dashed")),
                (7, _) => out.extend(no_shadow(prototype)),
                _ => {}
            }
            out
        }
        Tool::Shape => {
            let mut out = vec![colour("outline_color", main)];
            match slot {
                5 => out.extend([colour("fill", c1), number("fill_opacity", 0.2)]),
                6 => out.extend([colour("fill", c1), number("fill_opacity", 1.0)]),
                7 => out.push(choice("line_style", "dashed")),
                _ => {}
            }
            out
        }
        Tool::Text => {
            let mut out = vec![text("font_family", heading), colour("color", main)];
            match slot {
                5 => out.push(number("font_size", 48.0)),
                6 => out.extend([colour("color", paper), colour("outline_color", c1)]),
                7 => out.extend([
                    text("font_family", body),
                    colour("color", k.ink),
                    flag("bold", false),
                    number("font_size", 24.0),
                    number("outline_width", 0.0),
                ]),
                _ => {}
            }
            out
        }
        Tool::Callout => {
            let filled = |fill: Rgba| {
                vec![
                    colour("fill", fill),
                    colour("outline_color", fill),
                    colour("color", k.on(fill)),
                ]
            };
            let mut out = vec![text("font_family", body)];
            out.extend(match slot {
                0..=3 => filled(k.c(slot)),
                4 => vec![
                    colour("fill", paper),
                    colour("outline_color", ink),
                    number("outline_width", 3.0),
                    colour("color", ink),
                ],
                5 => filled(ink),
                6 => vec![
                    colour("fill", paper),
                    colour("outline_color", c1),
                    number("outline_width", 3.0),
                    colour("color", c1),
                ],
                _ => filled(k.c(5)),
            });
            out
        }
        Tool::Step => {
            let filled =
                |fill: Rgba| vec![colour("color", fill), colour("label_color", k.on(fill))];
            match slot {
                0..=4 => filled(main),
                5 => {
                    let mut out = filled(c1);
                    out.push(choice("shape", "square"));
                    out
                }
                6 => {
                    let mut out = filled(c1);
                    out.push(number("radius", 40.0));
                    out
                }
                _ => vec![
                    colour("color", paper),
                    colour("label_color", c1),
                    colour("ring_color", c1),
                    number("ring_width", 4.0),
                ],
            }
        }
        Tool::Highlighter => match slot {
            0 => vec![colour("color", Highlighter::YELLOW)],
            1..=5 => vec![colour("color", k.c(slot - 1))],
            6 => vec![colour("color", Highlighter::YELLOW), number("opacity", 0.3)],
            _ => vec![colour("color", c1), number("opacity", 0.3)],
        },
        Tool::Redact => match slot {
            0 => vec![],
            1 => vec![number("intensity", 100.0)],
            2 => vec![choice("redaction", "blur")],
            3 => vec![choice("redaction", "blur"), number("intensity", 100.0)],
            4 => vec![choice("redaction", "solid"), colour("color", ink)],
            5 => vec![choice("redaction", "solid"), colour("color", c1)],
            6 => vec![choice("shape", "ellipse")],
            _ => vec![choice("redaction", "blur"), choice("shape", "ellipse")],
        },
        Tool::Select => Vec::new(),
    }
}

/// The style of slot `slot` (0–7) of `tool` for `theme`: the tool's style properties, in the
/// sidebar's order. Changes a tool refuses (a font name is never refused) are left out.
pub fn style(theme: &Theme, tool: Tool, slot: usize) -> Vec<Property> {
    let toolbox = Toolbox::default();
    let Some(prototype) = toolbox.prototype(tool) else {
        return Vec::new();
    };
    let mut object = prototype.clone_box();
    let colours = Colours::new(theme);
    for (key, value) in changes(tool, slot, &colours, prototype) {
        // Never fails for the values above (tested); a refused value keeps the default.
        let _ = object.set_property(key, &value);
    }
    value::style_of(tool, &object.properties())
}

/// The eight styles of `tool` for `theme`.
pub fn styles(theme: &Theme, tool: Tool) -> Vec<Vec<Property>> {
    (0..SLOTS).map(|slot| style(theme, tool, slot)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::styles::theme::Origin;
    use crate::styles::value::TOOLS;

    fn northwind() -> Theme {
        Theme {
            id: "user/northwind".into(),
            origin: Origin::User("northwind".into()),
            name: "Northwind Ferries".into(),
            colours: [0x0B5CAD, 0x00A0B0, 0xF2A900, 0xE4572E, 0x2D2D34]
                .map(|v: u32| Rgba::opaque((v >> 16) as u8, (v >> 8) as u8, v as u8))
                .to_vec(),
            heading_font: "Inter Display".into(),
            body_font: "Inter".into(),
            format_minor: 0,
            unknown: Default::default(),
        }
    }

    fn get<'a>(values: &'a [Property], key: &str) -> &'a PropertyValue {
        &values.iter().find(|p| p.key == key).unwrap().value
    }

    #[test]
    fn annota_slot_0_is_every_tools_default() {
        let toolbox = Toolbox::default();
        for tool in TOOLS {
            let defaults = value::style_of(tool, &toolbox.properties(tool));
            assert_eq!(style(&Theme::annota(), tool, 0), defaults, "{tool:?}");
        }
    }

    #[test]
    fn every_change_is_accepted_by_its_tool() {
        // `style` ignores refused changes; here every one must be accepted.
        let theme = northwind();
        let colours = Colours::new(&theme);
        let toolbox = Toolbox::default();
        for tool in TOOLS {
            let prototype = toolbox.prototype(tool).unwrap();
            for slot in 0..SLOTS {
                let mut object = prototype.clone_box();
                for (key, value) in changes(tool, slot, &colours, prototype) {
                    object
                        .set_property(key, &value)
                        .unwrap_or_else(|e| panic!("{tool:?} slot {slot} {key}: {e}"));
                }
            }
        }
    }

    #[test]
    fn eight_different_styles_per_tool_even_with_two_colours() {
        let mut two = northwind();
        two.colours.truncate(2);
        for theme in [Theme::annota(), Theme::monochrome(), northwind(), two] {
            for tool in TOOLS {
                let all = styles(&theme, tool);
                assert_eq!(all.len(), SLOTS);
                for i in 0..SLOTS {
                    for j in i + 1..SLOTS {
                        assert_ne!(all[i], all[j], "{} {tool:?} {i} = {j}", theme.name);
                    }
                }
            }
        }
    }

    #[test]
    fn a_theme_with_brand_colours_colours_every_tool() {
        // Scenario "Apply theme": every tool's theme quick styles use the theme's colours.
        let theme = northwind();
        let blue = PropertyValue::Color(theme.colours[0]);
        assert_eq!(get(&style(&theme, Tool::Arrow, 0), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Line, 0), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Pen, 0), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Shape, 0), "outline_color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Text, 0), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Callout, 0), "fill"), &blue);
        assert_eq!(get(&style(&theme, Tool::Step, 0), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Highlighter, 1), "color"), &blue);
        assert_eq!(get(&style(&theme, Tool::Redact, 5), "color"), &blue);
        // Fonts: heading for the text tool, body for callouts.
        assert_eq!(
            get(&style(&theme, Tool::Text, 0), "font_family"),
            &PropertyValue::Text("Inter Display".into())
        );
        assert_eq!(
            get(&style(&theme, Tool::Callout, 0), "font_family"),
            &PropertyValue::Text("Inter".into())
        );
        // Ink: the darkest colour after the first.
        assert_eq!(
            get(&style(&theme, Tool::Arrow, 4), "color"),
            &PropertyValue::Color(theme.colours[4])
        );
        // Mockup 06: a yellow callout gets dark text, the blue one white.
        assert_eq!(
            get(&style(&theme, Tool::Callout, 2), "color"),
            &PropertyValue::Color(theme.colours[4])
        );
        assert_eq!(
            get(&style(&theme, Tool::Callout, 0), "color"),
            &PropertyValue::Color(palette::PAPER)
        );
    }

    #[test]
    fn generation_is_deterministic() {
        let theme = northwind();
        for tool in TOOLS {
            assert_eq!(styles(&theme, tool), styles(&theme, tool));
        }
    }

    #[test]
    fn contrast_and_luminance() {
        assert!((contrast(palette::PAPER, Rgba::opaque(0, 0, 0)) - 21.0).abs() < 1e-9);
        assert!(
            contrast(palette::CORAL, palette::PAPER) >= 3.0,
            "white on coral"
        );
        assert!(
            contrast(palette::AMBER, palette::PAPER) < 3.0,
            "dark on amber"
        );
    }
}
