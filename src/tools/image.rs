//! Image objects: a bitmap placed on the canvas as a normal object (spec editor-canvas: Recent
//! captures tray — "Picture in picture"; mockups 11/12). Border, corner radius, shadow,
//! opacity; movable, resizable and rotatable like any object.

use serde::{Deserialize, Serialize};

use crate::document::annotation::{
    box_handles, drag_box_handle, value, Annotation, Handle, HandleId, Modifiers, Property,
    PropertyError, PropertyValue, RenderContext, Target,
};
use crate::document::bitmap::Bitmap;
use crate::document::color::{palette, Rgba};
use crate::document::command::Command;
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::scene::Node;
use crate::document::style::Shadow;
use crate::document::Document;
use crate::render::{filter_for, shadow};

/// Largest border width offered (document pixels).
pub const MAX_BORDER: f64 = 100.0;
/// An inserted image fits within this share of the canvas (mockup 12: 480 × 300 on 1600 × 1000).
pub const INSERT_FRACTION: f64 = 0.3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    /// Resource id of the bitmap (`images/<n>.png`).
    pub source: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees clockwise around the centre.
    pub rotation: f64,
    /// 0.0–1.0.
    pub opacity: f64,
    pub border_color: Rgba,
    /// Border stroke width, centred on the image edge; 0 = no border.
    pub border_width: f64,
    pub corner_radius: f64,
    pub shadow: Shadow,
}

impl Image {
    pub const TYPE: &'static str = "image";

    /// Defaults from mockup 12: Paper border 6 px, no rounding, large soft shadow.
    pub const DEFAULT_SHADOW: Shadow = Shadow {
        enabled: true,
        color: Rgba::opaque(0, 0, 6),
        opacity: 0.45,
        angle: 52.0,
        distance: 11.4,
        blur: 8.0,
    };

