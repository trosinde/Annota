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

#[cfg(test)]
mod tests {
    use super::*;

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
