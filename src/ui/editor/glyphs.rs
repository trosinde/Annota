//! Small drawings inside the sidebar's controls (redline "Kit additions needed": shape, line
//! style, alignment and end-style glyphs were improvised in the mockups and are not in the
//! icon kit or Adwaita): drawn with cairo in the widget's own foreground colour, so they follow
//! light/dark and the checked state like symbolic icons. End styles and line styles are drawn
//! by the real arrow renderer, so the tiles show exactly what the canvas draws.
//!
//! Also the colour swatches (any colour, the check mark of the current one, None's red slash,
//! the Custom swatch's hue wheel) and the shadow editor's angle dial.

use std::cell::RefCell;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use annota::document::annotation::{Annotation, RenderContext, Target};
use annota::document::bitmap::Bitmap;
use annota::document::color::Rgba;
use annota::document::geometry::Point;
use annota::document::style::{LineStyle, Shadow};
use annota::document::Document;
use annota::tools::arrow::{Arrow, EndStyle};

use super::colour::{self, Swatch};

/// What a glyph shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Polygon,
    Cloud,
    StepCircle,
    StepSquare,
    StepRoundedSquare,
    StepPin,
    Line(LineStyle),
    /// An end style; `start` draws it at the left end.
    End {
        style: EndStyle,
        start: bool,
    },
}

thread_local! {
    /// The arrow renderer reads nothing from the document; it still wants one.
    static NOTHING: RefCell<Option<Document>> = const { RefCell::new(None) };
}

fn with_document<T>(f: impl FnOnce(&Document) -> T) -> Option<T> {
    NOTHING.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Document::new(Bitmap::filled(1, 1, Rgba::TRANSPARENT).ok()?));
        }
        slot.as_ref().map(f)
    })
}

fn rgba(fg: (f64, f64, f64, f64)) -> Rgba {
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba::new(byte(fg.0), byte(fg.1), byte(fg.2), byte(fg.3))
}

fn rounded(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 1.5 * PI);
    cr.close_path();
}

fn arrow_glyph(
    cr: &cairo::Context,
    (w, h): (f64, f64),
    fg: (f64, f64, f64, f64),
    line: LineStyle,
    end: Option<(EndStyle, bool)>,
) -> Result<(), cairo::Error> {
    let y = h / 2.0;
    let mut a = Arrow::line(Point::new(3.0, y), Point::new(w - 3.0, y));
    a.width = 2.0;
    a.color = rgba(fg);
    a.shadow = Shadow {
        enabled: false,
        ..Shadow::DEFAULT
    };
    a.line_style = line;
    if let Some((style, start)) = end {
        if start {
            a.start_style = style;
        } else {
            a.end_style = style;
        }
        // Heads of a 2 px line are small; the tiles show them at a readable size.
        a.start_size = 2.0;
        a.end_size = 2.0;
    }
    with_document(|doc| {
        a.render(
            cr,
            &RenderContext {
                document: doc,
                zoom: 1.0,
                target: Target::Export,
            },
        )
    })
    .unwrap_or(Ok(()))
}

