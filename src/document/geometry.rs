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

    /// The overlap of both rectangles, `None` if they do not overlap.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if self.is_none() || other.is_none() {
            return None;
        }
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (x1 >= x0 && y1 >= y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
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

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height)
    }

    /// The overlap with `other`, `None` unless it covers at least one pixel.
    pub fn intersection(&self, other: &IRect) -> Option<IRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (x1 > x0 && y1 > y0).then(|| IRect::new(x0, y0, x1 - x0, y1 - y0))
    }

    /// The smallest rectangle around both.
    pub fn union(&self, other: &IRect) -> IRect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        IRect::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn translate(&self, dx: i32, dy: i32) -> IRect {
        IRect::new(
            self.x.saturating_add(dx),
            self.y.saturating_add(dy),
            self.width,
            self.height,
        )
    }

    /// The whole pixels covering `r` (outward).
    pub fn covering(r: &Rect) -> IRect {
        let x0 = r.x.floor() as i32;
        let y0 = r.y.floor() as i32;
        let x1 = r.right().ceil() as i32;
        let y1 = r.bottom().ceil() as i32;
        IRect::new(x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
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

/// Mirror axis of a [`Transform`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mirror {
    /// Left and right swap (`x → −x`): Flip Horizontally.
    Horizontal,
    /// Top and bottom swap (`y → −y`): Flip Vertically.
    Vertical,
}

/// A transform of the whole document, applied by the image operations (crop, rotate by 90°,
/// flip, resize) to the canvas and to every object, so annotations stay on the image pixels they
/// belong to. Applied in this order: scale (`scale_x`, `scale_y`), mirror, `quarter_turns`
/// clockwise quarter turns around the origin, then translation by (`dx`, `dy`).
///
/// Only axis-preserving maps exist, so a box stays a box: exactly for translations, quarter
/// turns, mirrors and uniform scaling; a rotated box under non-uniform scaling becomes a
/// parallelogram, which [`Transform::map_arrange`] approximates by scaling along the box's own
/// axes (like `Group::set_arrange`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub scale_x: f64,
    pub scale_y: f64,
    pub mirror: Option<Mirror>,
    /// Clockwise quarter turns, 0–3.
    pub quarter_turns: u8,
    pub dx: f64,
    pub dy: f64,
    /// Factor for sizes that are style rather than geometry — line and border widths, text
    /// sizes, shadow distances, corner radii. 1 keeps them ("Scale annotations" off).
    pub style_scale: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        scale_x: 1.0,
        scale_y: 1.0,
        mirror: None,
        quarter_turns: 0,
        dx: 0.0,
        dy: 0.0,
        style_scale: 1.0,
    };

    pub fn translation(dx: f64, dy: f64) -> Self {
        Self {
            dx,
            dy,
            ..Self::IDENTITY
        }
    }

    /// Only a translation (crop): objects can then move exactly with `Annotation::translate`.
    pub fn is_translation(&self) -> bool {
        self.scale_x == 1.0
            && self.scale_y == 1.0
            && self.mirror.is_none()
            && self.quarter_turns % 4 == 0
            && self.style_scale == 1.0
    }

    /// Everything but the translation, applied to a vector.
    fn linear(&self, x: f64, y: f64) -> (f64, f64) {
        let (mut x, mut y) = (x * self.scale_x, y * self.scale_y);
        match self.mirror {
            Some(Mirror::Horizontal) => x = -x,
            Some(Mirror::Vertical) => y = -y,
            None => {}
        }
        for _ in 0..self.quarter_turns % 4 {
            // A clockwise quarter turn on screen (y points down).
            (x, y) = (-y, x);
        }
        (x, y)
    }

    /// As an affine matrix `[xx, yx, xy, yy, x0, y0]` (cairo's order): `x' = xx·x + xy·y + x0`,
    /// `y' = yx·x + yy·y + y0`.
    pub fn matrix(&self) -> [f64; 6] {
        let (xx, yx) = self.linear(1.0, 0.0);
        let (xy, yy) = self.linear(0.0, 1.0);
        [xx, yx, xy, yy, self.dx, self.dy]
    }

    pub fn apply(&self, p: Point) -> Point {
        let (x, y) = self.linear(p.x, p.y);
        Point::new(x + self.dx, y + self.dy)
    }

    /// Bounding box of the mapped corners of `r` (exact: the map is axis-preserving).
    pub fn map_rect(&self, r: &Rect) -> Rect {
        if r.is_none() {
            return *r;
        }
        let pts = r.corners().map(|p| self.apply(p));
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in pts {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// [`Transform::map_rect`] for whole pixels (the canvas): rounded to the nearest pixel.
    pub fn map_irect(&self, r: IRect) -> IRect {
        let m = self.map_rect(&r.to_rect());
        let x0 = m.x.round() as i32;
        let y0 = m.y.round() as i32;
        let x1 = m.right().round() as i32;
        let y1 = m.bottom().round() as i32;
        IRect::new(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1))
    }

    /// The new orientation of something turned by `degrees`. A mirror cannot be expressed as a
    /// rotation: the angle is negated (the reflection of a turn by `r` is a turn by `−r`), and
    /// the content is left as it is unless the type mirrors it itself (`Image` does; text and
    /// stamps keep reading the right way round).
    pub fn map_rotation(&self, degrees: f64) -> f64 {
        // Direction of the box's width axis after scaling, then mirrored and turned.
        let (sin, cos) = degrees.to_radians().sin_cos();
        let mut angle = (sin * self.scale_y).atan2(cos * self.scale_x).to_degrees();
        if self.mirror.is_some() {
            angle = -angle;
        }
        normalize_degrees(angle + 90.0 * f64::from(self.quarter_turns % 4))
    }

    /// Maps a box: its centre exactly, its size scaled along its own axes, its rotation by
    /// [`Transform::map_rotation`].
    pub fn map_arrange(&self, a: &Arrange) -> Arrange {
        let c = self.apply(a.rect.centre());
        let (sin, cos) = a.rotation.to_radians().sin_cos();
        let (sx, sy) = (self.scale_x.abs(), self.scale_y.abs());
        let w = a.rect.width * (sx * cos).hypot(sy * sin);
        let h = a.rect.height * (sx * sin).hypot(sy * cos);
        Arrange::new(
            Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h),
            self.map_rotation(a.rotation),
        )
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
    fn quarter_turn_maps_the_image_onto_itself_turned() {
        // Rotate Right of a 100 × 40 image: (x, y) → (40 − y, x); the image becomes 40 × 100.
        let t = Transform {
            quarter_turns: 1,
            dx: 40.0,
            ..Transform::IDENTITY
        };
        let p = t.apply(Point::new(10.0, 5.0));
        assert!(close(p.x, 35.0) && close(p.y, 10.0));
        assert_eq!(
            t.map_irect(IRect::new(0, 0, 100, 40)),
            IRect::new(0, 0, 40, 100)
        );
        // An extended canvas turns with it.
        assert_eq!(
            t.map_irect(IRect::new(-20, 0, 120, 60)),
            IRect::new(-20, -20, 60, 120)
        );
        let a = t.map_arrange(&Arrange::new(Rect::new(10.0, 10.0, 20.0, 10.0), 15.0));
        assert!(close(a.rotation, 105.0));
        assert!(close(a.rect.width, 20.0) && close(a.rect.height, 10.0));
        assert!(close(a.rect.centre().x, 25.0) && close(a.rect.centre().y, 20.0));
    }

    #[test]
    fn mirror_negates_angles_and_keeps_sizes() {
        let t = Transform {
            mirror: Some(Mirror::Horizontal),
            dx: 100.0,
            ..Transform::IDENTITY
        };
        let a = t.map_arrange(&Arrange::new(Rect::new(10.0, 10.0, 20.0, 10.0), 30.0));
        assert!(close(a.rotation, -30.0));
        assert!(close(a.rect.centre().x, 80.0) && close(a.rect.centre().y, 15.0));
        assert!(close(a.rect.width, 20.0) && close(a.rect.height, 10.0));
        let v = Transform {
            mirror: Some(Mirror::Vertical),
            dy: 40.0,
            ..Transform::IDENTITY
        };
        assert!(close(v.map_rotation(30.0), -30.0));
        assert!(close(v.apply(Point::new(3.0, 4.0)).y, 36.0));
    }

    #[test]
    fn scaling_scales_boxes_along_their_axes() {
        let t = Transform {
            scale_x: 2.0,
            scale_y: 0.5,
            ..Transform::IDENTITY
        };
        let a = t.map_arrange(&Arrange::new(Rect::new(10.0, 10.0, 20.0, 10.0), 0.0));
        assert_eq!(a, Arrange::new(Rect::new(20.0, 5.0, 40.0, 5.0), 0.0));
        // A quarter-turned box: its width runs along y, so it shrinks with scale_y.
        let a = t.map_arrange(&Arrange::new(Rect::new(0.0, 0.0, 20.0, 10.0), 90.0));
        assert!(close(a.rect.width, 10.0) && close(a.rect.height, 20.0));
        assert!(close(a.rotation, 90.0));
        assert!(!t.is_translation() && Transform::translation(3.0, 4.0).is_translation());
    }

    #[test]
    fn degrees_normalise() {
        assert_eq!(normalize_degrees(190.0), -170.0);
        assert_eq!(normalize_degrees(-180.0), 180.0);
        assert_eq!(normalize_degrees(540.0), 180.0);
        assert_eq!(normalize_degrees(15.0), 15.0);
    }
}
