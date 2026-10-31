//! Shapes (spec annotation-tools: Shape; mockups 06, 07): rectangle, rounded rectangle,
//! ellipse and polygon, with independent outline and fill (a fill may be None — transparent —
//! for a frame).
//!
//! Every kind is a box (`x`, `y`, `width`, `height`, `rotation`) like an image object, so it
//! moves, resizes and rotates with the generic box handles. A polygon keeps its vertices as
//! fractions of that box (0–1), so resizing and rotating it is exactly the box's; the polygon is
//! drawn with round joins. A rounded rectangle also offers its corner-radius handle.

use serde::{Deserialize, Serialize};

use super::common::{
    self, choices, distance_to_polyline, ellipse_path, inside_polygon, shadow_scaled, style_scaled,
    styled, with_shadow,
};
use crate::document::annotation::{
    box_handles, drag_box_handle, value, Annotation, Handle, HandleId, HandleKind, Modifiers,
    Property, PropertyError, PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Mirror, Point, Rect, Transform};
use crate::document::style::{LineStyle, Shadow};
use crate::render::shadow::rounded_rect;

pub const MAX_OUTLINE: f64 = 40.0;
/// The rounded rectangle's corner-radius handle.
pub const RADIUS: HandleId = HandleId::Custom(10);

choices!(
    /// What the shape is (sidebar: Shape type toggle).
    ShapeKind {
        Rectangle => "rectangle",
        RoundedRectangle => "rounded_rectangle",
        Ellipse => "ellipse",
        Polygon => "polygon",
    }
);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub kind: ShapeKind,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees clockwise around the centre.
    pub rotation: f64,
    /// Polygon vertices as fractions of the box (0–1), in drawing order; empty for the other
    /// kinds.
    pub points: Vec<Point>,
    /// Rounded rectangle only.
    pub corner_radius: f64,
    pub outline_color: Rgba,
    /// 0 = no outline.
    pub outline_width: f64,
    /// Transparent (alpha 0) = no fill.
    pub fill: Rgba,
    /// 0.0–1.0, applied on top of the fill colour's own alpha (mockup 07: "Fill opacity").
    pub fill_opacity: f64,
    pub line_style: LineStyle,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
}

impl Shape {
    pub const TYPE: &'static str = "shape";

    /// A new shape with the tool defaults (redline: rounded rectangle, outline Coral 6 px, fill
    /// None, radius 16 px, shadow on).
    pub fn new(rect: Rect) -> Self {
        Self {
            kind: ShapeKind::RoundedRectangle,
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            rotation: 0.0,
            points: Vec::new(),
            corner_radius: 16.0,
            outline_color: palette::CORAL,
            outline_width: 6.0,
            fill: Rgba::TRANSPARENT,
            fill_opacity: 1.0,
            line_style: LineStyle::Solid,
            opacity: 1.0,
            shadow: Shadow::DEFAULT,
        }
    }

    /// A polygon through `vertices` (document space); `None` for fewer than three or vertices
    /// that span no area.
    pub fn polygon(vertices: &[Point], style: &Shape) -> Option<Self> {
        if vertices.len() < 3 {
            return None;
        }
        let b = common::points_bounds(vertices.iter().copied());
        if b.width <= 0.0 || b.height <= 0.0 {
            return None;
        }
        let points = vertices
            .iter()
            .map(|p| Point::new((p.x - b.x) / b.width, (p.y - b.y) / b.height))
            .collect();
        Some(Self {
            kind: ShapeKind::Polygon,
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            rotation: 0.0,
            points,
            ..style.clone()
        })
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// The radius the rounded rectangle is drawn with.
    fn radius(&self) -> f64 {
        match self.kind {
            ShapeKind::RoundedRectangle => self
                .corner_radius
                .max(0.0)
                .min(self.width.abs() / 2.0)
                .min(self.height.abs() / 2.0),
            _ => 0.0,
        }
    }

    /// Polygon vertices in the shape's own frame (origin at the box's top-left).
    fn local_points(&self) -> Vec<Point> {
        let unit: &[Point] = if self.points.len() >= 3 {
            &self.points
        } else {
            &[
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(0.0, 1.0),
            ]
        };
        unit.iter()
            .map(|p| Point::new(p.x * self.width, p.y * self.height))
            .collect()
    }

    fn enter_frame(&self, cr: &cairo::Context) {
        let c = self.rect().centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        cr.translate(-self.width / 2.0, -self.height / 2.0);
    }

    /// The outline path in the shape's own frame.
    fn path(&self, cr: &cairo::Context) {
        let (w, h) = (self.width, self.height);
        match self.kind {
            ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
                rounded_rect(cr, 0.0, 0.0, w, h, self.radius());
            }
            ShapeKind::Ellipse => ellipse_path(cr, Rect::new(0.0, 0.0, w, h)),
            ShapeKind::Polygon => {
                let pts = self.local_points();
                if let Some(first) = pts.first() {
                    cr.move_to(first.x, first.y);
                    for p in &pts[1..] {
                        cr.line_to(p.x, p.y);
                    }
                    cr.close_path();
                }
            }
        }
    }

