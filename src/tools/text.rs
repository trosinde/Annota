//! Text objects (spec annotation-tools: Text; mockups 08, 20) and the text layout every type
//! with text uses (callouts, step labels).
//!
//! **Layout.** Text is laid out with Pango through one font context per thread whose metrics are
//! not hinted, so a layout has the same size at every zoom level and in export (CLAUDE.md §2:
//! WYSIWYG). Sizes are pixel sizes in document units. A [`TextBlock`] is a laid-out text placed
//! in an object's (possibly rotated) frame; it answers what in-place editing needs: caret and
//! selection geometry and the text position under the pointer.
//!
//! **Text objects** grow with their text (no wrapping: lines are as typed). Their box is the
//! text's logical extent; resizing it scales the font, as text cannot stretch (corner and edge
//! handles keep the ratio). A document transform moves the text's centre; "Scale annotations"
//! scales the font, outline and shadow — mirroring never mirrors the text (it stays readable).

use std::cell::Cell;
use std::hash::{Hash, Hasher};

use pango::prelude::*;
use serde::{Deserialize, Serialize};

use super::common::{self, choices, shadow_scaled, style_scaled, styled, with_shadow};
use crate::document::annotation::{
    drag_box_handle, value, Annotation, HandleId, Modifiers, Property, PropertyError,
    PropertyValue, RenderContext,
};
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Arrange, Point, Rect, Transform};
use crate::document::style::Shadow;

/// The UI font of the kit (redline: Adwaita Sans); Pango falls back to the system's sans-serif
/// font where it is not installed (Ubuntu 24.04 ships Cantarell).
pub const DEFAULT_FAMILY: &str = "Adwaita Sans";
pub const MIN_FONT_SIZE: f64 = 4.0;
pub const MAX_FONT_SIZE: f64 = 1000.0;
pub const MAX_OUTLINE: f64 = 40.0;

choices!(
    /// Paragraph alignment.
    Alignment {
        Left => "left",
        Center => "center",
        Right => "right",
    }
);

impl Alignment {
    fn pango(self) -> pango::Alignment {
        match self {
            Alignment::Left => pango::Alignment::Left,
            Alignment::Center => pango::Alignment::Center,
            Alignment::Right => pango::Alignment::Right,
        }
    }
}

/// How a text is laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: String,
    /// Pixel size (document units).
    pub size: f64,
    pub weight: pango::Weight,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub alignment: Alignment,
    /// Wrap at this width (document units); `None`: lines as typed.
    pub wrap: Option<f64>,
}

impl TextStyle {
    fn key(&self, text: &str) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        self.family.hash(&mut h);
        self.size.to_bits().hash(&mut h);
        format!("{:?}", self.weight).hash(&mut h);
        (self.italic, self.underline, self.strikethrough).hash(&mut h);
        self.alignment.key().hash(&mut h);
        self.wrap.map(f64::to_bits).hash(&mut h);
        h.finish()
    }
}

fn new_context() -> Option<pango::Context> {
    let map = pangocairo::FontMap::new();
    let context = map.create_context();
    let mut options = cairo::FontOptions::new().ok()?;
    options.set_hint_metrics(cairo::HintMetrics::Off);
    options.set_hint_style(cairo::HintStyle::None);
    options.set_antialias(cairo::Antialias::Gray);
    pangocairo::functions::context_set_font_options(&context, Some(&options));
    pangocairo::functions::context_set_resolution(&context, 96.0);
    Some(context)
}

thread_local! {
    /// Font context for measuring and drawing text: unhinted metrics, so sizes do not depend
    /// on the zoom level.
    static CONTEXT: Option<pango::Context> = new_context();
}

fn scale() -> f64 {
    f64::from(pango::SCALE)
}

