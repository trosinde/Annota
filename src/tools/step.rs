//! Step markers (spec annotation-tools: Step; mockups 11–13): numbered or lettered markers in
//! sequences.
//!
//! Each marker stores which sequence it belongs to (`sequence`, an id; Restart Sequence starts
//! a new one), its position in that sequence (`number`, from 1) and how the sequence counts
//! (`counting`: 1, 2, 3 · A, B, C · a, b, c · I, II, III). Its label is that position in that
//! style unless a value was typed for this marker only (`value`). The numbering logic — the next
//! number when placing, renumbering the same sequence when markers are deleted — works on the
//! whole document and lives in `crate::editing::steps`.

use serde::{Deserialize, Serialize};

use super::common::{choices, corner_handles, shadow_scaled, style_scaled, styled, with_shadow};
use super::text::{draw_layout, layout, Alignment, TextStyle, DEFAULT_FAMILY};
use crate::document::annotation::{
    drag_box_handle, value, Annotation, Handle, HandleId, Modifiers, Property, PropertyError,
    PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::Shadow;
use crate::render::shadow::rounded_rect;

pub const MIN_RADIUS: f64 = 6.0;
pub const MAX_RADIUS: f64 = 200.0;
pub const MAX_RING: f64 = 20.0;

choices!(
    /// The marker's outline (sidebar: Circle / Square / Rounded Square / Pin).
    StepShape {
        Circle => "circle",
        Square => "square",
        RoundedSquare => "rounded_square",
        Pin => "pin",
    }
);

choices!(
    /// How a sequence counts (sidebar: Sequence — 1, 2, 3 · A, B, C · a, b, c · I, II, III).
    Counting {
        Numbers => "numbers",
        UpperLetters => "upper_letters",
        LowerLetters => "lower_letters",
        Roman => "roman",
    }
);

impl Counting {
    /// The label of position `n` (from 1): 27 → "AA" in letters (like spreadsheet columns),
    /// Roman numerals up to 3999, digits beyond.
    pub fn format(self, n: u32) -> String {
        match self {
            Counting::Numbers => n.to_string(),
            Counting::UpperLetters | Counting::LowerLetters => {
                let base = if self == Counting::UpperLetters {
                    b'A'
                } else {
                    b'a'
                };
                let mut n = n.max(1);
                let mut out = Vec::new();
                while n > 0 {
                    let rem = ((n - 1) % 26) as u8;
                    out.push((base + rem) as char);
                    n = (n - 1) / 26;
                }
                out.iter().rev().collect()
            }
            Counting::Roman => {
                if n == 0 || n > 3999 {
                    return n.to_string();
                }
                const TABLE: [(u32, &str); 13] = [
                    (1000, "M"),
                    (900, "CM"),
                    (500, "D"),
                    (400, "CD"),
                    (100, "C"),
                    (90, "XC"),
                    (50, "L"),
                    (40, "XL"),
                    (10, "X"),
                    (9, "IX"),
                    (5, "V"),
                    (4, "IV"),
                    (1, "I"),
                ];
                let mut n = n;
                let mut out = String::new();
                for (value, digits) in TABLE {
                    while n >= value {
                        out.push_str(digits);
                        n -= value;
                    }
                }
                out
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Centre of the marker's head (a pin's point is below it).
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub shape: StepShape,
    pub color: Rgba,
    pub label_color: Rgba,
    pub ring_color: Rgba,
    /// 0 = no ring.
    pub ring_width: f64,
    /// The sequence the marker belongs to.
    pub sequence: u32,
    /// Position in the sequence, from 1.
    pub number: u32,
    pub counting: Counting,
    /// A value typed for this marker only; `None`: the label follows `number`.
    pub value: Option<String>,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
}

impl Step {
    pub const TYPE: &'static str = "step";

    /// Marker `number` of `sequence` at `at` with the tool defaults (redline: circle, Coral,
    /// 28 px radius, white 800-weight label, 3 px white ring, sequence 1, 2, 3, shadow on).
    pub fn new(at: Point, sequence: u32, number: u32) -> Self {
        Self {
            x: at.x,
            y: at.y,
            radius: 28.0,
            shape: StepShape::Circle,
            color: palette::CORAL,
            label_color: palette::PAPER,
            ring_color: palette::PAPER,
            ring_width: 3.0,
            sequence,
            number,
            counting: Counting::Numbers,
            value: None,
            opacity: 1.0,
            shadow: Shadow::DEFAULT,
        }
    }

    /// What the marker reads.
    pub fn label(&self) -> String {
        match &self.value {
            Some(v) => v.clone(),
            None => self.counting.format(self.number),
        }
    }

    pub fn centre(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Radius of a pin's round head (mockup 12: 24 for size 28).
    fn head_radius(&self) -> f64 {
        match self.shape {
            StepShape::Pin => self.radius * 24.0 / 28.0,
            _ => self.radius,
        }
    }

    /// How far a pin's point is below the head's centre.
    fn pin_tip(&self) -> f64 {
        self.head_radius() * 1.9
    }

    fn path(&self, cr: &cairo::Context) {
        let (x, y, r) = (self.x, self.y, self.radius);
        match self.shape {
            StepShape::Circle => {
                cr.new_sub_path();
                cr.arc(x, y, r, 0.0, std::f64::consts::TAU);
                cr.close_path();
            }
            StepShape::Square => cr.rectangle(x - r, y - r, 2.0 * r, 2.0 * r),
            StepShape::RoundedSquare => rounded_rect(cr, x - r, y - r, 2.0 * r, 2.0 * r, 0.4 * r),
            StepShape::Pin => {
                // Mockup 12's teardrop: a round head narrowing to a point below.
                let h = self.head_radius();
                let tip = y + self.pin_tip();
                cr.move_to(x, tip);
                cr.curve_to(x - 0.5 * h, y + 1.25 * h, x - h, y + 0.75 * h, x - h, y);
                cr.arc(x, y, h, std::f64::consts::PI, std::f64::consts::TAU);
                cr.curve_to(x + h, y + 0.75 * h, x + 0.5 * h, y + 1.25 * h, x, tip);
                cr.close_path();
            }
        }
    }

    fn body_bounds(&self) -> Rect {
        let r = self.radius;
        let mut b = Rect::new(self.x - r, self.y - r, 2.0 * r, 2.0 * r);
        if self.shape == StepShape::Pin {
            b.height = r + self.pin_tip();
        }
        b.inflate(self.ring_width.max(0.0) / 2.0 + 1.0)
    }

    fn label_style(&self, size: f64) -> TextStyle {
        TextStyle {
            family: DEFAULT_FAMILY.to_string(),
            size,
            weight: pango::Weight::Ultrabold,
            italic: false,
            underline: false,
            strikethrough: false,
            alignment: Alignment::Center,
            wrap: None,
        }
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        self.path(cr);
        self.color.set_source(cr);
        cr.fill_preserve()?;
        if self.ring_width > 0.0 && self.ring_color.a > 0 {
            self.ring_color.set_source(cr);
            cr.set_line_width(self.ring_width);
            cr.set_line_join(cairo::LineJoin::Round);
            cr.stroke()?;
        } else {
            cr.new_path();
        }
        let label = self.label();
        if label.is_empty() {
            return Ok(());
        }
        // Mockups 11/12: the label is 1.04 × the head's radius, smaller if it would not fit.
        let head = self.head_radius();
        let mut size = head * 1.04;
        let Some(mut layout) = layout(&label, &self.label_style(size)) else {
            return Ok(());
        };
        let room = head * 1.5;
        let (w, _) = layout.pixel_size();
        if f64::from(w) > room {
            size *= room / f64::from(w);
            match super::text::layout(&label, &self.label_style(size)) {
                Some(l) => layout = l,
                None => return Ok(()),
            }
        }
        let (_, logical) = layout.extents();
        let s = f64::from(pango::SCALE);
        let (lw, lh) = (
            f64::from(logical.width()) / s,
            f64::from(logical.height()) / s,
        );
        cr.translate(self.x - lw / 2.0, self.y - lh / 2.0);
        draw_layout(cr, &layout, self.label_color, None)
    }
}

impl Annotation for Step {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let reach = self.ring_width.max(0.0) / 2.0 + tolerance.max(0.0);
        let r = self.radius + reach;
        match self.shape {
            StepShape::Circle => point.distance(self.centre()) <= r,
            StepShape::Square | StepShape::RoundedSquare => {
                (point.x - self.x).abs() <= r && (point.y - self.y).abs() <= r
            }
            StepShape::Pin => {
                let h = self.head_radius() + reach;
                point.distance(self.centre()) <= h
                    || ((point.x - self.x).abs() <= h * 0.6
                        && point.y >= self.y
                        && point.y <= self.y + self.pin_tip() + reach)
            }
        }
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        let r = self.radius;
        Arrange::new(Rect::new(self.x - r, self.y - r, 2.0 * r, 2.0 * r), 0.0)
    }

    /// Markers stay upright and round: the new box's centre and its shorter side.
    fn set_arrange(&mut self, arrange: Arrange) {
        let c = arrange.rect.centre();
        self.x = c.x;
        self.y = c.y;
        self.radius =
            (arrange.rect.width.min(arrange.rect.height) / 2.0).clamp(MIN_RADIUS, MAX_RADIUS);
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    /// The marker's place follows the document; it stays upright, and its size changes only
    /// with "Scale annotations".
    fn transform(&mut self, t: &Transform) {
        let c = t.apply(self.centre());
        self.x = c.x;
        self.y = c.y;
        self.radius = style_scaled(self.radius, t).clamp(MIN_RADIUS, MAX_RADIUS);
        self.ring_width = style_scaled(self.ring_width, t).min(MAX_RING);
        shadow_scaled(&mut self.shadow, t);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("shape", PropertyValue::Choice(self.shape.key().into())),
            Property::new("color", PropertyValue::Color(self.color)),
            Property::new("radius", PropertyValue::Number(self.radius)),
            Property::new("label_color", PropertyValue::Color(self.label_color)),
            Property::new("ring_color", PropertyValue::Color(self.ring_color)),
            Property::new("ring_width", PropertyValue::Number(self.ring_width)),
            Property::new(
                "counting",
                PropertyValue::Choice(self.counting.key().into()),
            ),
            // Empty text: the label follows the sequence again.
            Property::new(
                "value",
                PropertyValue::Text(self.value.clone().unwrap_or_default()),
            ),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "shape" => self.shape = super::common::choice(key, v, StepShape::from_key)?,
            "color" => self.color = value::color(key, v)?,
            "radius" => self.radius = value::number(key, v, MIN_RADIUS..=MAX_RADIUS)?,
            "label_color" => self.label_color = value::color(key, v)?,
            "ring_color" => self.ring_color = value::color(key, v)?,
            "ring_width" => self.ring_width = value::number(key, v, 0.0..=MAX_RING)?,
            "counting" => self.counting = super::common::choice(key, v, Counting::from_key)?,
            "value" => {
                let text = super::common::text(key, v)?;
                self.value = (!text.trim().is_empty()).then(|| text.trim().to_string());
            }
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    /// Four corner handles that scale the marker.
    fn handles(&self) -> Vec<Handle> {
        corner_handles(&self.arrange())
    }

    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        drag_box_handle(self, handle, to, modifiers, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::{Compass, Target};
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    #[test]
    fn counting_styles() {
        let f = |c: Counting, ns: &[u32]| ns.iter().map(|&n| c.format(n)).collect::<Vec<_>>();
        assert_eq!(f(Counting::Numbers, &[1, 2, 3]), ["1", "2", "3"]);
        assert_eq!(
            f(Counting::UpperLetters, &[1, 2, 26, 27, 52]),
            ["A", "B", "Z", "AA", "AZ"]
        );
        assert_eq!(f(Counting::LowerLetters, &[1, 3]), ["a", "c"]);
        assert_eq!(
            f(Counting::Roman, &[1, 2, 3, 4, 9, 14, 1994]),
            ["I", "II", "III", "IV", "IX", "XIV", "MCMXCIV"]
        );
    }

    #[test]
    fn defaults_and_manual_value() {
        let mut s = Step::new(Point::new(50.0, 50.0), 1, 7);
        assert_eq!(
            (s.shape, s.color, s.radius),
            (StepShape::Circle, palette::CORAL, 28.0)
        );
        assert_eq!(
            (s.label_color, s.ring_color, s.ring_width),
            (palette::PAPER, palette::PAPER, 3.0)
        );
        assert_eq!(s.label(), "7");
        s.set_property("value", &PropertyValue::Text("7b".into()))
            .unwrap();
        assert_eq!(s.label(), "7b");
        s.set_property("value", &PropertyValue::Text(String::new()))
            .unwrap();
        assert_eq!(s.value, None);
        s.counting = Counting::UpperLetters;
        assert_eq!(s.label(), "G");
    }

    #[test]
    fn shapes_render_with_ring_and_label() {
        let doc = Document::new(Bitmap::filled(200, 200, Rgba::WHITE).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        for shape in StepShape::ALL {
            let mut s = Step::new(Point::new(100.0, 80.0), 1, 1);
            s.shape = *shape;
            s.shadow.enabled = false;
            let out = Bitmap::draw(200, 200, |cr| {
                palette::INK.set_source(cr);
                cr.paint()?;
                s.render(cr, &ctx)
            })
            .unwrap();
            // Coral body left of the label, the white ring at the head's left edge.
            assert_eq!(out.pixel(80, 80), Some(palette::CORAL), "{shape:?}");
            let left = (100.0 - s.head_radius()) as i32;
            assert_eq!(out.pixel(left, 80), Some(palette::PAPER), "{shape:?}");
            // Some white label pixels near the centre.
            let label = (90..110)
                .flat_map(|x| (70..90).map(move |y| (x, y)))
                .filter(|&(x, y)| out.pixel(x, y) == Some(palette::PAPER))
                .count();
            assert!(label > 10, "{shape:?}: {label}");
            assert!(s.hit_test(Point::new(100.0, 80.0), 0.0));
        }
        let pin = Step {
            shape: StepShape::Pin,
            ..Step::new(Point::new(100.0, 80.0), 1, 1)
        };
        assert!(
            pin.hit_test(Point::new(100.0, 80.0 + 40.0), 0.0),
            "the point"
        );
        assert!(pin.bounds().bottom() > 80.0 + 45.0);
    }

    #[test]
    fn corner_handles_scale_it_round() {
        let mut s = Step::new(Point::new(100.0, 100.0), 1, 1);
        assert_eq!(s.handles().len(), 4);
        s.drag_handle(
            HandleId::Resize(Compass::SE),
            Point::new(160.0, 140.0),
            Modifiers::default(),
        );
        // The NW corner (72, 72) stays; ratio kept by the larger change: 88 px → radius 44.
        assert_eq!(s.radius, 44.0);
        assert_eq!(s.centre(), Point::new(116.0, 116.0));
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut s = Step::new(Point::new(10.0, 10.0), 2, 3);
        let before = s.clone();
        for p in s.clone().properties() {
            s.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(s, before);
        assert!(s
            .set_property("counting", &PropertyValue::Choice("greek".into()))
            .is_err());
        assert!(s
            .set_property("radius", &PropertyValue::Number(1.0))
            .is_err());
    }
}
