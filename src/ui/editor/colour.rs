//! Colours in the properties sidebar (add-core-annotation-tools redline "Colour", "Colour
//! picker popover"; mockups 02, 06, 08, 16, 18): which swatches a colour row offers, the
//! recent custom colours, and the colour arithmetic of the picker popover (saturation/value
//! area, hue and alpha sliders, Hex/R/G/B entries). No widgets here.

use annota::document::color::{palette, Rgba};

/// The highlighter's yellow (redline "Defaults": rgb(255 222 0)); its colour row offers it
/// where the others offer Amber (mockup 14).
pub const HIGHLIGHT_YELLOW: Rgba = Rgba::opaque(255, 222, 0);

/// How many custom colours are remembered (mockup 16: three, 24 px, in the popover; also
/// on the colour row's second line).
pub const RECENT: usize = 3;

/// The seven palette swatches of the first line (redline: Coral, Amber, Lime, Teal, Azure,
/// Violet, Ink).
pub const PALETTE: [Rgba; 7] = [
    palette::CORAL,
    palette::AMBER,
    palette::LIME,
    palette::TEAL,
    palette::AZURE,
    palette::VIOLET,
    palette::INK,
];

/// One swatch of a colour row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    /// A palette colour, Paper, or a recent custom colour.
    Colour(Rgba),
    /// No colour (a transparent fill or outline).
    None,
    /// Opens the colour picker popover.
    Custom,
    /// Picks a colour from the screen (xdg-desktop-portal `PickColor`).
    Eyedropper,
}

/// The kinds of colour rows in the redline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SwatchSet {
    /// 32 px; palette, then [None] · Paper · recent custom colours · Custom · Eyedropper.
    Full { none: bool },
    /// The highlighter's: like `Full`, with its yellow in Amber's place.
    Highlight,
    /// 32 px, the palette only (shape and callout outline, solid redaction colour).
    Palette,
    /// 24 px on one line: None, Paper, the palette (text outline, mockup 08).
    Outline,
    /// 24 px on one line: Paper, the palette (a callout's text colour).
    Text,
    /// 24 px, the palette only (shadow colour, mockup 18).
    Shadow,
}

impl SwatchSet {
    /// Swatch diameter in pixels.
    pub fn size(self) -> i32 {
        match self {
            SwatchSet::Full { .. } | SwatchSet::Highlight | SwatchSet::Palette => 32,
            SwatchSet::Outline | SwatchSet::Text | SwatchSet::Shadow => 24,
        }
    }

    /// The swatches, line by line, with the default palette ([`PALETTE`], the Annota theme).
    #[cfg(test)]
    pub fn lines(self, recent: &[Rgba]) -> Vec<Vec<Swatch>> {
        self.lines_with(&PALETTE, recent)
    }

    /// The swatches, line by line, with `palette` in the palette's places — the current
    /// theme's colours (add-style-system mockup 07: "Properties palette shows the theme
    /// colours"). The highlighter's set shows its yellow where the palette has Amber, else
    /// first.
    pub fn lines_with(self, colours: &[Rgba], recent: &[Rgba]) -> Vec<Vec<Swatch>> {
        let first = |yellow: bool| -> Vec<Swatch> {
            let mut line: Vec<Swatch> = colours.iter().map(|c| Swatch::Colour(*c)).collect();
            if yellow {
                let yellow = Swatch::Colour(HIGHLIGHT_YELLOW);
                match line
                    .iter()
                    .position(|s| *s == Swatch::Colour(palette::AMBER))
                {
                    Some(i) => line[i] = yellow,
                    None if !line.contains(&yellow) => line.insert(0, yellow),
                    None => {}
                }
            }
            line
        };
        let second = |none: bool| -> Vec<Swatch> {
            let mut line = Vec::new();
            if none {
                line.push(Swatch::None);
            }
            line.push(Swatch::Colour(palette::PAPER));
            line.extend(recent.iter().take(RECENT).map(|c| Swatch::Colour(*c)));
            line.push(Swatch::Custom);
            line.push(Swatch::Eyedropper);
            line
        };
        match self {
            SwatchSet::Full { none } => vec![first(false), second(none)],
            SwatchSet::Highlight => vec![first(true), second(false)],
            SwatchSet::Palette | SwatchSet::Shadow => vec![first(false)],
            SwatchSet::Outline => {
                let mut line = vec![Swatch::None, Swatch::Colour(palette::PAPER)];
                line.extend(first(false));
                vec![line]
            }
            SwatchSet::Text => {
                let mut line = vec![Swatch::Colour(palette::PAPER)];
                line.extend(first(false));
                vec![line]
            }
        }
    }

    /// Whether the set offers the colour picker and the eyedropper.
    pub fn custom(self) -> bool {
        matches!(self, SwatchSet::Full { .. } | SwatchSet::Highlight)
    }
}

