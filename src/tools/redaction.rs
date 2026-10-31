//! Redactions (spec annotation-tools: Redaction; mockup 15): blur, pixelate or a solid box over
//! part of the capture, as a rectangle or an ellipse.
//!
//! Blur and pixelate process the original image's own pixels under the redaction
//! (`crate::render::redact`, read through `RenderContext::document`), in document pixels, so
//! canvas and export match. They hide what the capture shows; annotation objects underneath a
//! redaction are drawn as they are (they are the user's own). Where a redaction reaches beyond
//! the image (an extended canvas) there is nothing to hide and it draws nothing for blur and
//! pixelate. Solid boxes are filled with their colour everywhere.
//!
//! The warning that blur and pixelate may be reversible (D7, mockup 19) belongs to every way
//! out of Annota — export, copy, drag, share — and is built with add-export-share.

use serde::{Deserialize, Serialize};

use super::common::{self, choices, ellipse_path};
use crate::document::annotation::{
    value, Annotation, Property, PropertyError, PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, IRect, Point, Rect};
use crate::render::redact::{self, Effect};

choices!(
    /// What the redaction does (sidebar: Blur / Pixelate / Solid).
    RedactionKind {
        Blur => "blur",
        Pixelate => "pixelate",
        Solid => "solid",
    }
);

choices!(
    /// Its outline (sidebar: Rectangle / Ellipse).
    RedactionShape {
        Rectangle => "rectangle",
        Ellipse => "ellipse",
    }
);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Redaction {
    pub kind: RedactionKind,
    /// 0–100: blur radius or block size (not used by solid boxes).
    pub intensity: f64,
    pub shape: RedactionShape,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees clockwise around the centre.
    pub rotation: f64,
    /// Colour of a solid box.
    pub color: Rgba,
}

impl Redaction {
    pub const TYPE: &'static str = "redaction";

    /// A new redaction with the tool defaults (redline: Pixelate, intensity 70, rectangle;
    /// solid colour Ink).
    pub fn new(rect: Rect) -> Self {
        Self {
            kind: RedactionKind::Pixelate,
            intensity: 70.0,
            shape: RedactionShape::Rectangle,
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            rotation: 0.0,
            color: palette::INK,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }

    /// The effect blur and pixelate apply: intensity 100 is a 20 px blur (standard deviation)
    /// or 32 px blocks.
    pub fn effect(&self) -> Option<Effect> {
        let i = self.intensity.clamp(0.0, 100.0) / 100.0;
        match self.kind {
            RedactionKind::Blur => Some(Effect::Blur {
                sigma: (20.0 * i).max(1.0),
            }),
            RedactionKind::Pixelate => Some(Effect::Pixelate {
                block: (32.0 * i).round().max(2.0) as i32,
            }),
            RedactionKind::Solid => None,
        }
    }

    /// The outline path in document space.
    fn path(&self, cr: &cairo::Context) {
        let m = cr.matrix();
        let c = self.rect().centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        let local = Rect::new(
            -self.width / 2.0,
            -self.height / 2.0,
            self.width,
            self.height,
        );
        match self.shape {
            RedactionShape::Rectangle => cr.rectangle(local.x, local.y, local.width, local.height),
            RedactionShape::Ellipse => ellipse_path(cr, local),
        }
        cr.set_matrix(m);
    }
}

impl Annotation for Redaction {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        self.rect().inflate(1.0).rotated_bounds(self.rotation)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let local = self.arrange().to_local(point);
        let tolerance = tolerance.max(0.0);
        match self.shape {
            RedactionShape::Rectangle => self.rect().inflate(tolerance).contains(local),
            RedactionShape::Ellipse => {
                let c = self.rect().centre();
                let (rx, ry) = (self.width / 2.0 + tolerance, self.height / 2.0 + tolerance);
                rx > 0.0
                    && ry > 0.0
                    && ((local.x - c.x) / rx).powi(2) + ((local.y - c.y) / ry).powi(2) <= 1.0
            }
        }
    }