/// Lays out `text` in `style`.
pub fn layout(text: &str, style: &TextStyle) -> Option<pango::Layout> {
    CONTEXT.with(|context| {
        let context = context.as_ref()?;
        let layout = pango::Layout::new(context);
        let mut desc = pango::FontDescription::new();
        desc.set_family(&style.family);
        desc.set_absolute_size(style.size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE) * scale());
        desc.set_weight(style.weight);
        if style.italic {
            desc.set_style(pango::Style::Italic);
        }
        layout.set_font_description(Some(&desc));
        layout.set_alignment(style.alignment.pango());
        if let Some(width) = style.wrap {
            layout.set_width((width.max(1.0) * scale()) as i32);
            layout.set_wrap(pango::WrapMode::WordChar);
        }
        let attrs = pango::AttrList::new();
        if style.underline {
            attrs.insert(pango::AttrInt::new_underline(pango::Underline::Single));
        }
        if style.strikethrough {
            attrs.insert(pango::AttrInt::new_strikethrough(true));
        }
        layout.set_attributes(Some(&attrs));
        layout.set_text(text);
        Some(layout)
    })
}

/// Measured size of a laid-out text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measure {
    /// Logical width and height (line boxes): the text's box.
    pub width: f64,
    pub height: f64,
    /// Where ink is, relative to the layout's origin (italic overhangs, outlines aside).
    pub ink: Rect,
}

/// Measures `text` in `style` (cached by the caller).
pub fn measure(text: &str, style: &TextStyle) -> Measure {
    let Some(layout) = layout(text, style) else {
        // No fonts at all: one line per line of text, a rough width.
        let lines = text.split('\n').count().max(1) as f64;
        let longest = text
            .split('\n')
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(0) as f64;
        let (w, h) = (longest * style.size * 0.55, lines * style.size * 1.2);
        return Measure {
            width: style.wrap.unwrap_or(w),
            height: h,
            ink: Rect::new(0.0, 0.0, w, h),
        };
    };
    let (ink, logical) = layout.extents();
    let r = |rect: pango::Rectangle| {
        Rect::new(
            f64::from(rect.x()) / scale(),
            f64::from(rect.y()) / scale(),
            f64::from(rect.width()) / scale(),
            f64::from(rect.height()) / scale(),
        )
    };
    let logical = r(logical);
    Measure {
        width: style.wrap.unwrap_or(logical.right().max(0.0)),
        height: logical.bottom().max(0.0),
        ink: r(ink),
    }
}

/// Draws a laid-out text with its top-left at the current origin: an outline (stroked behind
/// the glyphs, like SVG's `paint-order: stroke`) if `outline` has width and colour, then the
/// glyphs in `color`.
pub fn draw_layout(
    cr: &cairo::Context,
    layout: &pango::Layout,
    color: Rgba,
    outline: Option<(Rgba, f64)>,
) -> Result<(), cairo::Error> {
    if let Some((outline_color, width)) = outline.filter(|(c, w)| c.a > 0 && *w > 0.0) {
        cr.new_path();
        cr.move_to(0.0, 0.0);
        pangocairo::functions::layout_path(cr, layout);
        outline_color.set_source(cr);
        cr.set_line_width(width);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.set_dash(&[], 0.0);
        cr.stroke()?;
    }
    cr.move_to(0.0, 0.0);
    color.set_source(cr);
    pangocairo::functions::show_layout(cr, layout);
    cr.new_path();
    Ok(())
}

/// Clamps `index` to a char boundary of `text` at or before it.
pub fn floor_boundary(text: &str, index: usize) -> usize {
    let mut i = index.min(text.len());
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// A laid-out text placed in an object's frame, for in-place editing.
#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    pub text: String,
    pub style: TextStyle,
    /// Top-left of the layout in document space before the frame's rotation.
    pub origin: Point,
    /// The frame the text turns with (around its centre).
    pub frame: Arrange,
}

impl TextBlock {
    fn layout(&self) -> Option<pango::Layout> {
        layout(&self.text, &self.style)
    }

    fn doc(&self, x: f64, y: f64) -> Point {
        self.frame
            .to_document(Point::new(self.origin.x + x, self.origin.y + y))
    }

    fn local(&self, doc: Point) -> Point {
        let p = self.frame.to_local(doc);
        Point::new(p.x - self.origin.x, p.y - self.origin.y)
    }

