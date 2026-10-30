//! Colours as stored in `document.json`: `"#RRGGBBAA"` (`"#RRGGBB"` is accepted on read).

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A straight (non-premultiplied) sRGB colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);
    pub const WHITE: Rgba = Rgba::new(0xFF, 0xFF, 0xFF, 0xFF);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 0xFF)
    }

    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        match hex.len() {
            6 => Some(Self::opaque(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    /// Components as 0.0–1.0 for cairo's `set_source_rgba`.
    pub fn to_f64(self) -> (f64, f64, f64, f64) {
        (
            f64::from(self.r) / 255.0,
            f64::from(self.g) / 255.0,
            f64::from(self.b) / 255.0,
            f64::from(self.a) / 255.0,
        )
    }

    pub fn set_source(self, cr: &cairo::Context) {
        let (r, g, b, a) = self.to_f64();
        cr.set_source_rgba(r, g, b, a);
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02X}{:02X}{:02X}{:02X}",
            self.r, self.g, self.b, self.a
        )
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgba::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid colour '{text}'")))
    }
}

/// The Annota annotation palette (`docs/design/kit/tokens.css`). `add-style-system` makes the
/// palette themeable; documents always store the resolved colour.
pub mod palette {
    use super::Rgba;

    pub const CORAL: Rgba = Rgba::opaque(0xFF, 0x5A, 0x4E);
    pub const AMBER: Rgba = Rgba::opaque(0xF5, 0xA5, 0x24);
    pub const LIME: Rgba = Rgba::opaque(0x8B, 0xC3, 0x4A);
    pub const TEAL: Rgba = Rgba::opaque(0x14, 0xB8, 0xA6);
    pub const AZURE: Rgba = Rgba::opaque(0x2F, 0x80, 0xED);
    pub const VIOLET: Rgba = Rgba::opaque(0x8B, 0x5C, 0xF6);
    pub const INK: Rgba = Rgba::opaque(0x1F, 0x1F, 0x24);
    pub const PAPER: Rgba = Rgba::opaque(0xFF, 0xFF, 0xFF);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let c = Rgba::new(0x12, 0xAB, 0xFF, 0x80);
        assert_eq!(c.to_string(), "#12ABFF80");
        assert_eq!(Rgba::parse("#12abff80"), Some(c));
        assert_eq!(Rgba::parse("#FF5A4E"), Some(palette::CORAL));
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"#12ABFF80\"");
        assert_eq!(serde_json::from_str::<Rgba>("\"#12ABFF80\"").unwrap(), c);
    }

    #[test]
    fn invalid_colours_are_rejected() {
        for bad in ["", "#", "FF5A4E", "#FF5A4", "#GG0000", "#FF5A4E0", "#ÿÿÿ"] {
            assert_eq!(Rgba::parse(bad), None, "{bad}");
        }
        assert!(serde_json::from_str::<Rgba>("\"red\"").is_err());
    }
}