    fn render(&self, cr: &cairo::Context, ctx: &RenderContext<'_>) -> Result<(), cairo::Error> {
        let Some(effect) = self.effect() else {
            self.path(cr);
            self.color.set_source(cr);
            return cr.fill();
        };
        let image = &ctx.document.image().bitmap;
        let image_rect = IRect::new(0, 0, image.width(), image.height());
        let Some(region) = IRect::covering(&self.bounds()).intersection(&image_rect) else {
            return Ok(());
        };
        let Some(processed) = redact::processed(image, region, effect) else {
            return Ok(());
        };
        let r = image_rect.to_rect();
        cr.rectangle(r.x, r.y, r.width, r.height);
        cr.clip();
        self.path(cr);
        cr.clip();
        cr.set_source_surface(
            &processed.surface,
            f64::from(processed.rect.x),
            f64::from(processed.rect.y),
        )?;
        // Blocks stay crisp at every zoom; a blur is smooth anyway.
        cr.source().set_filter(match effect {
            Effect::Pixelate { .. } => cairo::Filter::Nearest,
            Effect::Blur { .. } => cairo::Filter::Good,
        });
        cr.paint()
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
            Property::new("redaction", PropertyValue::Choice(self.kind.key().into())),
            Property::new("intensity", PropertyValue::Number(self.intensity)),
            Property::new("shape", PropertyValue::Choice(self.shape.key().into())),
            Property::new("color", PropertyValue::Color(self.color)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "redaction" => self.kind = common::choice(key, v, RedactionKind::from_key)?,
            "intensity" => self.intensity = value::number(key, v, 0.0..=100.0)?,
            "shape" => self.shape = common::choice(key, v, RedactionShape::from_key)?,
            "color" => self.color = value::color(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::Target;
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    /// A capture with fine black-and-white detail ("text").
    fn detailed() -> Document {
        Document::new(
            Bitmap::draw(120, 60, |cr| {
                for x in 0..120 {
                    for y in 0..60 {
                        let c = if (x + y) % 3 == 0 {
                            palette::INK
                        } else {
                            palette::PAPER
                        };
                        c.set_source(cr);
                        cr.rectangle(f64::from(x), f64::from(y), 1.0, 1.0);
                        cr.fill()?;
                    }
                }
                Ok(())
            })
            .unwrap(),
        )
    }

    fn render(doc: &Document, r: &Redaction) -> Bitmap {
        let ctx = RenderContext {
            document: doc,
            zoom: 1.0,
            target: Target::Export,
        };
        Bitmap::draw(120, 60, |cr| {
            cr.set_source_surface(doc.image().bitmap.surface(), 0.0, 0.0)?;
            cr.paint()?;
            r.render(cr, &ctx)
        })
        .unwrap()
    }

    /// Distinct colours in a block of the output.
    fn distinct(out: &Bitmap, x0: i32, y0: i32, n: i32) -> usize {
        let mut seen = std::collections::HashSet::new();
        for x in x0..x0 + n {
            for y in y0..y0 + n {
                seen.insert(out.pixel(x, y));
            }
        }
        seen.len()
    }

    #[test]
    fn defaults_follow_the_redline() {
        let r = Redaction::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(
            (r.kind, r.intensity, r.shape),
            (RedactionKind::Pixelate, 70.0, RedactionShape::Rectangle)
        );
        assert_eq!(r.color, palette::INK);
    }

    #[test]
    fn pixelate_blur_and_solid_hide_the_detail() {
        let doc = detailed();
        let mut r = Redaction::new(Rect::new(20.0, 10.0, 66.0, 44.0));
        // Pixelate: one colour per 22 px block; outside the redaction the detail is intact.
        let out = render(&doc, &r);
        assert_eq!(distinct(&out, 22, 12, 18), 1);
        assert_eq!(distinct(&out, 2, 2, 10), 2);
        // Blur: the black/white pattern becomes near-uniform grey.
        r.kind = RedactionKind::Blur;
        let out = render(&doc, &r);
        let p = out.pixel(50, 30).unwrap();
        assert!(p.r > 140 && p.r < 220, "{p:?}");
        let q = out.pixel(52, 31).unwrap();
        assert!((i32::from(p.r) - i32::from(q.r)).abs() < 12);
        // Solid: Ink.
        r.kind = RedactionKind::Solid;
        let out = render(&doc, &r);
        assert_eq!(distinct(&out, 22, 12, 18), 1);
        assert_eq!(out.pixel(30, 20), Some(palette::INK));
        // An ellipse leaves its corners alone.
        r.shape = RedactionShape::Ellipse;
        let out = render(&doc, &r);
        assert_eq!(out.pixel(53, 32), Some(palette::INK));
        assert_ne!(out.pixel(21, 11), Some(palette::INK));
        assert!(!r.hit_test(Point::new(21.0, 11.0), 0.0));
    }

    #[test]
    fn canvas_and_export_show_the_same_effect() {
        let doc = detailed();
        let r = Redaction::new(Rect::new(20.0, 10.0, 66.0, 44.0));
        let at_200 = {
            let ctx = RenderContext {
                document: &doc,
                zoom: 2.0,
                target: Target::Screen,
            };
            Bitmap::draw(240, 120, |cr| {
                cr.scale(2.0, 2.0);
                r.render(cr, &ctx)
            })
            .unwrap()
        };
        let at_100 = render(&doc, &r);
        for (x, y) in [(25, 15), (60, 40), (80, 50)] {
            assert_eq!(at_200.pixel(2 * x, 2 * y), at_100.pixel(x, y), "({x}, {y})");
        }
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut r = Redaction::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        let before = r.clone();
        for p in r.clone().properties() {
            r.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(r, before);
        r.set_property("redaction", &PropertyValue::Choice("solid".into()))
            .unwrap();
        assert_eq!(r.kind, RedactionKind::Solid);
        assert!(r
            .set_property("intensity", &PropertyValue::Number(101.0))
            .is_err());
    }
}