    /// The caret before byte `index`: its top and bottom in document space.
    pub fn caret(&self, index: usize) -> (Point, Point) {
        let index = floor_boundary(&self.text, index);
        match self.layout() {
            Some(layout) => {
                let pos = layout.index_to_pos(index as i32);
                let x = f64::from(pos.x()) / scale();
                let y = f64::from(pos.y()) / scale();
                let h = f64::from(pos.height()) / scale();
                (self.doc(x, y), self.doc(x, y + h))
            }
            None => (self.doc(0.0, 0.0), self.doc(0.0, self.style.size)),
        }
    }

    /// The selected byte range `start..end` as quadrilaterals (one per line) in document space.
    pub fn selection(&self, start: usize, end: usize) -> Vec<[Point; 4]> {
        let (start, end) = (start.min(end), start.max(end));
        let Some(layout) = self.layout() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut iter = layout.iter();
        loop {
            if let Some(line) = iter.line_readonly() {
                let (top, bottom) = iter.line_yrange();
                let (top, bottom) = (f64::from(top) / scale(), f64::from(bottom) / scale());
                let line_start = line.start_index().max(0) as usize;
                let line_end = line_start + line.length().max(0) as usize;
                let (s, e) = (start.max(line_start), end.min(line_end));
                // A selected line break shows as a little room after the line.
                let breaks = end > line_end && start <= line_end;
                if s < e || breaks {
                    let ranges = line.x_ranges(s as i32, e.max(s) as i32);
                    let line_x = f64::from(iter.line_extents().1.x()) / scale();
                    let mut pairs: Vec<(f64, f64)> = ranges
                        .chunks(2)
                        .filter_map(|c| match c {
                            [a, b] => Some((f64::from(*a) / scale(), f64::from(*b) / scale())),
                            _ => None,
                        })
                        .collect();
                    if s >= e {
                        let x =
                            line_x + f64::from(line.index_to_x(line_end as i32, false)) / scale();
                        pairs = vec![(x, x)];
                    }
                    for (x0, x1) in pairs {
                        let x1 = if breaks {
                            x1 + self.style.size * 0.3
                        } else {
                            x1
                        };
                        out.push([
                            self.doc(x0, top),
                            self.doc(x1, top),
                            self.doc(x1, bottom),
                            self.doc(x0, bottom),
                        ]);
                    }
                }
            }
            if !iter.next_line() {
                break;
            }
        }
        out
    }

    /// The byte index nearest to `doc` (a click while editing).
    pub fn index_at(&self, doc: Point) -> usize {
        let local = self.local(doc);
        let Some(layout) = self.layout() else {
            return self.text.len();
        };
        let (_, index, trailing) =
            layout.xy_to_index((local.x * scale()) as i32, (local.y * scale()) as i32);
        let mut i = floor_boundary(&self.text, index.max(0) as usize);
        for _ in 0..trailing.max(0) {
            match self.text[i..].chars().next() {
                Some(c) => i += c.len_utf8(),
                None => break,
            }
        }
        i
    }

    /// The index `lines` lines up (negative) or down from `index`, at the same x; the start or
    /// end of the text past the first or last line.
    pub fn vertical(&self, index: usize, lines: i32) -> usize {
        let Some(layout) = self.layout() else {
            return index;
        };
        let index = floor_boundary(&self.text, index);
        let (line, x) = layout.index_to_line_x(index as i32, false);
        let target = line + lines;
        if target < 0 {
            return 0;
        }
        if target >= layout.line_count() {
            return self.text.len();
        }
        let Some(target_line) = layout.line_readonly(target) else {
            return index;
        };
        let hit = target_line.x_to_index(x);
        let mut i = floor_boundary(&self.text, hit.index().max(0) as usize);
        for _ in 0..hit.trailing().max(0) {
            match self.text[i..].chars().next() {
                Some(c) => i += c.len_utf8(),
                None => break,
            }
        }
        // Never past the line's own end (onto the next line's start).
        let line_end = (target_line.start_index() + target_line.length()).max(0) as usize;
        i.min(floor_boundary(&self.text, line_end))
    }

