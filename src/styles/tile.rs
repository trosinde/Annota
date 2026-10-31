//! The picture of a quick style (redline: 63 × 48 tiles on paper, mockups 01–07): a sample
//! object of the tool in that style, drawn by the shared renderer — so a tile shows exactly
//! what the style draws (dashes, shadows, end styles, blur), not an approximation.
//!
//! Samples are laid out in a 104 × 78 box and scaled into the tile; an object
//! that would not fit (a 200 px font) is scaled down further. Callouts show two text bars
//! instead of words (mockups 04, 05), highlights and redactions lie over a small sample
//! "screenshot" so their effect shows.

use std::cell::RefCell;

use crate::document::annotation::{Annotation, Property, RenderContext, Target};
use crate::document::bitmap::Bitmap;
use crate::document::color::{palette, Rgba};
use crate::document::geometry::{Point, Rect};
use crate::document::Document;
use crate::editing::tool::Tool;
use crate::tools::arrow::Arrow;
use crate::tools::callout::Callout;
use crate::tools::highlighter::Highlighter;
use crate::tools::pen::Pen;
use crate::tools::redaction::Redaction;
use crate::tools::shape::Shape;
use crate::tools::step::Step;
use crate::tools::text::Text;

/// The sample box, document pixels.
pub const SAMPLE: (f64, f64) = (104.0, 78.0);

/// A small "screenshot" under highlights and redactions: lines of text-like bars and colour
/// blocks, so blur and pixelation show.
fn sample_page(cr: &cairo::Context) -> Result<(), cairo::Error> {
    palette::PAPER.set_source(cr);
    cr.paint()?;
    let bars: [(f64, f64, f64, Rgba); 6] = [
        (14.0, 22.0, 48.0, palette::INK),
        (66.0, 22.0, 24.0, palette::AZURE),
        (14.0, 34.0, 76.0, Rgba::opaque(0x5E, 0x5C, 0x64)),
        (14.0, 46.0, 34.0, palette::CORAL),
        (52.0, 46.0, 38.0, palette::INK),
        (14.0, 58.0, 58.0, Rgba::opaque(0x5E, 0x5C, 0x64)),
    ];
    for (x, y, w, c) in bars {
        c.set_source(cr);
        cr.rectangle(x, y, w, 6.0);
        cr.fill()?;
    }
    Ok(())
}

thread_local! {
    /// The document samples are drawn in (redactions read its image).
    static PAGE: RefCell<Option<Document>> = const { RefCell::new(None) };
}