/// Draws `glyph` into a `width` × `height` area in the colour `fg` (r, g, b, a in 0–1).
pub fn draw(
    cr: &cairo::Context,
    glyph: Glyph,
    width: f64,
    height: f64,
    fg: (f64, f64, f64, f64),
) -> Result<(), cairo::Error> {
    cr.set_source_rgba(fg.0, fg.1, fg.2, fg.3);
    cr.set_line_width(1.5);
    cr.set_line_join(cairo::LineJoin::Round);
    let s = width.min(height);
    let (x, y) = ((width - s) / 2.0, (height - s) / 2.0);
    let inset = 1.75;
    let (bx, by, bs) = (x + inset, y + inset + s * 0.12, s - 2.0 * inset);
    let bh = bs * 0.76;
    match glyph {
        Glyph::Rectangle => {
            cr.rectangle(bx, by, bs, bh);
            cr.stroke()
        }
        Glyph::RoundedRectangle => {
            rounded(cr, bx, by, bs, bh, bh * 0.35);
            cr.stroke()
        }
        Glyph::Ellipse => {
            let m = cr.matrix();
            cr.translate(x + s / 2.0, y + s / 2.0);
            cr.scale(bs / 2.0, bh / 2.0);
            cr.arc(0.0, 0.0, 1.0, 0.0, TAU);
            cr.set_matrix(m);
            cr.stroke()
        }
        Glyph::Polygon => {
            let (cx, cy, r) = (x + s / 2.0, y + s / 2.0 + 0.5, s / 2.0 - inset);
            for k in 0..5 {
                let a = -FRAC_PI_2 + f64::from(k) * TAU / 5.0;
                let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
                if k == 0 {
                    cr.move_to(px, py);
                } else {
                    cr.line_to(px, py);
                }
            }
            cr.close_path();
            cr.stroke()
        }
        Glyph::Cloud => {
            let (cx, cy) = (x + s / 2.0, y + s * 0.55);
            cr.new_sub_path();
            cr.arc(cx - s * 0.2, cy, s * 0.2, 0.5 * PI, 1.5 * PI);
            cr.arc(cx, cy - s * 0.12, s * 0.24, PI * 1.05, PI * 1.95);
            cr.arc(cx + s * 0.22, cy + 0.02 * s, s * 0.18, 1.5 * PI, 0.5 * PI);
            cr.close_path();
            cr.stroke()
        }
        Glyph::StepCircle => {
            cr.arc(x + s / 2.0, y + s / 2.0, s / 2.0 - inset, 0.0, TAU);
            cr.stroke()?;
            cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size(s * 0.6);
            let extents = cr.text_extents("1")?;
            cr.move_to(
                x + s / 2.0 - extents.x_bearing() - extents.width() / 2.0,
                y + s / 2.0 - extents.y_bearing() - extents.height() / 2.0,
            );
            cr.show_text("1")
        }
        Glyph::StepSquare => {
            cr.rectangle(x + inset, y + inset, s - 2.0 * inset, s - 2.0 * inset);
            cr.stroke()
        }
        Glyph::StepRoundedSquare => {
            let side = s - 2.0 * inset;
            rounded(cr, x + inset, y + inset, side, side, side * 0.28);
            cr.stroke()
        }
        Glyph::StepPin => {
            let (cx, r) = (x + s / 2.0, s * 0.3);
            let cy = y + inset + r;
            cr.new_sub_path();
            cr.arc(cx, cy, r, PI * 0.8, PI * 0.2);
            cr.line_to(cx, y + s - inset);
            cr.close_path();
            cr.fill()
        }
        Glyph::Line(style) => arrow_glyph(cr, (width, height), fg, style, None),
        Glyph::End { style, start } => arrow_glyph(
            cr,
            (width, height),
            fg,
            LineStyle::Solid,
            Some((style, start)),
        ),
    }
}

/// A conic hue wheel in the circle of radius `r` around `c` (the Custom swatch).
fn hue_wheel(cr: &cairo::Context, c: (f64, f64), r: f64) -> Result<(), cairo::Error> {
    const SLICES: u32 = 36;
    for k in 0..SLICES {
        let a0 = f64::from(k) / f64::from(SLICES) * TAU - FRAC_PI_2;
        let a1 = f64::from(k + 1) / f64::from(SLICES) * TAU - FRAC_PI_2 + 0.02;
        let (red, green, blue) = colour::hsv_to_rgb(f64::from(k) * 10.0, 0.75, 0.95);
        cr.set_source_rgb(red, green, blue);
        cr.move_to(c.0, c.1);
        cr.arc(c.0, c.1, r, a0, a1);
        cr.close_path();
        cr.fill()?;
    }
    Ok(())
}

