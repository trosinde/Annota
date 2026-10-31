//! Callouts (spec annotation-tools: Callout; mockups 09, 10): a text box — rectangle, rounded
//! rectangle, ellipse or cloud — with fill, outline, padding and zero or more tails.
//!
//! The body is a box like a shape's; the text wraps inside it and the body grows downwards when
//! the text needs more room ("The body auto-grows with text"). Each tail is a wedge from the
//! body's edge (where the line from the body's centre to the tip leaves the body) to its tip,
//! which is a point in document space: resizing the body leaves the tips where they point;
//! moving the callout moves them along.
//!
//! Body, tails and cloud bumps are drawn as one shape: the fill of all parts at once (one
//! layer, so overlaps do not add up), and each part's outline only where no other part covers
//! it, so the outline runs around the union.
//!
//! Handles (redline "On-canvas handles"): the body's 8 resize handles, a filled 12 px handle at
//! each tail tip, and the 18 px add-tail handle 23 px below the body. Dragging the add-tail
//! handle makes another tail; dragging a tip onto the body removes that tail, the last one too:
//! a callout without tails is a filled text box (the only way to put text on a background, since
//! text objects have no fill). The add-tail handle stays, so a tail can be added again.

use serde::{Deserialize, Serialize};

use super::common::{
    self, choices, ellipse_path, inside_polygon, map_point, points_bounds, shadow_scaled,
    style_scaled, styled, with_shadow,
};
use super::text::{
    draw_layout, layout, Alignment, EditableText, MeasureCache, TextBlock, TextStyle,
    DEFAULT_FAMILY, MAX_FONT_SIZE, MIN_FONT_SIZE,
};
use crate::document::annotation::{
    box_handles, resize_arrange, value, Annotation, Handle, HandleId, HandleKind, Modifiers,
    Property, PropertyError, PropertyValue, RenderContext, MIN_SIZE,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::Shadow;
use crate::render::shadow::rounded_rect;

pub const MAX_OUTLINE: f64 = 40.0;
pub const MAX_PADDING: f64 = 200.0;
/// The add-tail handle.
pub const ADD_TAIL: HandleId = HandleId::Custom(99);
/// Tail tip `i` is `HandleId::Custom(TAIL_BASE + i)`.
pub const TAIL_BASE: u32 = 100;
/// Distance of the add-tail handle below the body, in screen pixels (redline).
pub const ADD_TAIL_GAP: f64 = 23.0;

choices!(
    /// The body's outline (sidebar: Rectangle / Rounded / Ellipse / Cloud).
    CalloutShape {
        Rectangle => "rectangle",
        Rounded => "rounded_rectangle",
        Ellipse => "ellipse",
        Cloud => "cloud",
    }
);

/// Adds one part of a callout's outline to the path.
type PathPart<'a> = Box<dyn Fn(&cairo::Context) + 'a>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Callout {
    pub text: String,
    /// The body as drawn (before rotation). It is drawn taller when the text needs it.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees clockwise around the body's centre.
    pub rotation: f64,
    pub shape: CalloutShape,
    /// Tail tips in document space.
    pub tails: Vec<Point>,
    pub fill: Rgba,
    /// Transparent or width 0 = no outline.
    pub outline_color: Rgba,
    pub outline_width: f64,
    pub padding: f64,
    /// Rounded rectangle only.
    pub corner_radius: f64,
    pub font_family: String,
    pub font_size: f64,
    pub bold: bool,
    pub text_color: Rgba,
    pub alignment: Alignment,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
    #[serde(skip)]
    cache: MeasureCache,
}

/// Horizontal inset of an ellipse's text area per unit of width: the inscribed rectangle of an
/// ellipse is 1/√2 of its size.
const ELLIPSE_INSET: f64 = (1.0 - std::f64::consts::FRAC_1_SQRT_2) / 2.0;

impl Callout {
    pub const TYPE: &'static str = "callout";

    /// A new callout with body `rect` and one tail to `tip`, with the tool defaults (redline:
    /// rounded, fill Coral, text white 30 Bold, no outline, padding 18 px, 1 tail, shadow on).
    pub fn new(rect: Rect, tip: Point) -> Self {
        Self {
            text: String::new(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            rotation: 0.0,
            shape: CalloutShape::Rounded,
            tails: vec![tip],
            fill: palette::CORAL,
            outline_color: palette::CORAL,
            outline_width: 0.0,
            padding: 18.0,
            corner_radius: 18.0,
            font_family: DEFAULT_FAMILY.to_string(),
            font_size: 30.0,
            bold: true,
            text_color: palette::PAPER,
            alignment: Alignment::Center,
            opacity: 1.0,
            shadow: Shadow::DEFAULT,
            cache: Default::default(),
        }
    }

    /// Where a new callout's tail points by default: below its body's left third.
    pub fn default_tip(body: Rect) -> Point {
        Point::new(
            body.x + body.width * 0.25,
            body.bottom() + (body.height * 0.6).max(60.0),
        )
    }

    fn round(&self) -> bool {
        matches!(self.shape, CalloutShape::Ellipse | CalloutShape::Cloud)
    }

    /// Horizontal inset of the text from the body's sides.
    fn inset_x(&self) -> f64 {
        let p = self.padding.max(0.0);
        if self.round() {
            p + self.width * ELLIPSE_INSET
        } else {
            p
        }
    }

    pub fn style(&self) -> TextStyle {
        TextStyle {
            family: self.font_family.clone(),
            size: self.font_size,
            weight: if self.bold {
                pango::Weight::Bold
            } else {
                pango::Weight::Normal
            },
            italic: false,
            underline: false,
            strikethrough: false,
            alignment: self.alignment,
            wrap: Some((self.width - 2.0 * self.inset_x()).max(1.0)),
        }
    }

    fn text_height(&self) -> f64 {
        self.cache.get(&self.text, &self.style()).height
    }

    /// The body as drawn: as tall as the text needs, at least its own height.
    pub fn body(&self) -> Rect {
        let p = self.padding.max(0.0);
        let needed = if self.round() {
            (self.text_height() + 2.0 * p) * std::f64::consts::SQRT_2
        } else {
            self.text_height() + 2.0 * p
        };
        Rect::new(self.x, self.y, self.width, self.height.max(needed))
    }

    fn frame(&self) -> Arrange {
        Arrange::new(self.body(), self.rotation)
    }

    /// The tail to `tip` as a triangle in the body's unrotated frame, `None` while the tip is
    /// inside the body (an invisible tail).
    fn tail(&self, body: Rect, tip: Point) -> Option<[Point; 3]> {
        let frame = Arrange::new(body, self.rotation);
        let t = frame.to_local(tip);
        if self.inside_body(body, t, 0.0) {
            return None;
        }
        let c = body.centre();
        let (dx, dy) = (t.x - c.x, t.y - c.y);
        let (hw, hh) = (body.width / 2.0, body.height / 2.0);
        let s = if self.round() {
            1.0 / ((dx / hw.max(1e-9)).powi(2) + (dy / hh.max(1e-9)).powi(2)).sqrt()
        } else {
            (hw / dx.abs().max(1e-9)).min(hh / dy.abs().max(1e-9))
        };
        let half = (body.width.min(body.height) * 0.16).clamp(6.0, 24.0);
        let len = dx.hypot(dy).max(1e-9);
        let (ux, uy) = (dx / len, dy / len);
        // The base sits on the edge, pulled in so tail and body overlap.
        let edge = Point::new(c.x + dx * s - ux * half, c.y + dy * s - uy * half);
        let (bx, by) = (t.x - edge.x, t.y - edge.y);
        let blen = bx.hypot(by).max(1e-9);
        let (px, py) = (-by / blen * half, bx / blen * half);
        Some([
            Point::new(edge.x + px, edge.y + py),
            t,
            Point::new(edge.x - px, edge.y - py),
        ])
    }

    fn inside_body(&self, body: Rect, local: Point, tolerance: f64) -> bool {
        if self.round() {
            let (rx, ry) = (body.width / 2.0 + tolerance, body.height / 2.0 + tolerance);
            let c = body.centre();
            rx > 0.0
                && ry > 0.0
                && ((local.x - c.x) / rx).powi(2) + ((local.y - c.y) / ry).powi(2) <= 1.0
        } else {
            body.inflate(tolerance).contains(local)
        }
    }

    /// Circles of the cloud's bumps: (centre, radius), in the unrotated frame.
    fn bumps(&self, body: Rect) -> Vec<(Point, f64)> {
        let (rx, ry) = (body.width / 2.0, body.height / 2.0);
        let rb = (rx.min(ry) * 0.45).max(2.0);
        let (ix, iy) = ((rx - rb).max(0.0), (ry - rb).max(0.0));
        let perimeter =
            std::f64::consts::PI * (3.0 * (ix + iy) - ((3.0 * ix + iy) * (ix + 3.0 * iy)).sqrt());
        let n = ((perimeter / (rb * 1.3)).round() as usize).clamp(8, 64);
        let c = body.centre();
        (0..n)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / n as f64;
                (Point::new(c.x + ix * a.cos(), c.y + iy * a.sin()), rb)
            })
            .collect()
    }

    /// The shape's parts (body, cloud bumps, tails) as path builders, unrotated frame.
    fn parts(&self, body: Rect) -> Vec<PathPart<'_>> {
        let mut parts: Vec<PathPart<'_>> = Vec::new();
        match self.shape {
            CalloutShape::Rectangle => {
                parts.push(Box::new(move |cr| {
                    cr.rectangle(body.x, body.y, body.width, body.height)
                }));
            }
            CalloutShape::Rounded => {
                let r = self.corner_radius;
                parts.push(Box::new(move |cr| {
                    rounded_rect(cr, body.x, body.y, body.width, body.height, r)
                }));
            }
            CalloutShape::Ellipse => parts.push(Box::new(move |cr| ellipse_path(cr, body))),
            CalloutShape::Cloud => {
                let bumps = self.bumps(body);
                let rb = bumps.first().map_or(0.0, |b| b.1);
                let inner = body.inflate(-rb * 0.5);
                parts.push(Box::new(move |cr| ellipse_path(cr, inner)));
                for (c, r) in bumps {
                    parts.push(Box::new(move |cr| {
                        cr.new_sub_path();
                        cr.arc(c.x, c.y, r, 0.0, std::f64::consts::TAU);
                        cr.close_path();
                    }));
                }
            }
        }
        for tip in &self.tails {
            if let Some(tri) = self.tail(body, *tip) {
                parts.push(Box::new(move |cr| {
                    cr.move_to(tri[0].x, tri[0].y);
                    cr.line_to(tri[1].x, tri[1].y);
                    cr.line_to(tri[2].x, tri[2].y);
                    cr.close_path();
                }));
            }
        }
        parts
    }

    /// Everything the shape covers, unrotated frame (bumps reach a little beyond the body).
    fn local_extent(&self, body: Rect) -> Rect {
        let frame = Arrange::new(body, self.rotation);
        let tips = self.tails.iter().map(|t| frame.to_local(*t));
        points_bounds(tips.chain(body.corners()))
    }

    fn body_bounds(&self) -> Rect {
        let body = self.body();
        let frame = Arrange::new(body, self.rotation);
        let r = self
            .local_extent(body)
            .inflate(self.outline_width.max(0.0) / 2.0 + 2.0);
        points_bounds(r.corners().map(|p| frame.to_document(p)))
    }

    fn text_origin(&self, body: Rect) -> Point {
        let h = self.text_height();
        Point::new(body.x + self.inset_x(), body.y + (body.height - h) / 2.0)
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        let body = self.body();
        let c = body.centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        cr.translate(-c.x, -c.y);
        let parts = self.parts(body);
        // Fill: all parts as one layer.
        if self.fill.a > 0 {
            let (r, g, b, a) = self.fill.to_f64();
            cr.push_group();
            cr.set_source_rgb(r, g, b);
            for part in &parts {
                part(cr);
                cr.fill()?;
            }
            cr.pop_group_to_source()?;
            cr.paint_with_alpha(a)?;
        }
        // Outline around the union: each part's outline where no other part covers it.
        let w = self.outline_width.max(0.0);
        if w > 0.0 && self.outline_color.a > 0 {
            let room = self.local_extent(body).inflate(w + 4.0);
            for (i, part) in parts.iter().enumerate() {
                cr.save()?;
                for (j, other) in parts.iter().enumerate() {
                    if i == j {
                        continue;
                    }
                    cr.new_path();
                    cr.rectangle(room.x, room.y, room.width, room.height);
                    other(cr);
                    cr.set_fill_rule(cairo::FillRule::EvenOdd);
                    cr.clip();
                }
                cr.set_fill_rule(cairo::FillRule::Winding);
                cr.new_path();
                part(cr);
                self.outline_color.set_source(cr);
                cr.set_line_width(w);
                cr.set_line_join(cairo::LineJoin::Round);
                cr.stroke()?;
                cr.restore()?;
            }
        }
        if let Some(layout) = layout(&self.text, &self.style()) {
            let origin = self.text_origin(body);
            cr.translate(origin.x, origin.y);
            draw_layout(cr, &layout, self.text_color, None)?;
        }
        Ok(())
    }

    fn tail_index(handle: HandleId) -> Option<usize> {
        match handle {
            HandleId::Custom(n) if n >= TAIL_BASE => Some((n - TAIL_BASE) as usize),
            _ => None,
        }
    }
}

