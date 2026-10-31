//! Style values shared by several object types.

use serde::{Deserialize, Serialize};

use super::color::Rgba;

/// A drop shadow (spec annotation-tools: Common style properties — on/off, colour, angle,
/// distance, blur, opacity). The offset is applied in document space, so the light source does
/// not turn with a rotated object.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shadow {
    pub enabled: bool,
    /// Shadow colour; its alpha is multiplied with `opacity`.
    pub color: Rgba,
    /// 0.0–1.0.
    pub opacity: f64,
    /// Direction of the offset in degrees, clockwise from the positive x axis (90 = down).
    pub angle: f64,
    /// Length of the offset in document pixels.
    pub distance: f64,
    /// Gaussian standard deviation in document pixels.
    pub blur: f64,
}

impl Shadow {
    /// The shadow new annotation objects get (add-core-annotation-tools redline "Defaults":
    /// Ink, angle 45°, distance 9 px, blur 12 px, opacity 45 %). The redline's "blur 12 px" is
    /// a CSS-style blur radius, i.e. twice the Gaussian standard deviation stored here — the
    /// mockups render it as 3 px at 50 % zoom.
    pub const DEFAULT: Shadow = Shadow {
        enabled: true,
        color: super::color::palette::INK,
        opacity: 0.45,
        angle: 45.0,
        distance: 9.0,
        blur: 6.0,
    };

    /// [`Shadow::DEFAULT`] switched off (text: "shadow off").
    pub const DEFAULT_OFF: Shadow = Shadow {
        enabled: false,
        ..Shadow::DEFAULT
    };

    pub const OFF: Shadow = Shadow {
        enabled: false,
        color: Rgba::opaque(0, 0, 6),
        opacity: 0.32,
        angle: 54.5,
        distance: 4.3,
        blur: 3.0,
    };

    /// `(dx, dy)` in document pixels.
    pub fn offset(&self) -> (f64, f64) {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        (self.distance * cos, self.distance * sin)
    }

    /// How far the shadow can reach beyond the shape, for bounds/damage computation.
    pub fn reach(&self) -> f64 {
        if self.enabled {
            self.distance.abs() + 3.0 * self.blur.abs()
        } else {
            0.0
        }
    }

    /// Whether the shadow paints anything at all.
    pub fn is_visible(&self) -> bool {
        self.enabled && self.opacity > 0.0 && self.color.a > 0
    }
}

/// How a line or outline is drawn (sidebar: Line style — Solid / Dashed / Dotted).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl LineStyle {
    pub const ALL: [LineStyle; 3] = [LineStyle::Solid, LineStyle::Dashed, LineStyle::Dotted];

    /// Stable key, as stored and as `PropertyValue::Choice`.
    pub fn key(self) -> &'static str {
        match self {
            LineStyle::Solid => "solid",
            LineStyle::Dashed => "dashed",
            LineStyle::Dotted => "dotted",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }

    /// Dash pattern for a line `width` wide with round caps (mockups 03, 04: dashes 2.5 × the
    /// width with 1⅔ × the width between them, dots every 2 × the width).
    pub fn dashes(self, width: f64) -> Vec<f64> {
        let w = width.max(0.5);
        match self {
            LineStyle::Solid => Vec::new(),
            LineStyle::Dashed => vec![2.5 * w, 5.0 * w / 3.0],
            LineStyle::Dotted => vec![0.0, 2.0 * w],
        }
    }

    /// Sets dashes, round caps and round joins for a stroke `width` wide.
    pub fn apply(self, cr: &cairo::Context, width: f64) {
        cr.set_line_width(width);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.set_dash(&self.dashes(width), 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_styles_round_trip_their_keys() {
        for style in LineStyle::ALL {
            assert_eq!(LineStyle::from_key(style.key()), Some(style));
            assert_eq!(
                serde_json::to_value(style).unwrap(),
                serde_json::json!(style.key())
            );
        }
        assert_eq!(LineStyle::from_key("wavy"), None);
        assert!(LineStyle::Solid.dashes(6.0).is_empty());
        assert_eq!(LineStyle::Dashed.dashes(6.0), vec![15.0, 10.0]);
        assert_eq!(LineStyle::Dotted.dashes(4.0), vec![0.0, 8.0]);
    }

    #[test]
    fn offset_follows_angle() {
        let s = Shadow {
            enabled: true,
            angle: 90.0,
            distance: 10.0,
            ..Shadow::OFF
        };
        let (dx, dy) = s.offset();
        assert!(dx.abs() < 1e-9 && (dy - 10.0).abs() < 1e-9);
        assert_eq!(Shadow::OFF.reach(), 0.0);
        assert!((s.reach() - 19.0).abs() < 1e-9);
    }
}
