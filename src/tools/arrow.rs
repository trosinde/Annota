//! Arrows and lines (spec annotation-tools: Arrow, Line; mockups 02–04).
//!
//! One type serves both: a line is an arrow without default end styles, saved under its own tag
//! (`"line"`), so "Select All Lines" and "Select All Arrows" stay apart. The geometry is three
//! points — the two ends and the point the line passes halfway (`middle`, the curve handle
//! drawn at t = 0.5, redline "On-canvas handles") — so the line is a quadratic Bézier that
//! "bends smoothly through the handle". A straight line has its middle halfway between the
//! ends. Dragging an end keeps the bend relative to the ends; Shift constrains an end to
//! 0/45/90° from the other.
//!
//! Head proportions follow the mockups' SVG (03: a 6 px line has a 20 × 16 px triangle; 02: an
//! 8 px line a 26 × 21 px one) and scale with the end size (100 % = 1.0).

use serde::{Deserialize, Serialize};

use super::common::{
    self, choices, constrain_45, distance_to_polyline, map_point, points_bounds, shadow_scaled,
    style_scaled, styled, with_shadow,
};
use crate::document::annotation::{
    value, Annotation, Handle, HandleId, HandleKind, Modifiers, Property, PropertyError,
    PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::{LineStyle, Shadow};

/// Line width range (spec: 1–40 px).
pub const MIN_WIDTH: f64 = 1.0;
pub const MAX_WIDTH: f64 = 40.0;
/// End size range (100 % = 1.0).
pub const MIN_END_SIZE: f64 = 0.25;
pub const MAX_END_SIZE: f64 = 4.0;

/// Handle ids.
pub const START: HandleId = HandleId::Custom(0);
pub const END: HandleId = HandleId::Custom(1);
pub const CURVE: HandleId = HandleId::Custom(2);

/// Points sampled along the curve for hit-testing and bounds.
const SAMPLES: usize = 32;

choices!(
    /// What an end of the line looks like (mockup 02's end-style popover).
    EndStyle {
        None => "none",
        Triangle => "triangle",
        Open => "open",
        Circle => "circle",
        Square => "square",
        Bar => "bar",
    }
);

/// Which of the two tools made the object; decides the saved type tag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArrowKind {
    #[default]
    Arrow,
    Line,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arrow {
    pub start: Point,
    pub end: Point,
    /// Where the line passes halfway (t = 0.5); halfway between the ends when it is straight.
    pub middle: Point,
    pub color: Rgba,
    pub width: f64,
    pub start_style: EndStyle,
    pub end_style: EndStyle,
    /// Size of the start's head, 1.0 = 100 %.
    pub start_size: f64,
    pub end_size: f64,
    pub line_style: LineStyle,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
    /// Arrow or line; not stored (the type tag says it).
    #[serde(skip)]
    pub kind: ArrowKind,
}

fn midpoint(a: Point, b: Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

fn unit(dx: f64, dy: f64) -> Option<(f64, f64)> {
    let len = dx.hypot(dy);
    (len > 1e-9).then(|| (dx / len, dy / len))
}

impl Arrow {
    pub const TYPE: &'static str = "arrow";
    pub const LINE_TYPE: &'static str = "line";

    /// A new arrow with the tool defaults (redline: Coral, 8 px, start None, end Triangle,
    /// ends 100 %, solid, opacity 100 %, shadow on).
    pub fn new(start: Point, end: Point) -> Self {
        Self {
            start,
            end,
            middle: midpoint(start, end),
            color: palette::CORAL,
            width: 8.0,
            start_style: EndStyle::None,
            end_style: EndStyle::Triangle,
            start_size: 1.0,
            end_size: 1.0,
            line_style: LineStyle::Solid,
            opacity: 1.0,
            shadow: Shadow::DEFAULT,
            kind: ArrowKind::Arrow,
        }
    }

    /// A new line with the tool defaults (redline: Coral, 6 px, ends None, solid, shadow on).
    pub fn line(start: Point, end: Point) -> Self {
        Self {
            width: 6.0,
            end_style: EndStyle::None,
            kind: ArrowKind::Line,
            ..Self::new(start, end)
        }
    }

    /// Serde decoder for the `"line"` tag.
    pub(crate) fn decode_line(
        value: serde_json::Value,
    ) -> Result<Box<dyn Annotation>, serde_json::Error> {
        let mut line: Arrow = serde_json::from_value(value)?;
        line.kind = ArrowKind::Line;
        Ok(Box::new(line))
    }

    /// The quadratic Bézier's control point (the curve passes `middle` at t = 0.5).
    pub fn control(&self) -> Point {
        let m = midpoint(self.start, self.end);
        Point::new(2.0 * self.middle.x - m.x, 2.0 * self.middle.y - m.y)
    }

    fn at(&self, t: f64) -> Point {
        let c = self.control();
        let u = 1.0 - t;
        Point::new(
            u * u * self.start.x + 2.0 * u * t * c.x + t * t * self.end.x,
            u * u * self.start.y + 2.0 * u * t * c.y + t * t * self.end.y,
        )
    }

    /// Points along the line, start to end.
    pub fn samples(&self) -> Vec<Point> {
        (0..=SAMPLES)
            .map(|i| self.at(i as f64 / SAMPLES as f64))
            .collect()
    }

    /// Unit direction in which the line runs into `end` (or out of `start`, reversed).
    fn direction_at(&self, end: bool) -> (f64, f64) {
        let c = self.control();
        let (from, to) = if end { (c, self.end) } else { (c, self.start) };
        unit(to.x - from.x, to.y - from.y)
            .or_else(|| {
                let (a, b) = if end {
                    (self.start, self.end)
                } else {
                    (self.end, self.start)
                };
                unit(b.x - a.x, b.y - a.y)
            })
            .unwrap_or((1.0, 0.0))
    }

    /// The middle as (along, across) the line from start to end, in units of its length, so
    /// moving an end keeps the bend.
    fn bend(&self) -> (f64, f64) {
        let (vx, vy) = (self.end.x - self.start.x, self.end.y - self.start.y);
        let len2 = vx * vx + vy * vy;
        if len2 <= 1e-12 {
            return (0.0, 0.0);
        }
        let m = midpoint(self.start, self.end);
        let (dx, dy) = (self.middle.x - m.x, self.middle.y - m.y);
        ((dx * vx + dy * vy) / len2, (dx * -vy + dy * vx) / len2)
    }

    fn set_bend(&mut self, (along, across): (f64, f64)) {
        let (vx, vy) = (self.end.x - self.start.x, self.end.y - self.start.y);
        let m = midpoint(self.start, self.end);
        self.middle = Point::new(
            m.x + along * vx - across * vy,
            m.y + along * vy + across * vx,
        );
    }

    fn style_of(&self, end: bool) -> (EndStyle, f64) {
        if end {
            (self.end_style, self.end_size)
        } else {
            (self.start_style, self.start_size)
        }
    }

    /// (length, half width) of a head.
    fn head_size(&self, size: f64) -> (f64, f64) {
        let w = self.width.max(0.0);
        ((3.0 * w + 2.0) * size, (1.2 * w + 0.8) * size)
    }

    /// How far a head reaches from its end point.
    fn head_reach(&self, end: bool) -> f64 {
        let (style, size) = self.style_of(end);
        let (length, half) = self.head_size(size);
        let w = self.width.max(0.0);
        match style {
            EndStyle::None => 0.0,
            EndStyle::Triangle | EndStyle::Open => length.hypot(half) + w / 2.0,
            EndStyle::Circle => (0.9 * w + 0.7) * size,
            EndStyle::Square => 1.6 * w * size * std::f64::consts::FRAC_1_SQRT_2,
            EndStyle::Bar => half + w / 2.0,
        }
    }

    /// The box around the line, not counting width, heads or shadow.
    fn path_bounds(&self) -> Rect {
        points_bounds(self.samples())
    }

    fn body_bounds(&self) -> Rect {
        let reach = (self.width / 2.0)
            .max(self.head_reach(false))
            .max(self.head_reach(true));
        self.path_bounds().inflate(reach + 1.0)
    }

    /// Draws one head at `tip`, the line running into it along `dir`.
    fn draw_head(
        &self,
        cr: &cairo::Context,
        tip: Point,
        dir: (f64, f64),
        end: bool,
    ) -> Result<(), cairo::Error> {
        let (style, size) = self.style_of(end);
        let (length, half) = self.head_size(size);
        let w = self.width.max(0.0);
        let (dx, dy) = dir;
        let (px, py) = (-dy, dx);
        let back = Point::new(tip.x - dx * length, tip.y - dy * length);
        cr.set_dash(&[], 0.0);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        match style {
            EndStyle::None => Ok(()),
            EndStyle::Triangle => {
                cr.move_to(tip.x, tip.y);
                cr.line_to(back.x + px * half, back.y + py * half);
                cr.line_to(back.x - px * half, back.y - py * half);
                cr.close_path();
                cr.set_line_width(0.4 * w);
                cr.fill_preserve()?;
                cr.stroke()
            }
            EndStyle::Open => {
                cr.move_to(back.x + px * half, back.y + py * half);
                cr.line_to(tip.x, tip.y);
                cr.line_to(back.x - px * half, back.y - py * half);
                cr.set_line_width(w);
                cr.stroke()
            }
            EndStyle::Circle => {
                cr.new_sub_path();
                cr.arc(
                    tip.x,
                    tip.y,
                    (0.9 * w + 0.7) * size,
                    0.0,
                    std::f64::consts::TAU,
                );
                cr.fill()
            }
            EndStyle::Square => {
                let a = 1.6 * w * size / 2.0;
                let corner = |s: f64, t: f64| {
                    Point::new(
                        tip.x + dx * a * s + px * a * t,
                        tip.y + dy * a * s + py * a * t,
                    )
                };
                let pts = [
                    corner(1.0, 1.0),
                    corner(1.0, -1.0),
                    corner(-1.0, -1.0),
                    corner(-1.0, 1.0),
                ];
                cr.move_to(pts[0].x, pts[0].y);
                for p in &pts[1..] {
                    cr.line_to(p.x, p.y);
                }
                cr.close_path();
                cr.fill()
            }
            EndStyle::Bar => {
                cr.move_to(tip.x + px * half, tip.y + py * half);
                cr.line_to(tip.x - px * half, tip.y - py * half);
                cr.set_line_width(w);
                cr.stroke()
            }
        }
    }

    /// Where the line itself stops at an end: a filled triangle covers the line, so the line
    /// ends inside it and its round cap does not poke through the tip.
    fn line_end(&self, end: bool) -> Point {
        let (point, (dx, dy)) = if end {
            (self.end, self.direction_at(true))
        } else {
            (self.start, self.direction_at(false))
        };
        let (style, size) = self.style_of(end);
        if style != EndStyle::Triangle {
            return point;
        }
        let (length, _) = self.head_size(size);
        let total = self.start.distance(self.end).max(1e-9);
        let back = (0.85 * length).min(total / 2.0);
        Point::new(point.x - dx * back, point.y - dy * back)
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        self.color.set_source(cr);
        let (s, e) = (self.line_end(false), self.line_end(true));
        let c = self.control();
        // The quadratic through `c` as a cubic.
        let c1 = Point::new(s.x + 2.0 / 3.0 * (c.x - s.x), s.y + 2.0 / 3.0 * (c.y - s.y));
        let c2 = Point::new(e.x + 2.0 / 3.0 * (c.x - e.x), e.y + 2.0 / 3.0 * (c.y - e.y));
        self.line_style.apply(cr, self.width);
        cr.move_to(s.x, s.y);
        cr.curve_to(c1.x, c1.y, c2.x, c2.y, e.x, e.y);
        cr.stroke()?;
        self.draw_head(cr, self.start, self.direction_at(false), false)?;
        self.draw_head(cr, self.end, self.direction_at(true), true)
    }

    /// Moves one end to `to` (Shift: 0/45/90° from the other end), keeping the bend.
    fn move_end(&mut self, end: bool, to: Point, modifiers: Modifiers) {
        let bend = self.bend();
        let other = if end { self.start } else { self.end };
        let to = if modifiers.shift {
            constrain_45(other, to)
        } else {
            to
        };
        if end {
            self.end = to;
        } else {
            self.start = to;
        }
        self.set_bend(bend);
    }

    /// Places the end while the line is being drawn (the start stays): straight, Shift
    /// constrains the angle.
    pub fn draw_to(&mut self, to: Point, modifiers: Modifiers) {
        self.end = if modifiers.shift {
            constrain_45(self.start, to)
        } else {
            to
        };
        self.middle = midpoint(self.start, self.end);
    }
}

impl Annotation for Arrow {
    fn type_name(&self) -> &'static str {
        match self.kind {
            ArrowKind::Arrow => Self::TYPE,
            ArrowKind::Line => Self::LINE_TYPE,
        }
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let tolerance = tolerance.max(0.0);
        if distance_to_polyline(point, &self.samples()) <= self.width / 2.0 + tolerance {
            return true;
        }
        [(false, self.start), (true, self.end)]
            .iter()
            .any(|&(end, p)| {
                let reach = self.head_reach(end);
                reach > 0.0 && point.distance(p) <= reach + tolerance
            })
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        Arrange::new(self.path_bounds(), 0.0)
    }

    fn set_arrange(&mut self, arrange: Arrange) {
        let from = self.path_bounds();
        if from.is_none() {
            return;
        }
        self.start = map_point(self.start, from, &arrange);
        self.end = map_point(self.end, from, &arrange);
        self.middle = map_point(self.middle, from, &arrange);
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        for p in [&mut self.start, &mut self.end, &mut self.middle] {
            p.x += dx;
            p.y += dy;
        }
    }

    fn transform(&mut self, t: &Transform) {
        self.start = t.apply(self.start);
        self.end = t.apply(self.end);
        self.middle = t.apply(self.middle);
        self.width = style_scaled(self.width, t).clamp(MIN_WIDTH, MAX_WIDTH);
        shadow_scaled(&mut self.shadow, t);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("color", PropertyValue::Color(self.color)),
            Property::new("width", PropertyValue::Number(self.width)),
            Property::new(
                "start_style",
                PropertyValue::Choice(self.start_style.key().into()),
            ),
            Property::new(
                "end_style",
                PropertyValue::Choice(self.end_style.key().into()),
            ),
            Property::new("start_size", PropertyValue::Number(self.start_size)),
            Property::new("end_size", PropertyValue::Number(self.end_size)),
            Property::new(
                "line_style",
                PropertyValue::Choice(self.line_style.key().into()),
            ),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "color" => self.color = value::color(key, v)?,
            "width" => self.width = value::number(key, v, MIN_WIDTH..=MAX_WIDTH)?,
            "start_style" => self.start_style = common::choice(key, v, EndStyle::from_key)?,
            "end_style" => self.end_style = common::choice(key, v, EndStyle::from_key)?,
            "start_size" => {
                self.start_size = value::number(key, v, MIN_END_SIZE..=MAX_END_SIZE)?;
            }
            "end_size" => self.end_size = value::number(key, v, MIN_END_SIZE..=MAX_END_SIZE)?,
            "line_style" => self.line_style = common::choice(key, v, LineStyle::from_key)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    /// The two ends (12 px white circles) and the curve handle on the line at t = 0.5 (12 px
    /// accent diamond); no box (redline "On-canvas handles").
    fn handles(&self) -> Vec<Handle> {
        vec![
            Handle::new(START, HandleKind::End, self.start),
            Handle::new(END, HandleKind::End, self.end),
            Handle::new(CURVE, HandleKind::Curve, self.middle),
        ]
    }

    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        match handle {
            START => self.move_end(false, to, modifiers),
            END => self.move_end(true, to, modifiers),
            CURVE => self.middle = to,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    fn close(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-6
    }

    #[test]
    fn defaults_follow_the_redline() {
        let a = Arrow::new(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        assert_eq!(a.type_name(), "arrow");
        assert_eq!((a.color, a.width), (palette::CORAL, 8.0));
        assert_eq!(
            (a.start_style, a.end_style),
            (EndStyle::None, EndStyle::Triangle)
        );
        assert_eq!((a.start_size, a.end_size, a.opacity), (1.0, 1.0, 1.0));
        assert_eq!(a.line_style, LineStyle::Solid);
        assert!(a.shadow.enabled);
        // Spec "Plain line": the line tool makes a line without arrowheads.
        let l = Arrow::line(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        assert_eq!(l.type_name(), "line");
        assert_eq!(
            (l.width, l.start_style, l.end_style),
            (6.0, EndStyle::None, EndStyle::None)
        );
    }

    #[test]
    fn curve_handle_bends_the_line_through_it() {
        // Spec "Curved arrow": dragging the middle handle bends the arrow through the handle.
        let mut a = Arrow::new(Point::new(0.0, 100.0), Point::new(200.0, 100.0));
        a.drag_handle(CURVE, Point::new(100.0, 40.0), Modifiers::default());
        assert!(close(a.at(0.5), Point::new(100.0, 40.0)));
        // Smooth: every sample lies above the chord, highest in the middle.
        let ys: Vec<f64> = a.samples().iter().map(|p| p.y).collect();
        assert!(ys[1..SAMPLES].iter().all(|&y| y < 100.0));
        assert!((ys[SAMPLES / 2] - 40.0).abs() < 1e-9);
        // The handle sits where the line passes.
        let curve = a.handles().into_iter().find(|h| h.id == CURVE).unwrap();
        assert!(close(curve.at, Point::new(100.0, 40.0)));
        // Moving an end keeps the bend relative to the ends.
        a.drag_handle(END, Point::new(400.0, 100.0), Modifiers::default());
        assert!(close(a.middle, Point::new(200.0, -20.0)));
    }

    #[test]
    fn shift_constrains_ends_to_45_degrees() {
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        let mut a = Arrow::new(Point::new(0.0, 0.0), Point::new(100.0, 0.0));
        a.drag_handle(END, Point::new(100.0, 7.0), shift);
        assert!(close(a.end, Point::new(100.0, 0.0)));
        a.draw_to(Point::new(90.0, 95.0), shift);
        assert!((a.end.x - a.end.y).abs() < 1e-9);
        assert!(close(a.middle, Point::new(a.end.x / 2.0, a.end.y / 2.0)));
    }

    #[test]
    fn double_ended_arrow_draws_both_heads() {
        // Spec "Double-ended": both ends triangle → both ends show arrowheads.
        let mut a = Arrow::new(Point::new(40.0, 50.0), Point::new(160.0, 50.0));
        a.shadow.enabled = false;
        a.set_property("start_style", &PropertyValue::Choice("triangle".into()))
            .unwrap();
        let doc = Document::new(Bitmap::filled(200, 100, Rgba::WHITE).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: crate::document::annotation::Target::Export,
        };
        let out = Bitmap::draw(200, 100, |cr| a.render(cr, &ctx)).unwrap();
        // 20 px from each tip: off the line's own width (8 px → ±4) but inside the head (±9.6).
        let coral = |x, y| out.pixel(x, y) == Some(palette::CORAL);
        assert!(coral(60, 57) && coral(60, 43), "start head");
        assert!(coral(140, 57) && coral(140, 43), "end head");
        assert!(!coral(100, 57), "no head in the middle");
        // Without the start style, the start is plain.
        a.start_style = EndStyle::None;
        let out = Bitmap::draw(200, 100, |cr| a.render(cr, &ctx)).unwrap();
        assert_ne!(out.pixel(60, 57), Some(palette::CORAL));
    }

    #[test]
    fn hit_test_follows_the_curve_and_heads() {
        let mut a = Arrow::new(Point::new(0.0, 100.0), Point::new(200.0, 100.0));
        a.middle = Point::new(100.0, 40.0);
        assert!(a.hit_test(Point::new(100.0, 42.0), 0.0));
        assert!(
            !a.hit_test(Point::new(100.0, 100.0), 2.0),
            "the chord is empty"
        );
        assert!(a.hit_test(Point::new(198.0, 108.0), 0.0), "head");
    }

    #[test]
    fn arrange_maps_the_three_points() {
        let mut a = Arrow::new(Point::new(0.0, 0.0), Point::new(100.0, 50.0));
        assert_eq!(a.arrange().rect, Rect::new(0.0, 0.0, 100.0, 50.0));
        a.set_arrange(Arrange::new(Rect::new(10.0, 10.0, 200.0, 100.0), 0.0));
        assert!(close(a.start, Point::new(10.0, 10.0)));
        assert!(close(a.end, Point::new(210.0, 110.0)));
        assert!(close(a.middle, Point::new(110.0, 60.0)));
        a.translate(5.0, -5.0);
        assert!(close(a.start, Point::new(15.0, 5.0)));
        // A quarter turn of the document maps the end points exactly.
        let t = Transform {
            quarter_turns: 1,
            dx: 1000.0,
            style_scale: 2.0,
            ..Transform::IDENTITY
        };
        a.transform(&t);
        assert!(close(a.start, Point::new(995.0, 15.0)));
        assert_eq!(a.width, 16.0);
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut a = Arrow::new(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let before = a.clone();
        for p in a.clone().properties() {
            a.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(a, before);
        assert!(a
            .set_property("width", &PropertyValue::Number(41.0))
            .is_err());
        assert!(a
            .set_property("width", &PropertyValue::Number(0.5))
            .is_err());
        assert!(a
            .set_property("end_style", &PropertyValue::Choice("diamond".into()))
            .is_err());
        a.set_property("line_style", &PropertyValue::Choice("dashed".into()))
            .unwrap();
        assert_eq!(a.line_style, LineStyle::Dashed);
        a.set_property("end_size", &PropertyValue::Number(1.5))
            .unwrap();
        assert_eq!(a.end_size, 1.5);
    }
}