impl EditableText for Callout {
    fn text(&self) -> &str {
        &self.text
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
    }

    fn text_block(&self) -> TextBlock {
        let body = self.body();
        TextBlock {
            text: self.text.clone(),
            style: self.style(),
            origin: self.text_origin(body),
            frame: Arrange::new(body, self.rotation),
        }
    }
}

impl Annotation for Callout {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let tolerance = tolerance.max(0.0) + self.outline_width.max(0.0) / 2.0;
        let body = self.body();
        let local = Arrange::new(body, self.rotation).to_local(point);
        if self.inside_body(body, local, tolerance) {
            return true;
        }
        if self.shape == CalloutShape::Cloud
            && self
                .bumps(body)
                .iter()
                .any(|(c, r)| local.distance(*c) <= r + tolerance)
        {
            return true;
        }
        self.tails.iter().any(|tip| {
            self.tail(body, *tip).is_some_and(|tri| {
                inside_polygon(local, &tri)
                    || common::distance_to_polyline(local, &[tri[0], tri[1], tri[2], tri[0]])
                        <= tolerance
            })
        })
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        self.frame()
    }

    /// Fits the body into `arrange` and maps the tail tips along (group scaling, Arrange
    /// entries). Dragging the body's own handles leaves the tips where they point
    /// ([`Annotation::drag_handle`]).
    fn set_arrange(&mut self, arrange: Arrange) {
        let old = self.frame();
        for tip in &mut self.tails {
            *tip = map_point(old.to_local(*tip), old.rect, &arrange);
        }
        self.x = arrange.rect.x;
        self.y = arrange.rect.y;
        self.width = arrange.rect.width.max(MIN_SIZE);
        self.height = arrange.rect.height.max(MIN_SIZE);
        self.rotation = arrange.rotation;
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
        for tip in &mut self.tails {
            tip.x += dx;
            tip.y += dy;
        }
    }

    /// The body maps like a box and its text stays readable; the tips map exactly.
    fn transform(&mut self, t: &Transform) {
        if t.is_translation() {
            self.translate(t.dx, t.dy);
            return;
        }
        let a = t.map_arrange(&self.frame());
        self.x = a.rect.x;
        self.y = a.rect.y;
        self.width = a.rect.width;
        self.height = a.rect.height;
        self.rotation = a.rotation;
        for tip in &mut self.tails {
            *tip = t.apply(*tip);
        }
        self.font_size = style_scaled(self.font_size, t).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.padding = style_scaled(self.padding, t).min(MAX_PADDING);
        self.outline_width = style_scaled(self.outline_width, t).min(MAX_OUTLINE);
        self.corner_radius = style_scaled(self.corner_radius, t);
        shadow_scaled(&mut self.shadow, t);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("text", PropertyValue::Text(self.text.clone())),
            Property::new("shape", PropertyValue::Choice(self.shape.key().into())),
            Property::new("fill", PropertyValue::Color(self.fill)),
            Property::new("outline_color", PropertyValue::Color(self.outline_color)),
            Property::new("outline_width", PropertyValue::Number(self.outline_width)),
            Property::new("padding", PropertyValue::Number(self.padding)),
            Property::new("corner_radius", PropertyValue::Number(self.corner_radius)),
            Property::new("font_family", PropertyValue::Text(self.font_family.clone())),
            Property::new("font_size", PropertyValue::Number(self.font_size)),
            Property::new("bold", PropertyValue::Bool(self.bold)),
            Property::new("color", PropertyValue::Color(self.text_color)),
            Property::new(
                "alignment",
                PropertyValue::Choice(self.alignment.key().into()),
            ),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "text" => self.text = common::text(key, v)?,
            "shape" => self.shape = common::choice(key, v, CalloutShape::from_key)?,
            "fill" => self.fill = value::color(key, v)?,
            "outline_color" => self.outline_color = value::color(key, v)?,
            "outline_width" => self.outline_width = value::number(key, v, 0.0..=MAX_OUTLINE)?,
            "padding" => self.padding = value::number(key, v, 0.0..=MAX_PADDING)?,
            "corner_radius" => self.corner_radius = value::number(key, v, 0.0..=f64::MAX)?,
            "font_family" => {
                let family = common::text(key, v)?;
                if family.trim().is_empty() {
                    return Err(PropertyError::Range(key.to_string()));
                }
                self.font_family = family;
            }
            "font_size" => self.font_size = value::number(key, v, MIN_FONT_SIZE..=MAX_FONT_SIZE)?,
            "bold" => self.bold = value::boolean(key, v)?,
            "color" => self.text_color = value::color(key, v)?,
            "alignment" => self.alignment = common::choice(key, v, Alignment::from_key)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    fn handles(&self) -> Vec<Handle> {
        let frame = self.frame();
        let mut handles: Vec<Handle> = box_handles(&frame)
            .into_iter()
            .filter(|h| h.id != HandleId::Rotate)
            .collect();
        for (i, tip) in self.tails.iter().enumerate() {
            handles.push(Handle::new(
                HandleId::Custom(TAIL_BASE + i as u32),
                HandleKind::Tip,
                *tip,
            ));
        }
        let r = frame.rect;
        let bottom = frame.to_document(Point::new(r.centre().x, r.bottom()));
        let mut add = Handle::new(ADD_TAIL, HandleKind::Add, bottom);
        add.screen_offset =
            Point::new(0.0, ADD_TAIL_GAP).rotated_around(Point::default(), self.rotation);
        handles.push(add);
        handles
    }

    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        let body = self.body();
        let on_body = |c: &Callout| {
            let local = Arrange::new(body, c.rotation).to_local(to);
            c.inside_body(body, local, 0.0)
        };
        match handle {
            HandleId::Resize(compass) => {
                // The body only: the tips keep pointing where they point.
                let next = resize_arrange(&self.frame(), compass, to, modifiers.shift);
                self.x = next.rect.x;
                self.y = next.rect.y;
                self.width = next.rect.width;
                self.height = next.rect.height;
            }
            ADD_TAIL => {
                if !on_body(self) {
                    self.tails.push(to);
                }
            }
            _ => {
                let Some(i) = Self::tail_index(handle).filter(|&i| i < self.tails.len()) else {
                    return;
                };
                if on_body(self) {
                    self.tails.remove(i);
                } else {
                    self.tails[i] = to;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::{Compass, Target};
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    fn sample() -> Callout {
        let body = Rect::new(100.0, 100.0, 300.0, 80.0);
        let mut c = Callout::new(body, Point::new(150.0, 300.0));
        c.text = "Both paths".into();
        c
    }

    fn render(c: &Callout) -> Bitmap {
        let doc = Document::new(Bitmap::filled(600, 400, Rgba::WHITE).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        Bitmap::draw(600, 400, |cr| {
            Rgba::WHITE.set_source(cr);
            cr.paint()?;
            c.render(cr, &ctx)
        })
        .unwrap()
    }

    #[test]
    fn defaults_follow_the_redline() {
        let c = sample();
        assert_eq!(c.shape, CalloutShape::Rounded);
        assert_eq!((c.fill, c.text_color), (palette::CORAL, palette::PAPER));
        assert_eq!((c.font_size, c.bold, c.padding), (30.0, true, 18.0));
        assert_eq!((c.outline_width, c.tails.len()), (0.0, 1));
        assert!(c.shadow.enabled);
    }

    #[test]
    fn dragging_the_add_tail_handle_makes_a_second_tail() {
        // Spec "Two tails": the callout points to two locations.
        let mut c = sample();
        c.shadow.enabled = false;
        let add = c.handles().into_iter().find(|h| h.id == ADD_TAIL).unwrap();
        assert_eq!(add.kind, HandleKind::Add);
        assert_eq!(add.screen_offset, Point::new(0.0, ADD_TAIL_GAP));
        c.drag_handle(ADD_TAIL, Point::new(500.0, 350.0), Modifiers::default());
        assert_eq!(
            c.tails,
            vec![Point::new(150.0, 300.0), Point::new(500.0, 350.0)]
        );
        let tips: Vec<_> = c
            .handles()
            .into_iter()
            .filter(|h| h.kind == HandleKind::Tip)
            .map(|h| h.at)
            .collect();
        assert_eq!(tips, c.tails);
        // Both tails are drawn: on each tail's axis, a third of the way from its tip, there is
        // fill.
        let out = render(&c);
        let body = c.body();
        for tip in &c.tails {
            let tri = c.tail(body, *tip).unwrap();
            let p = Point::new(
                tip.x + ((tri[0].x + tri[2].x) / 2.0 - tip.x) / 3.0,
                tip.y + ((tri[0].y + tri[2].y) / 2.0 - tip.y) / 3.0,
            );
            assert_eq!(
                out.pixel(p.x as i32, p.y as i32),
                Some(palette::CORAL),
                "{p:?}"
            );
            assert!(c.hit_test(p, 0.0));
        }
        assert!(!c.hit_test(Point::new(300.0, 350.0), 0.0));
    }

    #[test]
    fn dropping_a_tip_on_the_body_removes_the_tail_even_the_last() {
        let mut c = sample();
        c.tails.push(Point::new(500.0, 350.0));
        let mut removed = c.clone();
        removed.drag_handle(
            HandleId::Custom(TAIL_BASE),
            Point::new(250.0, 140.0),
            Modifiers::default(),
        );
        assert_eq!(removed.tails, vec![Point::new(500.0, 350.0)]);
        // Adding a tail onto the body adds nothing.
        let mut none = removed.clone();
        none.drag_handle(ADD_TAIL, Point::new(250.0, 140.0), Modifiers::default());
        assert_eq!(none.tails.len(), 1);
        // Maintainer decision (phase B): the last tail goes too — a filled text box remains,
        // with its add-tail handle, and it saves, reads back and renders without a tail.
        let mut last = removed.clone();
        last.drag_handle(
            HandleId::Custom(TAIL_BASE),
            Point::new(250.0, 140.0),
            Modifiers::default(),
        );
        assert!(last.tails.is_empty());
        assert!(last.handles().iter().any(|h| h.id == ADD_TAIL));
        assert!(!last.handles().iter().any(|h| h.kind == HandleKind::Tip));
        crate::document::registry::check_reads_back(&last).unwrap();
        let out = render(&last);
        let centre = last.body().centre();
        assert_eq!(
            out.pixel(centre.x as i32 - 100, centre.y as i32),
            Some(palette::CORAL),
            "the body is still filled"
        );
        // And a tail can be added back.
        last.drag_handle(ADD_TAIL, Point::new(500.0, 350.0), Modifiers::default());
        assert_eq!(last.tails, vec![Point::new(500.0, 350.0)]);
    }

    #[test]
    fn the_body_grows_with_the_text() {
        let mut c = sample();
        let one = c.body().height;
        assert_eq!(one, 80.0, "one line fits");
        c.text = "Both paths must be on the same backup disk, always and everywhere".into();
        let more = c.body().height;
        assert!(more > one, "{more}");
        assert_eq!(c.height, 80.0, "the drawn height is kept");
        // Resizing the body keeps the tips.
        c.drag_handle(
            HandleId::Resize(Compass::E),
            Point::new(600.0, 140.0),
            Modifiers::default(),
        );
        assert_eq!(c.width, 500.0);
        assert_eq!(c.tails, vec![Point::new(150.0, 300.0)]);
        // Moving moves the tips.
        c.translate(10.0, 0.0);
        assert_eq!(c.tails, vec![Point::new(160.0, 300.0)]);
    }

    #[test]
    fn shapes_render_and_hit() {
        for shape in CalloutShape::ALL {
            let mut c = sample();
            c.shape = *shape;
            c.shadow.enabled = false;
            c.outline_width = 4.0;
            c.outline_color = palette::INK;
            let out = render(&c);
            // Inside the body, left of the text.
            assert_eq!(out.pixel(160, 140), Some(palette::CORAL), "{shape:?}");
            assert!(c.hit_test(Point::new(250.0, 140.0), 0.0));
            // The outline runs around the union: where the tail leaves the body there is fill,
            // not the body's outline.
            let body = c.body();
            let tri = c.tail(body, c.tails[0]).unwrap();
            let mid = Point::new((tri[0].x + tri[2].x) / 2.0, (tri[0].y + tri[2].y) / 2.0);
            let probe = (0..400)
                .map(|k| {
                    let t = f64::from(k) / 400.0;
                    Point::new(
                        mid.x + (tri[1].x - mid.x) * t,
                        mid.y + (tri[1].y - mid.y) * t,
                    )
                })
                .find(|p| !c.inside_body(body, *p, 0.0))
                .unwrap();
            assert_eq!(
                out.pixel(probe.x.round() as i32, probe.y.round() as i32),
                Some(palette::CORAL),
                "{shape:?} at {probe:?}"
            );
            // Elsewhere the body has its outline (a cloud's top edge is scalloped).
            if *shape != CalloutShape::Cloud {
                assert_eq!(out.pixel(250, 100), Some(palette::INK), "{shape:?}");
            }
        }
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut c = sample();
        let before = c.clone();
        for p in c.clone().properties() {
            c.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(c, before);
        c.set_property("shape", &PropertyValue::Choice("cloud".into()))
            .unwrap();
        assert_eq!(c.shape, CalloutShape::Cloud);
        assert!(c
            .set_property("padding", &PropertyValue::Number(-1.0))
            .is_err());
    }
}