    fn fill_alpha(&self) -> f64 {
        f64::from(self.fill.a) / 255.0 * self.fill_opacity.clamp(0.0, 1.0)
    }

    fn body_bounds(&self) -> Rect {
        self.rect()
            .inflate(self.outline_width.max(0.0) / 2.0 + 1.0)
            .rotated_bounds(self.rotation)
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        self.enter_frame(cr);
        let alpha = self.fill_alpha();
        if alpha > 0.0 {
            self.path(cr);
            let (r, g, b, _) = self.fill.to_f64();
            cr.set_source_rgba(r, g, b, alpha);
            cr.fill()?;
        }
        if self.outline_width > 0.0 && self.outline_color.a > 0 {
            self.path(cr);
            self.line_style.apply(cr, self.outline_width);
            if self.kind == ShapeKind::Rectangle {
                cr.set_line_join(cairo::LineJoin::Miter);
            }
            self.outline_color.set_source(cr);
            cr.stroke()?;
        }
        Ok(())
    }

    /// Whether `local` (the shape's own frame, origin at the box's centre-less top-left) is on
    /// the outline, within `reach`.
    fn on_outline(&self, local: Point, reach: f64) -> bool {
        let (w, h) = (self.width, self.height);
        match self.kind {
            ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
                let outer = Rect::new(0.0, 0.0, w, h).inflate(reach);
                let inner = Rect::new(reach, reach, w - 2.0 * reach, h - 2.0 * reach);
                outer.contains(local)
                    && !(inner.width > 0.0 && inner.height > 0.0 && inner.contains(local))
            }
            ShapeKind::Ellipse => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let norm = |rx: f64, ry: f64| {
                    if rx <= 0.0 || ry <= 0.0 {
                        return f64::INFINITY;
                    }
                    ((local.x - cx) / rx).powi(2) + ((local.y - cy) / ry).powi(2)
                };
                norm(cx + reach, cy + reach) <= 1.0 && norm(cx - reach, cy - reach) >= 1.0
            }
            ShapeKind::Polygon => {
                let mut pts = self.local_points();
                if let Some(first) = pts.first().copied() {
                    pts.push(first);
                }
                distance_to_polyline(local, &pts) <= reach
            }
        }
    }

    fn inside(&self, local: Point, tolerance: f64) -> bool {
        let (w, h) = (self.width, self.height);
        match self.kind {
            ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
                Rect::new(0.0, 0.0, w, h).inflate(tolerance).contains(local)
            }
            ShapeKind::Ellipse => {
                let (rx, ry) = (w / 2.0 + tolerance, h / 2.0 + tolerance);
                rx > 0.0
                    && ry > 0.0
                    && ((local.x - w / 2.0) / rx).powi(2) + ((local.y - h / 2.0) / ry).powi(2)
                        <= 1.0
            }
            ShapeKind::Polygon => {
                inside_polygon(local, &self.local_points()) || self.on_outline(local, tolerance)
            }
        }
    }
}