/// Draws a swatch (redline "Colour": a disc; the current one carries a check mark).
pub fn draw_swatch(
    cr: &cairo::Context,
    swatch: Swatch,
    size: f64,
    current: bool,
) -> Result<(), cairo::Error> {
    let (c, r) = ((size / 2.0, size / 2.0), size / 2.0);
    let light = match swatch {
        Swatch::Colour(colour) => {
            let (red, green, blue, alpha) = colour.to_f64();
            if alpha < 1.0 {
                // A checker under translucent colours.
                cr.save()?;
                cr.arc(c.0, c.1, r, 0.0, TAU);
                cr.clip();
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.paint()?;
                cr.set_source_rgb(0.8, 0.8, 0.8);
                let step = (size / 4.0).max(3.0);
                let mut yy = 0.0;
                let mut row = 0;
                while yy < size {
                    let mut xx = if row % 2 == 0 { 0.0 } else { step };
                    while xx < size {
                        cr.rectangle(xx, yy, step, step);
                        xx += 2.0 * step;
                    }
                    yy += step;
                    row += 1;
                }
                cr.fill()?;
                cr.restore()?;
            }
            cr.set_source_rgba(red, green, blue, alpha);
            cr.arc(c.0, c.1, r, 0.0, TAU);
            cr.fill()?;
            // Light colours get a hairline edge and a dark check mark.
            let luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
            let light = luma * alpha + (1.0 - alpha) > 0.75;
            if light {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.15);
                cr.set_line_width(1.0);
                cr.arc(c.0, c.1, r - 0.5, 0.0, TAU);
                cr.stroke()?;
            }
            light
        }
        Swatch::None => {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.arc(c.0, c.1, r, 0.0, TAU);
            cr.fill()?;
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.15);
            cr.set_line_width(1.0);
            cr.arc(c.0, c.1, r - 0.5, 0.0, TAU);
            cr.stroke()?;
            // The red slash.
            cr.save()?;
            cr.arc(c.0, c.1, r - 1.0, 0.0, TAU);
            cr.clip();
            cr.set_source_rgb(0.88, 0.11, 0.14);
            cr.set_line_width((size / 14.0).max(1.5));
            cr.move_to(size * 0.15, size * 0.85);
            cr.line_to(size * 0.85, size * 0.15);
            cr.stroke()?;
            cr.restore()?;
            true
        }
        Swatch::Custom => {
            hue_wheel(cr, c, r)?;
            false
        }
        Swatch::Eyedropper => return Ok(()),
    };
    if current {
        if light {
            cr.set_source_rgb(0.12, 0.12, 0.14);
        } else {
            cr.set_source_rgb(1.0, 1.0, 1.0);
        }
        cr.set_line_width((size / 14.0).max(1.75));
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_line_join(cairo::LineJoin::Round);
        cr.move_to(size * 0.31, size * 0.52);
        cr.line_to(size * 0.44, size * 0.65);
        cr.line_to(size * 0.69, size * 0.38);
        cr.stroke()?;
    }
    Ok(())
}

/// The shadow editor's angle dial (redline: 56 px): a disc, a hand pointing the way the shadow
/// falls (clockwise from the right, like the shadow's angle), a centre dot.
pub fn draw_dial(
    cr: &cairo::Context,
    size: f64,
    degrees: f64,
    accent: (f64, f64, f64, f64),
    track: (f64, f64, f64, f64),
) -> Result<(), cairo::Error> {
    let c = size / 2.0;
    cr.set_source_rgba(track.0, track.1, track.2, track.3);
    cr.arc(c, c, c, 0.0, TAU);
    cr.fill()?;
    cr.set_source_rgba(accent.0, accent.1, accent.2, accent.3);
    let a = degrees.to_radians();
    let hand = size * 22.0 / 56.0;
    cr.set_line_width(2.0);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.move_to(c, c);
    cr.line_to(c + hand * a.cos(), c + hand * a.sin());
    cr.stroke()?;
    cr.arc(c, c, size * 4.0 / 56.0, 0.0, TAU);
    cr.fill()
}

