//! Highlighter (spec annotation-tools: Highlighter; mockup 14): a semi-transparent rectangle
//! drawn with cairo's MULTIPLY operator, so what is underneath — text in particular — stays
//! legible: dark pixels stay dark, light pixels take the highlight's colour (CSS
//! `mix-blend-mode: multiply` with opacity, as in the mockup).

use serde::{Deserialize, Serialize};

use crate::document::annotation::{
    value, Annotation, Property, PropertyError, PropertyValue, RenderContext,
};
use crate::document::color::Rgba;
use crate::document::geometry::{Arrange, Point, Rect};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Highlighter {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Degrees clockwise around the centre.
    pub rotation: f64,
    pub color: Rgba,
    /// 0.0–1.0.
    pub opacity: f64,
}

impl Highlighter {
    pub const TYPE: &'static str = "highlighter";
    /// Redline: rgb(255 222 0).
    pub const YELLOW: Rgba = Rgba::opaque(0xFF, 0xDE, 0x00);

    /// A new highlight with the tool defaults (redline: rgb(255 222 0) at 45 %, Multiply).
    pub fn new(rect: Rect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            rotation: 0.0,
            color: Self::YELLOW,
            opacity: 0.45,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }
}

impl Annotation for Highlighter {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        self.rect().inflate(1.0).rotated_bounds(self.rotation)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let local = self.arrange().to_local(point);
        self.rect().inflate(tolerance.max(0.0)).contains(local)
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        let alpha = f64::from(self.color.a) / 255.0 * self.opacity.clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return Ok(());
        }
        let c = self.rect().centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        cr.rectangle(
            -self.width / 2.0,
            -self.height / 2.0,
            self.width,
            self.height,
        );
        let (r, g, b, _) = self.color.to_f64();
        cr.set_source_rgba(r, g, b, alpha);
        cr.set_operator(cairo::Operator::Multiply);
        cr.fill()
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
            Property::new("color", PropertyValue::Color(self.color)),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "color" => self.color = value::color(key, v)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
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
    use crate::document::color::palette;
    use crate::document::Document;

    #[test]
    fn text_stays_legible_through_the_highlight() {
        // Spec "Text readable": black text under the highlight stays black (multiply), white
        // paper turns yellow; plain alpha blending would have greyed the text.
        let doc = Document::new(Bitmap::filled(100, 40, Rgba::WHITE).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        let h = Highlighter::new(Rect::new(0.0, 0.0, 100.0, 40.0));
        let out = Bitmap::draw(100, 40, |cr| {
            palette::PAPER.set_source(cr);
            cr.paint()?;
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.rectangle(40.0, 10.0, 20.0, 20.0); // a "glyph"
            cr.fill()?;
            h.render(cr, &ctx)
        })
        .unwrap();
        let ink = out.pixel(50, 20).unwrap();
        assert_eq!((ink.r, ink.g, ink.b), (0, 0, 0), "text stays black");
        let paper = out.pixel(10, 20).unwrap();
        // White × 45 % yellow: (255, 255 − 0.45·33, 255 − 0.45·255).
        assert_eq!(paper.r, 255);
        assert!((paper.g as i32 - 240).abs() <= 2, "{paper:?}");
        assert!((paper.b as i32 - 140).abs() <= 2, "{paper:?}");
        assert_eq!(h.color, Highlighter::YELLOW);
        assert_eq!(h.opacity, 0.45);
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut h = Highlighter::new(Rect::new(0.0, 0.0, 10.0, 10.0));
        let before = h.clone();
        for p in h.clone().properties() {
            h.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(h, before);
        assert!(h
            .set_property("opacity", &PropertyValue::Number(2.0))
            .is_err());
    }
}