fn with_page<R>(f: impl FnOnce(&Document) -> R) -> Option<R> {
    PAGE.with(|page| {
        let mut page = page.borrow_mut();
        if page.is_none() {
            let bitmap = Bitmap::draw(SAMPLE.0 as i32, SAMPLE.1 as i32, sample_page).ok()?;
            *page = Some(Document::new(bitmap));
        }
        page.as_ref().map(f)
    })
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// The sample object of `tool` in its default style.
pub fn sample(tool: Tool) -> Option<Box<dyn Annotation>> {
    Some(match tool {
        Tool::Select => return None,
        Tool::Arrow => Box::new(Arrow::new(p(14.0, 66.0), p(90.0, 14.0))),
        Tool::Line => Box::new(Arrow::line(p(14.0, 62.0), p(90.0, 16.0))),
        Tool::Pen => {
            let mut pen = Pen::new(p(16.0, 40.0));
            pen.points = (0..=40)
                .map(|i| {
                    let t = f64::from(i) / 40.0;
                    p(
                        16.0 + 72.0 * t,
                        40.0 - 15.0 * (t * std::f64::consts::TAU).sin(),
                    )
                })
                .collect();
            Box::new(pen)
        }
        Tool::Shape => Box::new(Shape::new(Rect::new(16.0, 14.0, 72.0, 50.0))),
        Tool::Text => {
            let mut text = Text::new(p(0.0, 0.0));
            text.text = "Aa".into();
            Box::new(text)
        }
        Tool::Callout => Box::new(Callout::new(
            Rect::new(12.0, 10.0, 80.0, 42.0),
            p(26.0, 70.0),
        )),
        Tool::Step => Box::new(Step::new(p(52.0, 39.0), 1, 1)),
        Tool::Highlighter => Box::new(Highlighter::new(Rect::new(10.0, 31.0, 84.0, 16.0))),
        Tool::Redact => Box::new(Redaction::new(Rect::new(10.0, 18.0, 84.0, 42.0))),
    })
}

/// The sample of `tool` with `values` applied (values the type refuses are skipped).
pub fn styled_sample(tool: Tool, values: &[Property]) -> Option<Box<dyn Annotation>> {
    let mut object = sample(tool)?;
    for prop in values {
        let _ = object.set_property(prop.key, &prop.value);
    }
    // A callout's body grows with its padding and font: its tail goes below the grown body.
    if let Some(callout) = object.as_any_mut().downcast_mut::<Callout>() {
        let body = callout.body();
        callout.tails = vec![Point::new(
            body.x + body.width * 0.22,
            body.y + body.height * 1.45,
        )];
    }
    // Text grows with its font: centre it again.
    if tool == Tool::Text {
        let r = object.arrange().rect;
        object.translate(
            SAMPLE.0 / 2.0 - (r.x + r.width / 2.0),
            SAMPLE.1 / 2.0 - (r.y + r.height / 2.0),
        );
    }
    Some(object)
}

/// Draws `tool`'s sample in the style `values` into a `width` × `height` tile at the origin of
/// `cr` (the tile's paper background is the widget's; highlights and redactions bring their
/// sample page).
pub fn draw(
    cr: &cairo::Context,
    tool: Tool,
    values: &[Property],
    width: f64,
    height: f64,
) -> Result<(), cairo::Error> {
    let Some(object) = styled_sample(tool, values) else {
        return Ok(());
    };
    let mut scale = (width / SAMPLE.0).min(height / SAMPLE.1);
    // Shrink what does not fit (a margin of 4 sample pixels). By the object's box, not its
    // bounds: a shadow may be cut at the tile's edge, the object itself not.
    let b = fit_box(object.as_ref());
    let (mut cx, mut cy) = (SAMPLE.0 / 2.0, SAMPLE.1 / 2.0);
    if b.width > SAMPLE.0 - 8.0 || b.height > SAMPLE.1 - 8.0 {
        let fit = ((SAMPLE.0 - 8.0) / b.width).min((SAMPLE.1 - 8.0) / b.height);
        if fit.is_finite() && fit > 0.0 {
            scale *= fit;
            cx = b.x + b.width / 2.0;
            cy = b.y + b.height / 2.0;
        }
    }
    cr.save()?;
    cr.rectangle(0.0, 0.0, width, height);
    cr.clip();
    cr.translate(width / 2.0, height / 2.0);
    cr.scale(scale, scale);
    cr.translate(-cx, -cy);
    let result = with_page(|page| -> Result<(), cairo::Error> {
        if matches!(tool, Tool::Highlighter | Tool::Redact) {
            cr.set_source_surface(page.image().bitmap.surface(), 0.0, 0.0)?;
            cr.paint()?;
        }
        let ctx = RenderContext {
            document: page,
            zoom: scale,
            target: Target::Export,
        };
        object.render(cr, &ctx)?;
        if tool == Tool::Callout {
            text_bars(cr, object.as_ref())?;
        }
        Ok(())
    })
    .unwrap_or(Ok(()));
    cr.restore()?;
    result
}

/// What has to fit into the tile: the object's box; a callout's body and tail.
fn fit_box(object: &dyn Annotation) -> Rect {
    match object.as_any().downcast_ref::<Callout>() {
        Some(callout) => callout.tails.iter().fold(callout.body(), |r, t| {
            r.union(&Rect::new(t.x, t.y, 0.0, 0.0))
        }),
        None => object.arrange().bounds(),
    }
}

/// Two bars in the callout's text colour where its text would be (mockups 04, 05).
fn text_bars(cr: &cairo::Context, object: &dyn Annotation) -> Result<(), cairo::Error> {
    let Some(callout) = object.as_any().downcast_ref::<Callout>() else {
        return Ok(());
    };
    let body = callout.body();
    let colour = callout.text_color;
    let alpha = f64::from(colour.a) / 255.0 * callout.opacity.clamp(0.0, 1.0);
    let (r, g, b, _) = colour.to_f64();
    cr.set_source_rgba(r, g, b, alpha);
    let h = 6.0;
    let x = body.x + 14.0;
    let y = body.y + body.height / 2.0 - h - 2.0;
    for (dy, w) in [(0.0, body.width * 0.62), (h + 4.0, body.width * 0.4)] {
        let y = y + dy;
        cr.new_sub_path();
        cr.arc(
            x + h / 2.0,
            y + h / 2.0,
            h / 2.0,
            std::f64::consts::FRAC_PI_2,
            3.0 * std::f64::consts::FRAC_PI_2,
        );
        cr.arc(
            x + w - h / 2.0,
            y + h / 2.0,
            h / 2.0,
            -std::f64::consts::FRAC_PI_2,
            std::f64::consts::FRAC_PI_2,
        );
        cr.close_path();
        cr.fill()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::PropertyValue;
    use crate::styles::generate;
    use crate::styles::theme::Theme;
    use crate::styles::value::TOOLS;

    fn render(tool: Tool, values: &[Property]) -> Bitmap {
        Bitmap::draw(63, 48, |cr| {
            palette::PAPER.set_source(cr);
            cr.paint()?;
            draw(cr, tool, values, 63.0, 48.0)
        })
        .unwrap()
    }

    fn has_colour(bitmap: &Bitmap, c: Rgba) -> bool {
        (0..bitmap.height()).any(|y| {
            (0..bitmap.width()).any(|x| {
                let p = bitmap.pixel(x, y).unwrap();
                (i32::from(p.r) - i32::from(c.r)).abs() < 8
                    && (i32::from(p.g) - i32::from(c.g)).abs() < 8
                    && (i32::from(p.b) - i32::from(c.b)).abs() < 8
            })
        })
    }

    #[test]
    fn every_tile_shows_its_style() {
        for tool in TOOLS {
            for (slot, values) in generate::styles(&Theme::annota(), tool).iter().enumerate() {
                let bitmap = render(tool, values);
                let painted =
                    (0..48).any(|y| (0..63).any(|x| bitmap.pixel(x, y).unwrap() != palette::PAPER));
                assert!(painted, "{tool:?} slot {slot} draws nothing");
            }
        }
        let teal = [Property::new("color", PropertyValue::Color(palette::TEAL))];
        assert!(has_colour(&render(Tool::Arrow, &teal), palette::TEAL));
        assert!(!has_colour(&render(Tool::Arrow, &[]), palette::TEAL));
    }

    #[test]
    fn oversized_styles_still_fit() {
        let big = [Property::new("font_size", PropertyValue::Number(300.0))];
        let object = styled_sample(Tool::Text, &big).unwrap();
        assert!(
            object.bounds().width > SAMPLE.0,
            "the sample itself is too big"
        );
        let bitmap = render(Tool::Text, &big);
        // Scaled down: the paper shows at the tile's edges.
        assert_eq!(bitmap.pixel(0, 0).unwrap(), palette::PAPER);
    }
}