/// The angle (0–360°, clockwise from the right) of a pointer at `(x, y)` on a dial `size`
/// wide; `None` at the very centre.
pub fn dial_angle(size: f64, x: f64, y: f64) -> Option<f64> {
    let (dx, dy) = (x - size / 2.0, y - size / 2.0);
    if dx.hypot(dy) < 2.0 {
        return None;
    }
    Some(dy.atan2(dx).to_degrees().rem_euclid(360.0).round() % 360.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::color::palette;

    fn ink(bitmap: &Bitmap) -> usize {
        (0..bitmap.height())
            .flat_map(|y| (0..bitmap.width()).map(move |x| (x, y)))
            .filter(|&(x, y)| bitmap.pixel(x, y).is_some_and(|p| p.a > 100))
            .count()
    }

    #[test]
    fn every_glyph_draws_something() {
        let fg = (0.2, 0.2, 0.2, 1.0);
        let mut glyphs = vec![
            Glyph::Rectangle,
            Glyph::RoundedRectangle,
            Glyph::Ellipse,
            Glyph::Polygon,
            Glyph::Cloud,
            Glyph::StepCircle,
            Glyph::StepSquare,
            Glyph::StepRoundedSquare,
            Glyph::StepPin,
        ];
        glyphs.extend(LineStyle::ALL.map(Glyph::Line));
        for &style in EndStyle::ALL {
            glyphs.push(Glyph::End { style, start: true });
            glyphs.push(Glyph::End {
                style,
                start: false,
            });
        }
        for glyph in glyphs {
            let (w, h) = match glyph {
                Glyph::End { .. } | Glyph::Line(_) => (44, 16),
                _ => (16, 16),
            };
            let out =
                Bitmap::draw(w, h, |cr| draw(cr, glyph, f64::from(w), f64::from(h), fg)).unwrap();
            assert!(ink(&out) > 8, "{glyph:?}");
        }
        // The end tiles differ from each other (they show the real heads).
        let tile = |style| {
            Bitmap::draw(44, 16, |cr| {
                draw(
                    cr,
                    Glyph::End {
                        style,
                        start: false,
                    },
                    44.0,
                    16.0,
                    fg,
                )
            })
            .unwrap()
        };
        assert!(ink(&tile(EndStyle::Triangle)) > ink(&tile(EndStyle::None)));
        // The start tile puts the head on the left.
        let start = Bitmap::draw(44, 16, |cr| {
            draw(
                cr,
                Glyph::End {
                    style: EndStyle::Square,
                    start: true,
                },
                44.0,
                16.0,
                fg,
            )
        })
        .unwrap();
        // Ink away from the line (rows above and below it), left third against right third.
        let off_line = |x0: i32, x1: i32| {
            (0..16)
                .filter(|y| !(6..=9).contains(y))
                .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                .filter(|&(x, y)| start.pixel(x, y).is_some_and(|p| p.a > 100))
                .count()
        };
        let (left, right) = (off_line(0, 15), off_line(29, 44));
        assert!(left > right, "{left} {right}");
    }

    #[test]
    fn swatches_show_their_colour_and_the_current_one() {
        let draw_one = |swatch, current| {
            Bitmap::draw(32, 32, |cr| draw_swatch(cr, swatch, 32.0, current)).unwrap()
        };
        let coral = draw_one(Swatch::Colour(palette::CORAL), false);
        assert_eq!(coral.pixel(16, 6), Some(palette::CORAL));
        assert_eq!(coral.pixel(0, 0).map(|p| p.a), Some(0), "round");
        // The check mark: white on a dark colour, dark on Paper.
        let checked = draw_one(Swatch::Colour(palette::CORAL), true);
        assert_ne!(checked.pixel(14, 20), coral.pixel(14, 20));
        let paper = draw_one(Swatch::Colour(palette::PAPER), true);
        assert!(paper.pixel(14, 20).is_some_and(|p| p.r < 100));
        // None: a red slash.
        let none = draw_one(Swatch::None, false);
        assert!(none.pixel(16, 16).is_some_and(|p| p.r > 200 && p.g < 80));
        // Custom: a hue wheel.
        let custom = draw_one(Swatch::Custom, false);
        assert_ne!(custom.pixel(16, 4), custom.pixel(16, 28));
    }

    #[test]
    fn the_dial_points_where_the_shadow_falls() {
        assert_eq!(dial_angle(56.0, 56.0, 28.0), Some(0.0));
        assert_eq!(dial_angle(56.0, 28.0, 56.0), Some(90.0));
        assert_eq!(dial_angle(56.0, 0.0, 28.0), Some(180.0));
        assert_eq!(dial_angle(56.0, 28.0, 0.0), Some(270.0));
        assert_eq!(dial_angle(56.0, 28.0, 28.0), None);
        let accent = (0.21, 0.52, 0.89, 1.0);
        let out = Bitmap::draw(56, 56, |cr| {
            draw_dial(cr, 56.0, 45.0, accent, (0.0, 0.0, 0.0, 0.1))
        })
        .unwrap();
        // The hand runs down-right at 45°.
        assert!(out.pixel(40, 40).is_some_and(|p| p.b > 150));
        assert!(out.pixel(16, 40).is_some_and(|p| p.b < 150));
    }
}