impl Annotation for Shape {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    /// A filled shape is hit anywhere inside; a frame (no fill) only on its outline, so what
    /// it frames stays clickable.
    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let tolerance = tolerance.max(0.0);
        let local = self.arrange().to_local(point);
        let local = Point::new(local.x - self.x, local.y - self.y);
        if self.fill_alpha() > 0.0 && self.inside(local, tolerance) {
            return true;
        }
        self.on_outline(local, self.outline_width.max(0.0) / 2.0 + tolerance)
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        Arrange::new(self.rect(), self.rotation)
    }

    fn set_arrange(&mut self, arrange: Arrange) {
        self.x = arrange.rect.x;
        self.y = arrange.rect.y;
        self.width = arrange.rect.width;
        self.height = arrange.rect.height;
        self.rotation = arrange.rotation;
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    /// The box follows the document; a mirror also mirrors a polygon's vertices in its own
    /// frame (`M·R(r) = R(−r)·M`, as for image objects), so the polygon keeps its place on the
    /// image. "Scale annotations" scales the outline, the corner radius and the shadow.
    fn transform(&mut self, t: &Transform) {
        if t.is_translation() {
            self.translate(t.dx, t.dy);
            return;
        }
        self.set_arrange(t.map_arrange(&self.arrange()));
        match t.mirror {
            Some(Mirror::Horizontal) => self.points.iter_mut().for_each(|p| p.x = 1.0 - p.x),
            Some(Mirror::Vertical) => self.points.iter_mut().for_each(|p| p.y = 1.0 - p.y),
            None => {}
        }
        self.outline_width = style_scaled(self.outline_width, t).min(MAX_OUTLINE);
        self.corner_radius = style_scaled(self.corner_radius, t);
        shadow_scaled(&mut self.shadow, t);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("shape", PropertyValue::Choice(self.kind.key().into())),
            Property::new("outline_color", PropertyValue::Color(self.outline_color)),
            Property::new("outline_width", PropertyValue::Number(self.outline_width)),
            Property::new("fill", PropertyValue::Color(self.fill)),
            Property::new("fill_opacity", PropertyValue::Number(self.fill_opacity)),
            Property::new("corner_radius", PropertyValue::Number(self.corner_radius)),
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
            "shape" => self.kind = common::choice(key, v, ShapeKind::from_key)?,
            "outline_color" => self.outline_color = value::color(key, v)?,
            "outline_width" => self.outline_width = value::number(key, v, 0.0..=MAX_OUTLINE)?,
            "fill" => self.fill = value::color(key, v)?,
            "fill_opacity" => self.fill_opacity = value::number(key, v, 0.0..=1.0)?,
            "corner_radius" => self.corner_radius = value::number(key, v, 0.0..=f64::MAX)?,
            "line_style" => self.line_style = common::choice(key, v, LineStyle::from_key)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    /// The box handles; a rounded rectangle adds its corner-radius handle at (r, r) from the
    /// top-left corner (mockup 06).
    fn handles(&self) -> Vec<Handle> {
        let arrange = self.arrange();
        let mut handles = box_handles(&arrange);
        if self.kind == ShapeKind::RoundedRectangle {
            let r = self.radius();
            let at = arrange.to_document(Point::new(self.x + r, self.y + r));
            handles.push(Handle::new(RADIUS, HandleKind::Radius, at));
        }
        handles
    }

    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        if handle == RADIUS {
            let local = self.arrange().to_local(to);
            let r = ((local.x - self.x) + (local.y - self.y)) / 2.0;
            let max = self.width.abs().min(self.height.abs()) / 2.0;
            self.corner_radius = r.clamp(0.0, max).round();
            return;
        }
        drag_box_handle(self, handle, to, modifiers, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::Target;
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    fn render(shape: &Shape, w: i32, h: i32) -> Bitmap {
        let doc = Document::new(Bitmap::filled(w, h, Rgba::WHITE).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        Bitmap::draw(w, h, |cr| {
            Rgba::WHITE.set_source(cr);
            cr.paint()?;
            shape.render(cr, &ctx)
        })
        .unwrap()
    }

    #[test]
    fn defaults_follow_the_redline() {
        let s = Shape::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(s.kind, ShapeKind::RoundedRectangle);
        assert_eq!((s.outline_color, s.outline_width), (palette::CORAL, 6.0));
        assert_eq!((s.fill, s.corner_radius), (Rgba::TRANSPARENT, 16.0));
        assert!(s.shadow.enabled);
    }

    #[test]
    fn a_frame_shows_only_its_outline() {
        // Spec "Frame": a rectangle with transparent fill — only the outline is visible.
        let mut s = Shape::new(Rect::new(20.0, 20.0, 100.0, 60.0));
        s.kind = ShapeKind::Rectangle;
        s.shadow.enabled = false;
        let out = render(&s, 140, 100);
        assert_eq!(out.pixel(20, 50), Some(palette::CORAL), "outline");
        assert_eq!(
            out.pixel(70, 50),
            Some(Rgba::WHITE),
            "inside: the image shows"
        );
        // Clicks go through the empty middle to what is underneath.
        assert!(!s.hit_test(Point::new(70.0, 50.0), 2.0));
        assert!(s.hit_test(Point::new(21.0, 50.0), 2.0));
        // Filled, the inside is painted and hit.
        s.set_property("fill", &PropertyValue::Color(palette::AZURE))
            .unwrap();
        let out = render(&s, 140, 100);
        assert_eq!(out.pixel(70, 50), Some(palette::AZURE));
        assert!(s.hit_test(Point::new(70.0, 50.0), 0.0));
    }

    #[test]
    fn ellipse_and_polygon_hit_tests() {
        let mut e = Shape::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        e.kind = ShapeKind::Ellipse;
        assert!(e.hit_test(Point::new(100.0, 1.0), 0.0));
        assert!(!e.hit_test(Point::new(100.0, 50.0), 2.0));
        assert!(
            !e.hit_test(Point::new(5.0, 5.0), 2.0),
            "outside the ellipse"
        );
        let style = Shape::new(Rect::default());
        let p = Shape::polygon(
            &[
                Point::new(600.0, 520.0),
                Point::new(960.0, 470.0),
                Point::new(1130.0, 600.0),
                Point::new(1060.0, 860.0),
            ],
            &style,
        )
        .unwrap();
        assert_eq!(p.rect(), Rect::new(600.0, 470.0, 530.0, 390.0));
        assert!(p.hit_test(Point::new(1130.0, 600.0), 1.0));
        assert!(!p.hit_test(Point::new(900.0, 650.0), 1.0), "no fill");
        assert!(Shape::polygon(&[Point::new(0.0, 0.0), Point::new(1.0, 1.0)], &style).is_none());
        assert!(Shape::polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(5.0, 0.0),
                Point::new(9.0, 0.0)
            ],
            &style
        )
        .is_none());
    }

    #[test]
    fn mirroring_mirrors_polygon_vertices() {
        let style = Shape::new(Rect::default());
        let tri = [
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(0.0, 50.0),
        ];
        let mut p = Shape::polygon(&tri, &style).unwrap();
        // Flip Horizontally of a 200 px wide image: x → 200 − x.
        p.transform(&Transform {
            mirror: Some(Mirror::Horizontal),
            dx: 200.0,
            ..Transform::IDENTITY
        });
        assert_eq!(p.rect(), Rect::new(100.0, 0.0, 100.0, 50.0));
        // The right angle was at the left; it is at the right now.
        let local = p.local_points();
        assert_eq!(local[0], Point::new(100.0, 0.0));
        assert_eq!(local[2], Point::new(100.0, 50.0));
    }

    #[test]
    fn corner_radius_handle() {
        let mut s = Shape::new(Rect::new(0.0, 0.0, 200.0, 100.0));
        let h = s.handles().into_iter().find(|h| h.id == RADIUS).unwrap();
        assert_eq!(h.at, Point::new(16.0, 16.0));
        s.drag_handle(RADIUS, Point::new(30.0, 30.0), Modifiers::default());
        assert_eq!(s.corner_radius, 30.0);
        s.drag_handle(RADIUS, Point::new(300.0, 300.0), Modifiers::default());
        assert_eq!(s.corner_radius, 50.0, "at most half the shorter side");
        s.kind = ShapeKind::Ellipse;
        assert!(s.handles().iter().all(|h| h.id != RADIUS));
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut s = Shape::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        let before = s.clone();
        for p in s.clone().properties() {
            s.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(s, before);
        s.set_property("shape", &PropertyValue::Choice("ellipse".into()))
            .unwrap();
        assert_eq!(s.kind, ShapeKind::Ellipse);
        assert!(s
            .set_property("shape", &PropertyValue::Choice("star".into()))
            .is_err());
        assert!(s
            .set_property("outline_width", &PropertyValue::Number(-1.0))
            .is_err());
    }
}