    /// Start and end (byte indices) of the visual line holding `index` (Home, End).
    pub fn line_range(&self, index: usize) -> (usize, usize) {
        let Some(layout) = self.layout() else {
            return (0, self.text.len());
        };
        let index = floor_boundary(&self.text, index);
        let (line, _) = layout.index_to_line_x(index as i32, false);
        match layout.line_readonly(line) {
            Some(l) => {
                let start = l.start_index().max(0) as usize;
                let end = start + l.length().max(0) as usize;
                (
                    floor_boundary(&self.text, start),
                    floor_boundary(&self.text, end),
                )
            }
            None => (0, self.text.len()),
        }
    }

    /// The box the editing frame is drawn around (document space, with the frame's rotation).
    pub fn bounds(&self) -> Arrange {
        let m = measure(&self.text, &self.style);
        let local = Rect::new(self.origin.x, self.origin.y, m.width, m.height);
        // The frame rotates around its own centre; express the text box in the same rotation.
        let c = self.frame.to_document(local.centre());
        Arrange::new(
            Rect::new(
                c.x - local.width / 2.0,
                c.y - local.height / 2.0,
                local.width,
                local.height,
            ),
            self.frame.rotation,
        )
    }
}

/// Objects whose text is edited in place (text, callout).
pub trait EditableText {
    fn text(&self) -> &str;
    fn set_text(&mut self, text: String);
    /// The text as laid out on the canvas.
    fn text_block(&self) -> TextBlock;
}

/// The editable text of `object`, if it has one.
pub fn editable(object: &dyn Annotation) -> Option<&dyn EditableText> {
    let any = object.as_any();
    if let Some(t) = any.downcast_ref::<Text>() {
        return Some(t);
    }
    any.downcast_ref::<super::callout::Callout>()
        .map(|c| c as &dyn EditableText)
}

/// Mutable access to the editable text of `object`.
pub fn editable_mut(object: &mut dyn Annotation) -> Option<&mut dyn EditableText> {
    let any = object.as_any_mut();
    if any.is::<Text>() {
        return any
            .downcast_mut::<Text>()
            .map(|t| t as &mut dyn EditableText);
    }
    any.downcast_mut::<super::callout::Callout>()
        .map(|c| c as &mut dyn EditableText)
}

/// Cached measurement; never part of equality or the file.
#[derive(Clone, Debug, Default)]
pub(crate) struct MeasureCache(Cell<Option<(u64, Measure)>>);

impl PartialEq for MeasureCache {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl MeasureCache {
    pub(crate) fn get(&self, text: &str, style: &TextStyle) -> Measure {
        let key = style.key(text);
        if let Some((k, m)) = self.0.get() {
            if k == key {
                return m;
            }
        }
        let m = measure(text, style);
        self.0.set(Some((key, m)));
        m
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
    /// Top-left of the text's box (before rotation).
    pub x: f64,
    pub y: f64,
    /// Degrees clockwise around the box's centre.
    pub rotation: f64,
    pub font_family: String,
    /// Pixel size.
    pub font_size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub alignment: Alignment,
    pub color: Rgba,
    /// Transparent = no outline.
    pub outline_color: Rgba,
    pub outline_width: f64,
    /// 0.0–1.0.
    pub opacity: f64,
    pub shadow: Shadow,
    #[serde(skip)]
    cache: MeasureCache,
}

impl Text {
    pub const TYPE: &'static str = "text";

    /// A new, empty text at `at` (its top-left) with the tool defaults (redline: Adwaita Sans
    /// 34 Bold, Coral, outline Paper 6 px, left aligned, shadow off).
    pub fn new(at: Point) -> Self {
        Self {
            text: String::new(),
            x: at.x,
            y: at.y,
            rotation: 0.0,
            font_family: DEFAULT_FAMILY.to_string(),
            font_size: 34.0,
            bold: true,
            italic: false,
            underline: false,
            strikethrough: false,
            alignment: Alignment::Left,
            color: palette::CORAL,
            outline_color: palette::PAPER,
            outline_width: 6.0,
            opacity: 1.0,
            shadow: Shadow::DEFAULT_OFF,
            cache: MeasureCache::default(),
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
            italic: self.italic,
            underline: self.underline,
            strikethrough: self.strikethrough,
            alignment: self.alignment,
            wrap: None,
        }
    }