/// Whether `swatch` shows `current` (its ring and check mark).
pub fn is_current(swatch: Swatch, current: Rgba) -> bool {
    match swatch {
        Swatch::Colour(c) => c == current,
        Swatch::None => current.a == 0,
        Swatch::Custom | Swatch::Eyedropper => false,
    }
}

/// Whether `colour` is one of the fixed swatches (palette, highlight yellow, Paper,
/// transparent) — those are not remembered as custom colours.
pub fn is_fixed(colour: Rgba) -> bool {
    colour.a == 0
        || PALETTE.contains(&colour)
        || colour == palette::PAPER
        || colour == HIGHLIGHT_YELLOW
}

/// Remembers a committed custom colour (redline: "Committing adds the colour as a selected
/// swatch before the Custom swatch"): newest first, no repeats, at most [`RECENT`].
pub fn remember(recent: &mut Vec<Rgba>, colour: Rgba) -> bool {
    if is_fixed(colour) {
        return false;
    }
    recent.retain(|c| *c != colour);
    recent.insert(0, colour);
    recent.truncate(RECENT);
    true
}

/// "#RRGGBB" (with "AA" when not opaque), as the Hex entry shows it.
pub fn hex(colour: Rgba) -> String {
    let rgb = format!("#{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b);
    if colour.a == 255 {
        rgb
    } else {
        format!("{rgb}{:02X}", colour.a)
    }
}

/// A typed hex colour: "#2F6FD0", "2f6fd0", "#2F6FD080" or the short "#26D"; `None` if it is
/// not one.
pub fn parse_hex(text: &str) -> Option<Rgba> {
    let t = text.trim();
    let digits = t.strip_prefix('#').unwrap_or(t);
    if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        3 => {
            let expanded: String = digits.chars().flat_map(|c| [c, c]).collect();
            Rgba::parse(&format!("#{expanded}"))
        }
        6 | 8 => Rgba::parse(&format!("#{digits}")),
        _ => None,
    }
}

/// A typed R, G or B value (0–255).
pub fn parse_channel(text: &str) -> Option<u8> {
    text.trim().parse::<u8>().ok()
}

/// A colour as the picker edits it: hue 0–360°, saturation, value and alpha 0–1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsva {
    pub h: f64,
    pub s: f64,
    pub v: f64,
    pub a: f64,
}

