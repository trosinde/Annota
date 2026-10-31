//! Helpers shared by the annotation types of add-core-annotation-tools: opacity and drop
//! shadow around a type's drawing, point geometry (arrows, pen strokes), box mapping for types
//! whose geometry is a list of points, and property value parsing.

use crate::document::annotation::{
    box_handles, Compass, Handle, HandleId, PropertyError, PropertyValue,
};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::Shadow;
use crate::render::shadow;

/// Draws `draw` with the object's `opacity` (as one layer, so overlapping parts do not add up)
/// and its drop `shadow` under it. `body` (user space) covers everything `draw` paints.
pub fn styled(
    cr: &cairo::Context,
    opacity: f64,
    shadow: &Shadow,
    body: Rect,
    draw: impl Fn(&cairo::Context) -> Result<(), cairo::Error>,
) -> Result<(), cairo::Error> {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return Ok(());
    }
    if opacity < 1.0 {
        cr.push_group();
    }
    let drawn = (|| {
        shadow::drop_shadow(cr, shadow, body, &draw)?;
        cr.save()?;
        let drawn = draw(cr);
        cr.restore()?;
        drawn
    })();
    if opacity < 1.0 {
        cr.pop_group_to_source()?;
        drawn?;
        cr.paint_with_alpha(opacity)?;
        return Ok(());
    }
    drawn
}

/// Adds the ellipse inscribed in `r` to the path (the path keeps the current transformation).
pub fn ellipse_path(cr: &cairo::Context, r: Rect) {
    let m = cr.matrix();
    cr.translate(r.x + r.width / 2.0, r.y + r.height / 2.0);
    cr.scale((r.width / 2.0).max(1e-6), (r.height / 2.0).max(1e-6));
    cr.new_sub_path();
    cr.arc(0.0, 0.0, 1.0, 0.0, std::f64::consts::TAU);
    cr.close_path();
    cr.set_matrix(m);
}

/// `body` grown by where `shadow` can reach (offset and three standard deviations of blur).
pub fn with_shadow(body: Rect, shadow: &Shadow) -> Rect {
    if body.is_none() || !shadow.is_visible() {
        return body;
    }
    let (dx, dy) = shadow.offset();
    body.union(
        &body
            .translate(dx, dy)
            .inflate(3.0 * shadow.blur.max(0.0) + 1.0),
    )
}