    fn measure(&self) -> Measure {
        self.cache.get(&self.text, &self.style())
    }

    pub fn rect(&self) -> Rect {
        let m = self.measure();
        Rect::new(self.x, self.y, m.width, m.height)
    }

    fn body_bounds(&self) -> Rect {
        let m = self.measure();
        let ink = m.ink.translate(self.x, self.y);
        self.rect()
            .union(&ink)
            .inflate(self.outline_width.max(0.0) / 2.0 + 2.0)
            .rotated_bounds_around(self.rect().centre(), self.rotation)
    }

    fn draw(&self, cr: &cairo::Context) -> Result<(), cairo::Error> {
        let Some(layout) = layout(&self.text, &self.style()) else {
            return Ok(());
        };
        let r = self.rect();
        let c = r.centre();
        cr.translate(c.x, c.y);
        cr.rotate(self.rotation.to_radians());
        cr.translate(-r.width / 2.0, -r.height / 2.0);
        draw_layout(
            cr,
            &layout,
            self.color,
            Some((self.outline_color, self.outline_width)),
        )
    }

    /// Places the text so its box's centre is at `centre`.
    fn centre_at(&mut self, centre: Point) {
        let m = self.measure();
        self.x = centre.x - m.width / 2.0;
        self.y = centre.y - m.height / 2.0;
    }
}

impl EditableText for Text {
    fn text(&self) -> &str {
        &self.text
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
    }

    fn text_block(&self) -> TextBlock {
        TextBlock {
            text: self.text.clone(),
            style: self.style(),
            origin: Point::new(self.x, self.y),
            frame: self.arrange(),
        }
    }
}

impl Annotation for Text {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        with_shadow(self.body_bounds(), &self.shadow)
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        let local = self.arrange().to_local(point);
        self.rect()
            .inflate(tolerance.max(0.0) + self.outline_width.max(0.0) / 2.0)
            .contains(local)
    }

    fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
        if self.text.is_empty() {
            return Ok(());
        }
        styled(cr, self.opacity, &self.shadow, self.body_bounds(), |cr| {
            self.draw(cr)
        })
    }

    fn arrange(&self) -> Arrange {
        Arrange::new(self.rect(), self.rotation)
    }

    /// Text cannot stretch: a new size scales the font by the height ratio, centred in the new
    /// box (a pure move keeps the size exactly).
    fn set_arrange(&mut self, arrange: Arrange) {
        let old = self.rect();
        self.rotation = arrange.rotation;
        if (arrange.rect.height - old.height).abs() < 1e-9
            && (arrange.rect.width - old.width).abs() < 1e-9
        {
            self.x = arrange.rect.x;
            self.y = arrange.rect.y;
            return;
        }
        if old.height > 0.0 && arrange.rect.height > 0.0 {
            self.font_size = (self.font_size * arrange.rect.height / old.height)
                .clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        }
        self.centre_at(arrange.rect.centre());
    }

    fn translate(&mut self, dx: f64, dy: f64) {
        self.x += dx;
        self.y += dy;
    }

    /// Keeps reading the right way round: the centre follows the document, the angle as for
    /// any box; the font scales only with "Scale annotations".
    fn transform(&mut self, t: &Transform) {
        if t.is_translation() {
            self.translate(t.dx, t.dy);
            return;
        }
        let centre = t.apply(self.rect().centre());
        self.rotation = t.map_rotation(self.rotation);
        self.font_size = style_scaled(self.font_size, t).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.outline_width = style_scaled(self.outline_width, t).min(MAX_OUTLINE);
        shadow_scaled(&mut self.shadow, t);
        self.centre_at(centre);
    }

