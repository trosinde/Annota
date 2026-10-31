//! Freehand pen strokes (spec annotation-tools: Pen; mockup 05).
//!
//! A stroke is stored as the points the pointer went through, in document pixels — a vector
//! path, not pixels. Width, colour, opacity and smoothing are applied only when it is drawn, so
//! changing the width later re-renders the same path at the new width (spec "Editable stroke").
//! Smoothing 0–100 averages neighbouring points (after spacing them evenly, so straight
//! Shift-segments of two far-apart points keep their shape) and the path is drawn through the
//! midpoints of the smoothed points with quadratic curves.

use serde::{Deserialize, Serialize};

use super::common::{
    distance_to_polyline, map_point, points_bounds, resize_handles, shadow_scaled, style_scaled,
    styled, with_shadow,
};
use crate::document::annotation::{
    value, Annotation, Handle, Property, PropertyError, PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::Shadow;

pub const MIN_WIDTH: f64 = 1.0;
pub const MAX_WIDTH: f64 = 40.0;
/// Longest gap between points before smoothing, in document pixels.
const RESAMPLE: f64 = 6.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pen {
    /// The stroke as drawn, in document pixels.
    pub points: Vec<Point>,
    pub color: Rgba,
    pub width: f64,
    /// 0–100.
    pub smoothing: f64,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
}

impl Pen {
    pub const TYPE: &'static str = "pen";

    /// A stroke starting at `at` with the tool defaults (redline: Coral, 10 px, smoothing 60,
    /// shadow on).
    pub fn new(at: Point) -> Self {
        Self {
            points: vec![at],
            color: palette::CORAL,
            width: 10.0,
            smoothing: 60.0,
            opacity: 1.0,
            shadow: Shadow::DEFAULT,
        }
    }

    /// The points the stroke is drawn through: evenly spaced, then averaged with their
    /// neighbours according to `smoothing`. The ends stay where they are.
    pub fn smoothed(&self) -> Vec<Point> {
        let mut spaced: Vec<Point> = Vec::with_capacity(self.points.len());
        for &p in &self.points {
            if let Some(&last) = spaced.last() {
                let d = last.distance(p);
                let steps = (d / RESAMPLE).ceil().min(10_000.0) as usize;
                for k in 1..steps {
                    let t = k as f64 / steps as f64;
                    spaced.push(Point::new(
                        last.x + (p.x - last.x) * t,
                        last.y + (p.y - last.y) * t,
                    ));
                }
            }
            spaced.push(p);
        }
        let radius = (self.smoothing.clamp(0.0, 100.0) / 20.0).round() as usize;
        if radius == 0 || spaced.len() < 3 {
            return spaced;
        }
        let mut out = spaced.clone();
        for _ in 0..2 {
            let src = out.clone();
            let n = src.len();
            for (i, p) in out.iter_mut().enumerate().take(n - 1).skip(1) {
                let r = radius.min(i).min(n - 1 - i);
                let window = &src[i - r..=i + r];
                let k = window.len() as f64;
                *p = Point::new(
                    window.iter().map(|q| q.x).sum::<f64>() / k,
                    window.iter().map(|q| q.y).sum::<f64>() / k,
                );
            }
        }
        out
    }

    fn body_bounds(&self) -> Rect {
        points_bounds(self.smoothed()).inflate(self.width / 2.0 + 1.0)
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        let pts = self.smoothed();
        self.color.set_source(cr);
        match pts.as_slice() {
            [] => Ok(()),
            [one] => {
                cr.new_sub_path();
                cr.arc(one.x, one.y, self.width / 2.0, 0.0, std::f64::consts::TAU);
                cr.fill()
            }
            [first, rest @ ..] => {
                cr.set_line_width(self.width);
                cr.set_line_cap(cairo::LineCap::Round);
                cr.set_line_join(cairo::LineJoin::Round);
                cr.move_to(first.x, first.y);
                let mut from = *first;
                for pair in pts.windows(2).skip(1) {
                    // Quadratic through the midpoints, the point as control.
                    let (c, next) = (pair[0], pair[1]);
                    let to = Point::new((c.x + next.x) / 2.0, (c.y + next.y) / 2.0);
                    cr.curve_to(
                        from.x + 2.0 / 3.0 * (c.x - from.x),
                        from.y + 2.0 / 3.0 * (c.y - from.y),
                        to.x + 2.0 / 3.0 * (c.x - to.x),
                        to.y + 2.0 / 3.0 * (c.y - to.y),
                        to.x,
                        to.y,
                    );
                    from = to;
                }
                if let Some(last) = rest.last() {
                    cr.line_to(last.x, last.y);
                }
                cr.stroke()
            }
        }
    }
}

impl Annotation for Pen {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        distance_to_polyline(point, &self.smoothed()) <= self.width / 2.0 + tolerance.max(0.0)
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        Arrange::new(points_bounds(self.points.iter().copied()), 0.0)
    }

    fn set_arrange(&mut self, arrange: Arrange) {
        let from = points_bounds(self.points.iter().copied());
        if from.is_none() {
            return;
        }
        for p in &mut self.points {
            *p = map_point(*p, from, &arrange);
        }
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        for p in &mut self.points {
            p.x += dx;
            p.y += dy;
        }
    }

    fn transform(&mut self, t: &Transform) {
        for p in &mut self.points {
            *p = t.apply(*p);
        }
        self.width = style_scaled(self.width, t).clamp(MIN_WIDTH, MAX_WIDTH);
        shadow_scaled(&mut self.shadow, t);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("color", PropertyValue::Color(self.color)),
            Property::new("width", PropertyValue::Number(self.width)),
            Property::new("smoothing", PropertyValue::Number(self.smoothing)),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "color" => self.color = value::color(key, v)?,
            "width" => self.width = value::number(key, v, MIN_WIDTH..=MAX_WIDTH)?,
            "smoothing" => self.smoothing = value::number(key, v, 0.0..=100.0)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    /// Eight resize handles, no rotation handle (mockup 05).
    fn handles(&self) -> Vec<Handle> {
        resize_handles(&self.arrange())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::{Compass, HandleId, Modifiers, Target};
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    fn wave() -> Pen {
        let mut pen = Pen::new(Point::new(20.0, 50.0));
        for i in 1..=40 {
            let x = 20.0 + f64::from(i) * 4.0;
            pen.points
                .push(Point::new(x, 50.0 + 15.0 * (f64::from(i) * 0.3).sin()));
        }
        pen.shadow.enabled = false;
        pen
    }

    fn ink(out: &Bitmap, x0: i32, x1: i32, y: i32) -> usize {
        (x0..x1)
            .filter(|&x| out.pixel(x, y).is_some_and(|p| p.a > 0))
            .count()
    }

    #[test]
    fn changing_the_width_re_renders_the_same_path() {
        // Spec "Editable stroke": the stroke is a vector path; a new width redraws it.
        let doc = Document::new(Bitmap::filled(200, 100, Rgba::TRANSPARENT).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        let mut pen = wave();
        let thin = Bitmap::draw(200, 100, |cr| pen.render(cr, &ctx)).unwrap();
        let path = pen.points.clone();
        pen.set_property("width", &PropertyValue::Number(30.0))
            .unwrap();
        assert_eq!(pen.points, path, "the path is unchanged");
        let thick = Bitmap::draw(200, 100, |cr| pen.render(cr, &ctx)).unwrap();
        // Down a column through a crest of the wave: 30 px of ink instead of about 10.
        let column = |out: &Bitmap| {
            (0..100)
                .filter(|&y| out.pixel(83, y).is_some_and(|p| p.a > 128))
                .count()
        };
        assert!((8..=14).contains(&column(&thin)), "{}", column(&thin));
        assert!((27..=36).contains(&column(&thick)), "{}", column(&thick));
        assert!(ink(&thick, 0, 200, 50) > 0);
        assert!(pen.bounds().height > 40.0);
    }

    #[test]
    fn smoothing_rounds_corners_but_keeps_ends() {
        let mut pen = Pen::new(Point::new(0.0, 0.0));
        pen.points.push(Point::new(100.0, 0.0));
        pen.points.push(Point::new(100.0, 100.0));
        pen.smoothing = 0.0;
        assert!(pen.smoothed().contains(&Point::new(100.0, 0.0)));
        pen.smoothing = 100.0;
        let s = pen.smoothed();
        assert_eq!(s.first(), Some(&Point::new(0.0, 0.0)));
        assert_eq!(s.last(), Some(&Point::new(100.0, 100.0)));
        assert!(
            !s.contains(&Point::new(100.0, 0.0)),
            "the corner is rounded"
        );
        // A long straight segment stays straight away from its corners.
        assert!(s
            .iter()
            .any(|p| (p.y).abs() < 1e-9 && p.x > 20.0 && p.x < 60.0));
    }

    #[test]
    fn resizing_maps_the_points() {
        let mut pen = wave();
        let a = pen.arrange();
        pen.drag_handle(
            HandleId::Resize(Compass::E),
            Point::new(a.rect.right() + a.rect.width, a.rect.centre().y),
            Modifiers::default(),
        );
        let b = pen.arrange();
        assert!((b.rect.width - 2.0 * a.rect.width).abs() < 1e-9);
        assert_eq!(b.rect.x, a.rect.x);
        assert!(pen.handles().iter().all(|h| h.id != HandleId::Rotate));
        assert_eq!(pen.handles().len(), 8);
    }

    #[test]
    fn a_click_is_a_dot() {
        let pen = Pen::new(Point::new(10.0, 10.0));
        assert!(pen.hit_test(Point::new(13.0, 10.0), 0.0));
        assert!(!pen.hit_test(Point::new(16.0, 10.0), 0.0));
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut pen = wave();
        let before = pen.clone();
        for p in pen.clone().properties() {
            pen.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(pen, before);
        assert!(pen
            .set_property("smoothing", &PropertyValue::Number(101.0))
            .is_err());
        assert!(pen
            .set_property("radius", &PropertyValue::Number(1.0))
            .is_err());
    }
}