impl Hsva {
    pub fn from_rgba(c: Rgba) -> Self {
        let (r, g, b, a) = c.to_f64();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let h = if d <= f64::EPSILON {
            0.0
        } else if max == r {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        let s = if max <= f64::EPSILON { 0.0 } else { d / max };
        Self { h, s, v: max, a }
    }

    pub fn to_rgba(self) -> Rgba {
        let (r, g, b) = hsv_to_rgb(self.h, self.s, self.v);
        let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgba::new(byte(r), byte(g), byte(b), byte(self.a))
    }

    /// Moves the saturation/value point (fractions across and down the area).
    pub fn with_sv(self, across: f64, down: f64) -> Self {
        Self {
            s: across.clamp(0.0, 1.0),
            v: 1.0 - down.clamp(0.0, 1.0),
            ..self
        }
    }

    pub fn with_hue(self, fraction: f64) -> Self {
        Self {
            h: fraction.clamp(0.0, 1.0) * 360.0,
            ..self
        }
    }

    pub fn with_alpha(self, fraction: f64) -> Self {
        Self {
            a: fraction.clamp(0.0, 1.0),
            ..self
        }
    }
}

/// HSV (hue in degrees) to RGB, all 0–1.
pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (f64, f64, f64) {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    (r + m, g + m, b + m)
}

/// A colour from the portal's `PickColor` answer (three doubles 0–1), opaque.
pub fn from_portal(r: f64, g: f64, b: f64) -> Rgba {
    let byte = |v: f64| {
        if v.is_finite() {
            (v.clamp(0.0, 1.0) * 255.0).round() as u8
        } else {
            0
        }
    };
    Rgba::opaque(byte(r), byte(g), byte(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_offer_the_redline_swatches() {
        // Mockup 02 (arrow): palette, then Paper · Custom · Eyedropper.
        let lines = SwatchSet::Full { none: false }.lines(&[]);
        assert_eq!(lines[0].len(), 7);
        assert_eq!(
            lines[1],
            vec![
                Swatch::Colour(palette::PAPER),
                Swatch::Custom,
                Swatch::Eyedropper
            ]
        );
        // Mockup 06 (shape fill): None first; recent colours before Custom (mockup 16).
        let blue = Rgba::opaque(0x2F, 0x6F, 0xD0);
        let lines = SwatchSet::Full { none: true }.lines(&[blue]);
        assert_eq!(
            lines[1],
            vec![
                Swatch::None,
                Swatch::Colour(palette::PAPER),
                Swatch::Colour(blue),
                Swatch::Custom,
                Swatch::Eyedropper
            ]
        );
        // Mockup 14: the highlighter's yellow instead of Amber.
        assert_eq!(
            SwatchSet::Highlight.lines(&[])[0][1],
            Swatch::Colour(HIGHLIGHT_YELLOW)
        );
        // Mockup 08: text outline None, Paper, palette at 24 px; mockup 18: shadow palette.
        let outline = SwatchSet::Outline.lines(&[]);
        assert_eq!(outline.len(), 1);
        assert_eq!(
            outline[0][..2],
            [Swatch::None, Swatch::Colour(palette::PAPER)]
        );
        assert_eq!(SwatchSet::Outline.size(), 24);
        assert_eq!(
            SwatchSet::Shadow.lines(&[blue]),
            vec![PALETTE.map(Swatch::Colour).to_vec()]
        );
        assert!(!SwatchSet::Palette.custom());
    }

    #[test]
    fn a_theme_replaces_the_palette_swatches() {
        // add-style-system mockup 07: the theme's colours in the palette's places.
        let brand = [
            Rgba::opaque(0x0B, 0x5C, 0xAD),
            Rgba::opaque(0x00, 0xA0, 0xB0),
        ];
        let lines = SwatchSet::Full { none: false }.lines_with(&brand, &[]);
        assert_eq!(lines[0], brand.map(Swatch::Colour).to_vec());
        assert_eq!(lines[1][0], Swatch::Colour(palette::PAPER));
        // A theme without Amber: the highlighter's yellow comes first.
        let highlight = SwatchSet::Highlight.lines_with(&brand, &[]);
        assert_eq!(highlight[0][0], Swatch::Colour(HIGHLIGHT_YELLOW));
        assert_eq!(highlight[0].len(), 3);
    }

    #[test]
    fn current_swatch_and_recent_colours() {
        assert!(is_current(Swatch::None, Rgba::TRANSPARENT));
        assert!(!is_current(Swatch::None, palette::CORAL));
        assert!(is_current(Swatch::Colour(palette::CORAL), palette::CORAL));
        assert!(!is_current(Swatch::Custom, palette::CORAL));
        let mut recent = Vec::new();
        assert!(
            !remember(&mut recent, palette::CORAL),
            "palette colours are not custom"
        );
        let colours = [
            Rgba::opaque(1, 2, 3),
            Rgba::opaque(4, 5, 6),
            Rgba::opaque(7, 8, 9),
            Rgba::opaque(10, 11, 12),
        ];
        for c in colours {
            assert!(remember(&mut recent, c));
        }
        assert_eq!(recent, vec![colours[3], colours[2], colours[1]]);
        remember(&mut recent, colours[1]);
        assert_eq!(
            recent,
            vec![colours[1], colours[3], colours[2]],
            "no repeats"
        );
    }

    #[test]
    fn hex_and_channels() {
        let blue = Rgba::opaque(0x2F, 0x6F, 0xD0);
        assert_eq!(hex(blue), "#2F6FD0");
        assert_eq!(hex(Rgba::new(0x2F, 0x6F, 0xD0, 0x80)), "#2F6FD080");
        for text in ["#2F6FD0", "2f6fd0", " #2f6fd0 "] {
            assert_eq!(parse_hex(text), Some(blue), "{text}");
        }
        assert_eq!(parse_hex("#26d"), Some(Rgba::opaque(0x22, 0x66, 0xDD)));
        assert_eq!(parse_hex("#2F6FD080").map(|c| c.a), Some(0x80));
        for bad in ["", "#", "#12345", "zzzzzz", "#ÿÿÿ"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
        assert_eq!(parse_channel(" 208 "), Some(208));
        assert_eq!(parse_channel("256"), None);
    }

    #[test]
    fn hsv_round_trips() {
        for c in PALETTE.into_iter().chain([
            palette::PAPER,
            Rgba::opaque(0x2F, 0x6F, 0xD0),
            Rgba::new(10, 200, 30, 77),
        ]) {
            assert_eq!(Hsva::from_rgba(c).to_rgba(), c, "{c:?}");
        }
        let red = Hsva::from_rgba(Rgba::opaque(255, 0, 0));
        assert_eq!((red.h, red.s, red.v), (0.0, 1.0, 1.0));
        // The area: right = saturated, top = bright.
        let p = red.with_sv(0.0, 0.0);
        assert_eq!(p.to_rgba(), Rgba::opaque(255, 255, 255));
        let p = red.with_sv(1.0, 1.0);
        assert_eq!(p.to_rgba(), Rgba::opaque(0, 0, 0));
        assert_eq!(red.with_hue(1.0 / 3.0).to_rgba(), Rgba::opaque(0, 255, 0));
        assert_eq!(red.with_alpha(0.5).to_rgba().a, 128);
    }

    #[test]
    fn portal_colours_are_bytes() {
        // The PickColor answer: (r, g, b) as doubles 0–1.
        assert_eq!(
            from_portal(0.2078, 0.5176, 0.8941),
            Rgba::opaque(0x35, 0x84, 0xE4)
        );
        assert_eq!(from_portal(-1.0, 2.0, f64::NAN), Rgba::opaque(0, 255, 0));
    }
}