    fn properties(&self) -> Vec<Property> {
        vec![
            Property::new("text", PropertyValue::Text(self.text.clone())),
            Property::new("font_family", PropertyValue::Text(self.font_family.clone())),
            Property::new("font_size", PropertyValue::Number(self.font_size)),
            Property::new("bold", PropertyValue::Bool(self.bold)),
            Property::new("italic", PropertyValue::Bool(self.italic)),
            Property::new("underline", PropertyValue::Bool(self.underline)),
            Property::new("strikethrough", PropertyValue::Bool(self.strikethrough)),
            Property::new(
                "alignment",
                PropertyValue::Choice(self.alignment.key().into()),
            ),
            Property::new("color", PropertyValue::Color(self.color)),
            Property::new("outline_color", PropertyValue::Color(self.outline_color)),
            Property::new("outline_width", PropertyValue::Number(self.outline_width)),
            Property::new("opacity", PropertyValue::Number(self.opacity)),
            Property::new("shadow", PropertyValue::Shadow(self.shadow)),
        ]
    }

    fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
        match key {
            "text" => self.text = common::text(key, v)?,
            "font_family" => {
                let family = common::text(key, v)?;
                if family.trim().is_empty() {
                    return Err(PropertyError::Range(key.to_string()));
                }
                self.font_family = family;
            }
            "font_size" => self.font_size = value::number(key, v, MIN_FONT_SIZE..=MAX_FONT_SIZE)?,
            "bold" => self.bold = value::boolean(key, v)?,
            "italic" => self.italic = value::boolean(key, v)?,
            "underline" => self.underline = value::boolean(key, v)?,
            "strikethrough" => self.strikethrough = value::boolean(key, v)?,
            "alignment" => self.alignment = common::choice(key, v, Alignment::from_key)?,
            "color" => self.color = value::color(key, v)?,
            "outline_color" => self.outline_color = value::color(key, v)?,
            "outline_width" => self.outline_width = value::number(key, v, 0.0..=MAX_OUTLINE)?,
            "opacity" => self.opacity = value::number(key, v, 0.0..=1.0)?,
            "shadow" => self.shadow = value::shadow(key, v)?,
            _ => return Err(PropertyError::Unknown(key.to_string())),
        }
        Ok(())
    }

    /// Box handles; every resize keeps the ratio (text cannot stretch).
    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        drag_box_handle(self, handle, to, modifiers, true);
    }
}

trait RotatedAround {
    fn rotated_bounds_around(&self, centre: Point, degrees: f64) -> Rect;
}