    pub fn new(source: impl Into<String>, rect: Rect) -> Self {
        Self {
            source: source.into(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            rotation: 0.0,
            opacity: 1.0,
            border_color: palette::PAPER,
            border_width: 6.0,
            corner_radius: 0.0,
            shadow: Self::DEFAULT_SHADOW,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// The command that inserts `bitmap` as a new top-most image object centred on `centre`,
    /// scaled down to fit [`INSERT_FRACTION`] of the canvas (never enlarged).
    pub fn insert_command(doc: &Document, bitmap: Bitmap, centre: Point) -> Command {
        let canvas = doc.canvas().rect;
        let (bw, bh) = (f64::from(bitmap.width()), f64::from(bitmap.height()));
        let scale = (f64::from(canvas.width) * INSERT_FRACTION / bw)
            .min(f64::from(canvas.height) * INSERT_FRACTION / bh)
            .min(1.0);
        let (w, h) = ((bw * scale).round().max(1.0), (bh * scale).round().max(1.0));
        let rect = Rect::new(
            (centre.x - w / 2.0).round(),
            (centre.y - h / 2.0).round(),
            w,
            h,
        );
        let source = doc.resources().fresh_id();
        let id = doc.scene().fresh_ids(1)[0];
        let node = Node::new(id, Box::new(Image::new(source.clone(), rect)));
        Command::Insert {
            nodes: vec![(doc.scene().len(), node)],
            resources: vec![(source, bitmap)],
        }
    }

    fn outline(&self) -> Rect {
        self.rect().inflate(self.border_width.max(0.0) / 2.0)
    }

    /// Switches the context to the image's own frame: origin at its top-left corner.
    fn enter_frame(&self, cr: &cairo::Context) {
        let c = self.rect().centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        cr.translate(-self.width / 2.0, -self.height / 2.0);
    }

    fn frame_path(&self, cr: &cairo::Context) {
        let b = self.border_width.max(0.0) / 2.0;
        shadow::rounded_rect(
            cr,
            -b,
            -b,
            self.width + 2.0 * b,
            self.height + 2.0 * b,
            self.corner_radius + b,
        );
    }

    fn render_content(
        &self,
        cr: &cairo::Context,
        ctx: &RenderContext<'_>,
    ) -> Result<(), cairo::Error> {
        self.enter_frame(cr);
        cr.save()?;
        shadow::rounded_rect(cr, 0.0, 0.0, self.width, self.height, self.corner_radius);
        cr.clip();
        match ctx.document.resources().get(&self.source) {
            Some(bitmap) if bitmap.width() > 0 && bitmap.height() > 0 => {
                cr.scale(
                    self.width / f64::from(bitmap.width()),
                    self.height / f64::from(bitmap.height()),
                );
                cr.set_source_surface(bitmap.surface(), 0.0, 0.0)?;
                let effective = ctx.zoom * self.width / f64::from(bitmap.width());
                cr.source().set_filter(filter_for(effective));
                cr.paint()?;
            }
            // A missing bitmap cannot come from a valid file; show a placeholder on screen.
            _ if ctx.target == Target::Screen => {
                cr.set_source_rgba(0.5, 0.5, 0.5, 0.5);
                cr.paint()?;
            }
            _ => {}
        }
        cr.restore()?;
        if self.border_width > 0.0 && self.border_color.a > 0 {
            shadow::rounded_rect(cr, 0.0, 0.0, self.width, self.height, self.corner_radius);
            self.border_color.set_source(cr);
            cr.set_line_width(self.border_width);
            cr.stroke()?;
        }
        Ok(())
    }
}

impl Annotation for Image {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        let body = self.outline().rotated_bounds(self.rotation);
        if !self.shadow.is_visible() {
            return body;
        }
        let (dx, dy) = self.shadow.offset();
        body.union(
            &body
                .translate(dx, dy)
                .inflate(3.0 * self.shadow.blur.max(0.0) + 1.0),
        )
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let local = self.arrange().to_local(point);
        self.outline().inflate(tolerance.max(0.0)).contains(local)
    }

    fn render(&self, cr: &cairo::Context, ctx: &RenderContext<'_>) -> Result<(), cairo::Error> {
        let opacity = self.opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return Ok(());
        }
        if opacity < 1.0 {
            cr.push_group();
        }
        if self.shadow.is_visible() {
            cr.save()?;
            let (dx, dy) = self.shadow.offset();
            cr.translate(dx, dy);
            self.enter_frame(cr);
            let painted = shadow::paint(
                cr,
                self.shadow.color,
                self.shadow.opacity,
                self.shadow.blur,
                |cr| self.frame_path(cr),
            );
            cr.restore()?;
            painted?;
        }
        cr.save()?;
        let drawn = self.render_content(cr, ctx);
        cr.restore()?;
        drawn?;
        if opacity < 1.0 {
            cr.pop_group_to_source()?;
            cr.paint_with_alpha(opacity)?;
        }
        Ok(())
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

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("border_color", PropertyValue::Color(self.border_color)),
            Property::new("border_width", PropertyValue::Number(self.border_width)),
            Property::new("corner_radius", PropertyValue::Number(self.corner_radius)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "border_color" => self.border_color = value::color(key, v)?,
            "border_width" => self.border_width = value::number(key, v, 0.0..=MAX_BORDER)?,
            "corner_radius" => self.corner_radius = value::number(key, v, 0.0..=f64::MAX)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    fn handles(&self) -> Vec<Handle> {
        box_handles(&self.arrange())
    }

    /// Corner handles keep the aspect ratio unless Shift is held (mockup 12 sidebar note).
    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        drag_box_handle(self, handle, to, modifiers, true);
    }

    fn resources(&self) -> Vec<&str> {
        vec![self.source.as_str()]
    }

    /// The box follows the image (default); with "Scale annotations" the border, the rounding
    /// and the shadow scale too. Mirrored, the picture itself is not mirrored (it keeps reading
    /// the right way round).
    fn transform(&mut self, t: &Transform) {
        if t.is_translation() {
            self.translate(t.dx, t.dy);
            return;
        }
        self.set_arrange(t.map_arrange(&self.arrange()));
        let s = t.style_scale;
        if s.is_finite() && s > 0.0 && s != 1.0 {
            self.border_width = (self.border_width * s).min(MAX_BORDER);
            self.corner_radius *= s;
            self.shadow.distance *= s;
            self.shadow.blur *= s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::Compass;
    use crate::document::command::History;

    fn image() -> Image {
        Image::new("images/1.png", Rect::new(100.0, 100.0, 200.0, 100.0))
    }

    #[test]
    fn insert_scales_into_the_canvas_and_is_undoable() {
        // Spec "Picture in picture" (model part): the image becomes a movable object.
        let mut doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let mut history = History::new();
        let shot = Bitmap::filled(1600, 1000, palette::AZURE).unwrap();
        let cmd = Image::insert_command(&doc, shot.clone(), Point::new(1180.0, 630.0));
        history.execute(&mut doc, "Insert Image", cmd).unwrap();
        let node = &doc.scene().nodes()[0];
        let img = node.object.as_any().downcast_ref::<Image>().unwrap();
        // Mockup 12: 480 × 300 at (940, 480).
        assert_eq!(img.rect(), Rect::new(940.0, 480.0, 480.0, 300.0));
        assert!(doc.resources().get(&img.source).unwrap().ptr_eq(&shot));
        // Movable like any object.
        let id = node.id;
        let mv = Command::translate(&doc, &[id], 20.0, 0.0);
        history.execute(&mut doc, "Move", mv).unwrap();
        assert_eq!(doc.scene().get(id).unwrap().object.arrange().rect.x, 960.0);
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert!(doc.scene().is_empty());
    }

    #[test]
    fn small_images_are_not_enlarged() {
        let doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let cmd = Image::insert_command(
            &doc,
            Bitmap::filled(40, 20, Rgba::WHITE).unwrap(),
            Point::new(100.0, 100.0),
        );
        let Command::Insert { nodes, .. } = cmd else {
            panic!("insert expected");
        };
        assert_eq!(
            nodes[0].1.object.arrange().rect,
            Rect::new(80.0, 90.0, 40.0, 20.0)
        );
    }

    #[test]
    fn corner_drag_keeps_ratio_unless_shift() {
        let mut free = image();
        free.drag_handle(
            HandleId::Resize(Compass::SE),
            Point::new(500.0, 150.0),
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        );
        assert_eq!((free.width, free.height), (400.0, 50.0));
        let mut kept = image();
        kept.drag_handle(
            HandleId::Resize(Compass::SE),
            Point::new(500.0, 150.0),
            Modifiers::default(),
        );
        assert_eq!((kept.width, kept.height), (400.0, 200.0));
    }

    #[test]
    fn hit_test_follows_rotation() {
        let mut img = image();
        img.border_width = 0.0;
        img.rotation = 90.0;
        // Rotated a quarter turn around (200, 150): now 100 wide, 200 tall.
        assert!(img.hit_test(Point::new(200.0, 60.0), 0.0));
        assert!(!img.hit_test(Point::new(110.0, 150.0), 0.0));
        assert!(img.hit_test(Point::new(148.0, 150.0), 3.0));
    }

    #[test]
    fn bounds_include_border_and_shadow() {
        let mut img = image();
        img.shadow.enabled = false;
        assert_eq!(img.bounds(), Rect::new(97.0, 97.0, 206.0, 106.0));
        img.shadow = Image::DEFAULT_SHADOW;
        let b = img.bounds();
        let (dx, dy) = img.shadow.offset();
        assert!(b.right() >= 303.0 + dx + 24.0 && b.bottom() >= 203.0 + dy + 24.0);
        // The blur reaches back over the top-left edge too.
        assert!(b.x <= 97.0 && b.y <= 97.0);
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut img = image();
        for p in img.clone().properties() {
            img.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(img, image());
        img.set_property("border_color", &PropertyValue::Color(palette::CORAL))
            .unwrap();
        assert_eq!(img.border_color, palette::CORAL);
        assert!(img
            .set_property("opacity", &PropertyValue::Number(1.5))
            .is_err());
        assert!(img
            .set_property("border_width", &PropertyValue::Text("6".into()))
            .is_err());
        assert!(img
            .set_property("nonsense", &PropertyValue::Bool(true))
            .is_err());
    }
}
