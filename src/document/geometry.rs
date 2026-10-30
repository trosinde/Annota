//! Document-space geometry. One document unit is one pixel of the original image.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Point) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// This point rotated by `degrees` (clockwise on screen, y points down) around `centre`.
    pub fn rotated_around(self, centre: Point, degrees: f64) -> Point {
        if degrees == 0.0 {
            return self;
        }
        let (sin, cos) = degrees.to_radians().sin_cos();
        let dx = self.x - centre.x;
        let dy = self.y - centre.y;
        Point::new(
            centre.x + dx * cos - dy * sin,
            centre.y + dx * sin + dy * cos,
        )
    }
}

/// An axis-aligned rectangle. A negative width or height means "occupies nothing"
/// ([`Rect::NONE`]); zero-sized rectangles are valid (a point or a straight line).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const NONE: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: -1.0,
        height: -1.0,
    };

    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_points(a: Point, b: Point) -> Self {
        Self::new(
            a.x.min(b.x),
            a.y.min(b.y),
            (a.x - b.x).abs(),
            (a.y - b.y).abs(),
        )
    }

    /// True for [`Rect::NONE`] and for rectangles with non-finite coordinates.
    pub fn is_none(&self) -> bool {
        self.width < 0.0
            || self.height < 0.0
            || !(self.x.is_finite()
                && self.y.is_finite()
                && self.width.is_finite()
                && self.height.is_finite())
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn centre(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, p: Point) -> bool {
        !self.is_none()
            && p.x >= self.x
            && p.x <= self.right()
            && p.y >= self.y
            && p.y <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        !self.is_none()
            && !other.is_none()
            && self.x <= other.right()
            && other.x <= self.right()
            && self.y <= other.bottom()
            && other.y <= self.bottom()
    }

    /// Grown by `d` on every side.
    pub fn inflate(&self, d: f64) -> Rect {
        if self.is_none() {
            return *self;
        }
        Rect::new(
            self.x - d,
            self.y - d,
            (self.width + 2.0 * d).max(0.0),
            (self.height + 2.0 * d).max(0.0),
        )
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_none() {
            return *other;
        }
        if other.is_none() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }

    pub fn corners(&self) -> [Point; 4] {
        [
            Point::new(self.x, self.y),
            Point::new(self.right(), self.y),
            Point::new(self.right(), self.bottom()),
            Point::new(self.x, self.bottom()),
        ]
    }

    /// Bounding box of this rectangle rotated by `degrees` around its centre.
    pub fn rotated_bounds(&self, degrees: f64) -> Rect {
        if degrees == 0.0 || self.is_none() {
            return *self;
        }
        let c = self.centre();
        let pts = self.corners().map(|p| p.rotated_around(c, degrees));
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in pts {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
}

/// An integer rectangle in document pixels (canvas, crop).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn to_rect(self) -> Rect {
        Rect::new(
            f64::from(self.x),
            f64::from(self.y),
            f64::from(self.width),
            f64::from(self.height),
        )
    }
}

/// Generic placement every object exposes for the sidebar's Arrange group and the selection
/// box: an unrotated rectangle plus a clockwise rotation in degrees around its centre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrange {
    pub rect: Rect,
    pub rotation: f64,
}

impl Arrange {
    pub fn new(rect: Rect, rotation: f64) -> Self {
        Self { rect, rotation }
    }

    /// Converts a document point into the object's unrotated frame.
    pub fn to_local(&self, p: Point) -> Point {
        p.rotated_around(self.rect.centre(), -self.rotation)
    }

    /// Converts a point of the object's unrotated frame into document space.
    pub fn to_document(&self, p: Point) -> Point {
        p.rotated_around(self.rect.centre(), self.rotation)
    }

    pub fn bounds(&self) -> Rect {
        self.rect.rotated_bounds(self.rotation)
    }
}

/// Angle normalised to (-180, 180].
pub fn normalize_degrees(degrees: f64) -> f64 {
    let mut d = degrees % 360.0;
    if d <= -180.0 {
        d += 360.0;
    } else if d > 180.0 {
        d -= 360.0;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn rotation_is_clockwise_on_screen() {
        let p = Point::new(10.0, 0.0).rotated_around(Point::default(), 90.0);
        assert!(close(p.x, 0.0) && close(p.y, 10.0));
    }

    #[test]
    fn rotated_bounds_of_square_at_45_degrees() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0).rotated_bounds(45.0);
        let d = 10.0 * std::f64::consts::SQRT_2;
        assert!(close(r.width, d) && close(r.height, d));
        assert!(close(r.centre().x, 5.0) && close(r.centre().y, 5.0));
    }

    #[test]
    fn none_rect_never_intersects() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(!Rect::NONE.intersects(&r));
        assert!(!r.intersects(&Rect::NONE));
        assert_eq!(Rect::NONE.union(&r), r);
        assert!(Rect::new(f64::NAN, 0.0, 1.0, 1.0).is_none());
        // Zero-sized rectangles are points, not "none".
        assert!(Rect::new(5.0, 5.0, 0.0, 0.0).intersects(&r));
    }

    #[test]
    fn local_and_document_frames_are_inverse() {
        let a = Arrange::new(Rect::new(10.0, 20.0, 40.0, 30.0), 33.0);
        let p = Point::new(17.0, -4.0);
        let back = a.to_document(a.to_local(p));
        assert!(close(back.x, p.x) && close(back.y, p.y));
    }

    #[test]
    fn degrees_normalise() {
        assert_eq!(normalize_degrees(190.0), -170.0);
        assert_eq!(normalize_degrees(-180.0), 180.0);
        assert_eq!(normalize_degrees(540.0), 180.0);
        assert_eq!(normalize_degrees(15.0), 15.0);
    }
}