impl RotatedAround for Rect {
    /// Bounding box of this rectangle turned by `degrees` around `centre` (not its own centre).
    fn rotated_bounds_around(&self, centre: Point, degrees: f64) -> Rect {
        if degrees == 0.0 || self.is_none() {
            return *self;
        }
        common::points_bounds(self.corners().map(|p| p.rotated_around(centre, degrees)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::{Compass, Target};
    use crate::document::bitmap::Bitmap;
    use crate::document::Document;

    fn sample() -> Text {
        let mut t = Text::new(Point::new(20.0, 10.0));
        t.text = "Runs nightly\non mains power".into();
        t
    }

    #[test]
    fn defaults_follow_the_redline() {
        let t = Text::new(Point::default());
        assert_eq!(
            (t.font_family.as_str(), t.font_size, t.bold),
            ("Adwaita Sans", 34.0, true)
        );
        assert_eq!(
            (t.color, t.outline_color, t.outline_width),
            (palette::CORAL, palette::PAPER, 6.0)
        );
        assert_eq!(t.alignment, Alignment::Left);
        assert!(!t.shadow.enabled);
    }

    #[test]
    fn multi_line_text_grows_with_its_lines() {
        let one = {
            let mut t = sample();
            t.text = "Runs nightly".into();
            t
        };
        let two = sample();
        assert!(two.rect().height > 1.6 * one.rect().height);
        assert!(one.rect().width > 50.0);
        // Bold is wider than regular.
        let mut regular = one.clone();
        regular.bold = false;
        assert!(regular.rect().width < one.rect().width);
    }

    #[test]
    fn text_renders_in_its_colour_with_an_outline() {
        let mut t = sample();
        t.text = "MMMM".into();
        t.font_size = 60.0;
        let doc = Document::new(Bitmap::filled(400, 120, Rgba::TRANSPARENT).unwrap());
        let ctx = RenderContext {
            document: &doc,
            zoom: 1.0,
            target: Target::Export,
        };
        let out = Bitmap::draw(400, 120, |cr| {
            palette::INK.set_source(cr);
            cr.paint()?;
            t.render(cr, &ctx)
        })
        .unwrap();
        let r = t.rect();
        let (coral, paper) = (0..out.height()).fold((0, 0), |acc, y| {
            (0..out.width()).fold(acc, |(c, p), x| match out.pixel(x, y) {
                Some(px) if px == palette::CORAL => (c + 1, p),
                Some(px) if px == palette::PAPER => (c, p + 1),
                _ => (c, p),
            })
        });
        assert!(coral > 500, "glyphs: {coral}");
        assert!(paper > 100, "outline: {paper}");
        assert!(r.width > 100.0);
    }

    #[test]
    fn resizing_scales_the_font() {
        let mut t = sample();
        let before = t.rect();
        t.drag_handle(
            HandleId::Resize(Compass::SE),
            Point::new(
                before.right() + before.width,
                before.bottom() + before.height,
            ),
            Modifiers::default(),
        );
        assert!((t.font_size - 68.0).abs() < 0.5, "{}", t.font_size);
        // A move keeps everything but the position.
        let mut moved = sample();
        moved.translate(5.0, 7.0);
        assert_eq!((moved.x, moved.y, moved.font_size), (25.0, 17.0, 34.0));
    }

    #[test]
    fn document_transforms_keep_the_text_readable() {
        let mut t = sample();
        let centre = t.rect().centre();
        t.transform(&Transform {
            mirror: Some(crate::document::geometry::Mirror::Horizontal),
            dx: 500.0,
            ..Transform::IDENTITY
        });
        assert_eq!(t.rotation, 0.0);
        let c = t.rect().centre();
        assert!((c.x - (500.0 - centre.x)).abs() < 1e-6 && (c.y - centre.y).abs() < 1e-6);
        assert_eq!(t.font_size, 34.0, "scale annotations off");
    }

    #[test]
    fn editing_geometry() {
        let t = sample();
        let block = t.text_block();
        let (top, bottom) = block.caret(0);
        assert!((top.x - t.x).abs() < 1.0 && bottom.y > top.y);
        // A click on the second line puts the caret there.
        let second_line = Point::new(t.x + 2.0, t.y + t.rect().height * 0.75);
        let i = block.index_at(second_line);
        assert_eq!(i, "Runs nightly\n".len());
        // Down from the start of the first line: the second line.
        assert_eq!(block.vertical(0, 1), "Runs nightly\n".len());
        assert_eq!(block.vertical(3, -1), 0);
        assert_eq!(block.vertical(3, 5), t.text.len());
        assert_eq!(block.line_range(15), (13, t.text.len()));
        let sel = block.selection(5, 17);
        assert_eq!(sel.len(), 2, "two lines selected");
    }

    #[test]
    fn properties_round_trip_and_validate() {
        let mut t = sample();
        let before = t.clone();
        for p in t.clone().properties() {
            t.set_property(p.key, &p.value).unwrap();
        }
        assert_eq!(t, before);
        t.set_property("italic", &PropertyValue::Bool(true))
            .unwrap();
        assert!(t.italic);
        assert!(t
            .set_property("alignment", &PropertyValue::Choice("justify".into()))
            .is_err());
        assert!(t
            .set_property("font_family", &PropertyValue::Text(" ".into()))
            .is_err());
        assert!(t
            .set_property("font_size", &PropertyValue::Number(0.0))
            .is_err());
    }
}