/// The smallest upright box around `points` (`Rect::NONE` for none).
pub fn points_bounds(points: impl IntoIterator<Item = Point>) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut any = false;
    for p in points {
        any = true;
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    if !any {
        return Rect::NONE;
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Distance from `p` to the segment `a`–`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 <= f64::EPSILON {
        return p.distance(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

/// Distance from `p` to the polyline through `points`.
pub fn distance_to_polyline(p: Point, points: &[Point]) -> f64 {
    match points {
        [] => f64::INFINITY,
        [one] => p.distance(*one),
        _ => points
            .windows(2)
            .map(|w| distance_to_segment(p, w[0], w[1]))
            .fold(f64::INFINITY, f64::min),
    }
}

/// Whether `p` is inside the polygon `points` (even-odd rule).
pub fn inside_polygon(p: Point, points: &[Point]) -> bool {
    let mut inside = false;
    let n = points.len();
    for i in 0..n {
        let (a, b) = (points[i], points[(i + n - 1) % n]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// `to` moved onto the nearest of the 0/45/90° directions from `from` (Shift while drawing an
/// arrow or line, spec "Arrow": "Shift constrains to 0/45/90°"); the distance along that
/// direction is the pointer's projection.
pub fn constrain_45(from: Point, to: Point) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx == 0.0 && dy == 0.0 {
        return to;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (dy.atan2(dx) / step).round() * step;
    let (sin, cos) = angle.sin_cos();
    let length = dx * cos + dy * sin;
    Point::new(from.x + length * cos, from.y + length * sin)
}

/// Angle of the line `from`–`to` in degrees, as the constraint chip shows it: counter-clockwise
/// from the positive x axis (screen convention: up is positive), in [0, 360).
pub fn chip_angle(from: Point, to: Point) -> f64 {
    let a = (-(to.y - from.y)).atan2(to.x - from.x).to_degrees();
    let a = a.round().rem_euclid(360.0);
    if a >= 359.5 {
        0.0
    } else {
        a
    }
}

/// Maps `p` from the upright box `from` into the placement `to` (its rectangle, then its
/// rotation around its centre). A side of zero length keeps the point's offset from that side's
/// centre line, so straight horizontal or vertical strokes survive a resize.
pub fn map_point(p: Point, from: Rect, to: &Arrange) -> Point {
    let axis = |v: f64, start: f64, size: f64, new_start: f64, new_size: f64| {
        if size > f64::EPSILON {
            new_start + (v - start) / size * new_size
        } else {
            new_start + new_size / 2.0 + (v - start - size / 2.0)
        }
    };
    let local = Point::new(
        axis(p.x, from.x, from.width, to.rect.x, to.rect.width),
        axis(p.y, from.y, from.height, to.rect.y, to.rect.height),
    );
    to.to_document(local)
}

/// The eight resize handles of `arrange`, without the rotation handle (pen strokes, mockup 05).
pub fn resize_handles(arrange: &Arrange) -> Vec<Handle> {
    box_handles(arrange)
        .into_iter()
        .filter(|h| h.id != HandleId::Rotate)
        .collect()
}

/// The four corner handles of `arrange` (step markers: they only scale).
pub fn corner_handles(arrange: &Arrange) -> Vec<Handle> {
    box_handles(arrange)
        .into_iter()
        .filter(|h| {
            matches!(
                h.id,
                HandleId::Resize(Compass::NE | Compass::NW | Compass::SE | Compass::SW)
            )
        })
        .collect()
}

/// Maps the style sizes of a whole-document transform ("Scale annotations").
pub fn style_scaled(value: f64, t: &Transform) -> f64 {
    let s = t.style_scale;
    if s.is_finite() && s > 0.0 {
        value * s
    } else {
        value
    }
}

/// Scales a shadow's distance and blur for a whole-document transform.
pub fn shadow_scaled(shadow: &mut Shadow, t: &Transform) {
    shadow.distance = style_scaled(shadow.distance, t);
    shadow.blur = style_scaled(shadow.blur, t);
}

/// One of a fixed set of choices by its key.
pub fn choice<T>(
    key: &str,
    value: &PropertyValue,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, PropertyError> {
    match value {
        PropertyValue::Choice(k) => parse(k).ok_or_else(|| PropertyError::Range(key.to_string())),
        _ => Err(PropertyError::Type(key.to_string())),
    }
}

/// Text property.
pub fn text(key: &str, value: &PropertyValue) -> Result<String, PropertyError> {
    match value {
        PropertyValue::Text(t) => Ok(t.clone()),
        _ => Err(PropertyError::Type(key.to_string())),
    }
}

/// Defines an enum of choices with stable keys (serialised as those keys).
macro_rules! choices {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $key:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $key)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// Stable key, as stored and as `PropertyValue::Choice`.
            pub fn key(self) -> &'static str {
                match self {
                    $($name::$variant => $key),+
                }
            }

            pub fn from_key(key: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|v| v.key() == key)
            }
        }
    };
}
pub(crate) use choices;

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn constraint_snaps_to_45_degree_steps() {
        let o = Point::new(100.0, 100.0);
        assert!(close(
            constrain_45(o, Point::new(200.0, 108.0)),
            Point::new(200.0, 100.0)
        ));
        assert!(close(
            constrain_45(o, Point::new(103.0, 30.0)),
            Point::new(100.0, 30.0)
        ));
        let d = constrain_45(o, Point::new(190.0, 210.0));
        assert!((d.x - 100.0 - (d.y - 100.0)).abs() < 1e-9, "45°: {d:?}");
        assert_eq!(chip_angle(o, Point::new(200.0, 100.0)), 0.0);
        assert_eq!(chip_angle(o, Point::new(100.0, 0.0)), 90.0);
        assert_eq!(chip_angle(o, Point::new(0.0, 200.0)), 225.0);
    }

    #[test]
    fn polygon_and_segment_distances() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        assert!(inside_polygon(Point::new(5.0, 5.0), &square));
        assert!(!inside_polygon(Point::new(15.0, 5.0), &square));
        assert_eq!(
            distance_to_segment(Point::new(5.0, 3.0), square[0], square[1]),
            3.0
        );
        assert_eq!(
            distance_to_polyline(Point::new(13.0, 14.0), &square[..3]),
            5.0
        );
    }

    #[test]
    fn points_map_between_boxes() {
        let from = Rect::new(0.0, 0.0, 100.0, 0.0);
        let to = Arrange::new(Rect::new(10.0, 10.0, 200.0, 0.0), 0.0);
        // A flat box: x scales, y keeps its place on the centre line.
        assert!(close(
            map_point(Point::new(50.0, 0.0), from, &to),
            Point::new(110.0, 10.0)
        ));
        let turned = Arrange::new(Rect::new(0.0, 0.0, 100.0, 0.0), 90.0);
        assert!(close(
            map_point(Point::new(100.0, 0.0), from, &turned),
            Point::new(50.0, 50.0)
        ));
    }
}
