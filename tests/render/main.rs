//! Golden-image tests of the shared renderer (CLAUDE.md §7: every object type has one).
//!
//! Each test renders a small synthetic document and compares it with
//! `tests/render/golden/<name>.png`. Run with `ANNOTA_UPDATE_GOLDEN=1` to (re)write the golden
//! files after an intended change, then look at them before committing.

use std::path::PathBuf;

use annota::document::annotation::HandleId;
use annota::document::annotation::{Annotation, PropertyValue as V};
use annota::document::bitmap::Bitmap;
use annota::document::color::palette;
use annota::document::command::{Command, History};
use annota::document::geometry::{IRect, Point, Rect};
use annota::document::scene::Node;
use annota::document::{Background, BaseImage, Canvas, Document};
use annota::render::{self, Options, Viewport};
use annota::tools::arrow::{Arrow, EndStyle};
use annota::tools::callout::{Callout, CalloutShape};
use annota::tools::group::group_command;
use annota::tools::highlighter::Highlighter;
use annota::tools::image::Image;
use annota::tools::pen::Pen;
use annota::tools::redaction::{Redaction, RedactionKind, RedactionShape};
use annota::tools::shape::{Shape, ShapeKind};
use annota::tools::step::{Counting, Step, StepShape};
use annota::tools::text::{Alignment, Text};

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/render/golden")
        .join(format!("{name}.png"))
}

/// Compares with the golden image. Antialiasing may differ by a few levels between pixman
/// versions (Ubuntu 24.04 vs. newer), so small differences on a few edge pixels are accepted.
fn assert_golden(name: &str, actual: &Bitmap) {
    assert_golden_masked(name, actual, &[]);
}

/// [`assert_golden`] ignoring the pixels inside `masks` (glyphs: fonts differ between systems;
/// the shapes around them are compared).
fn assert_golden_masked(name: &str, actual: &Bitmap, masks: &[Rect]) {
    let path = golden_path(name);
    if std::env::var_os("ANNOTA_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &*actual.png().unwrap()).unwrap();
        return;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "missing {}; run with ANNOTA_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    let golden = Bitmap::from_png(bytes).unwrap();
    assert_eq!(
        (actual.width(), actual.height()),
        (golden.width(), golden.height()),
        "{name}: size"
    );
    let mut off = 0usize;
    let mut worst = 0i32;
    let masked = |x: i32, y: i32| {
        masks
            .iter()
            .any(|m| m.contains(Point::new(f64::from(x) + 0.5, f64::from(y) + 0.5)))
    };
    for y in 0..golden.height() {
        for x in 0..golden.width() {
            if masked(x, y) {
                continue;
            }
            let (a, g) = (actual.pixel(x, y).unwrap(), golden.pixel(x, y).unwrap());
            let d = [
                i32::from(a.r) - i32::from(g.r),
                i32::from(a.g) - i32::from(g.g),
                i32::from(a.b) - i32::from(g.b),
                i32::from(a.a) - i32::from(g.a),
            ]
            .iter()
            .map(|v| v.abs())
            .max()
            .unwrap_or(0);
            worst = worst.max(d);
            if d > 3 {
                off += 1;
            }
        }
    }
    let total = (golden.width() * golden.height()) as usize;
    assert!(
        off * 1000 <= total * 5,
        "{name}: {off} of {total} pixels differ (worst by {worst}); \
         if intended, rerun with ANNOTA_UPDATE_GOLDEN=1"
    );
}

/// A synthetic "screenshot": light page, dark header, coloured blocks, a diagonal line.
fn test_card(width: i32, height: i32) -> Bitmap {
    Bitmap::draw(width, height, |cr| {
        let (w, h) = (f64::from(width), f64::from(height));
        cr.set_source_rgb(0.96, 0.95, 0.93);
        cr.paint()?;
        palette::INK.set_source(cr);
        cr.rectangle(0.0, 0.0, w, h * 0.15);
        cr.fill()?;
        for (i, c) in [palette::CORAL, palette::LIME, palette::AZURE]
            .iter()
            .enumerate()
        {
            c.set_source(cr);
            cr.rectangle(w * 0.08 + i as f64 * w * 0.3, h * 0.3, w * 0.24, h * 0.25);
            cr.fill()?;
        }
        palette::VIOLET.set_source(cr);
        cr.set_line_width(3.0);
        cr.move_to(w * 0.1, h * 0.9);
        cr.line_to(w * 0.9, h * 0.65);
        cr.stroke()
    })
    .unwrap()
}

#[test]
fn image_object() {
    // Mockup 12: an image inserted from the tray — Paper border, soft shadow; here also
    // rotated and on a canvas extended with a Paper background.
    let mut doc = Document::new(test_card(240, 150));
    let mut history = History::new();
    let insert = Image::insert_command(&doc, test_card(200, 125), Point::new(150.0, 90.0));
    history.execute(&mut doc, "Insert Image", insert).unwrap();
    let id = doc.scene().ids()[0];
    let mut object = doc.scene().get(id).unwrap().object.clone();
    let mut arrange = object.arrange();
    arrange.rect = Rect::new(120.0, 60.0, 100.0, 62.5);
    arrange.rotation = -8.0;
    object.set_arrange(arrange);
    history
        .execute(
            &mut doc,
            "Move",
            Command::Replace {
                objects: vec![(id, object)],
            },
        )
        .unwrap();
    history
        .execute(
            &mut doc,
            "Extend Canvas",
            Command::SetCanvas(Canvas {
                rect: IRect::new(0, 0, 300, 150),
                background: Background::Color {
                    color: palette::PAPER,
                },
            }),
        )
        .unwrap();
    assert_golden("image-object", &render::flatten(&doc, 1.0).unwrap());
}

#[test]
fn image_object_styles() {
    // Corner radius, coloured border, half opacity, no shadow.
    let mut doc = Document::new(test_card(160, 100));
    let mut history = History::new();
    let insert = Image::insert_command(&doc, test_card(160, 100), Point::new(80.0, 50.0));
    history.execute(&mut doc, "Insert Image", insert).unwrap();
    let id = doc.scene().ids()[0];
    let mut object = doc.scene().get(id).unwrap().object.clone();
    use annota::document::annotation::PropertyValue as V;
    object
        .set_property("corner_radius", &V::Number(8.0))
        .unwrap();
    object
        .set_property("border_color", &V::Color(palette::CORAL))
        .unwrap();
    object
        .set_property("border_width", &V::Number(4.0))
        .unwrap();
    object.set_property("opacity", &V::Number(0.5)).unwrap();
    let mut shadow = Image::DEFAULT_SHADOW;
    shadow.enabled = false;
    object.set_property("shadow", &V::Shadow(shadow)).unwrap();
    let mut arrange = object.arrange();
    arrange.rect = Rect::new(30.0, 20.0, 100.0, 62.5);
    object.set_arrange(arrange);
    history
        .execute(
            &mut doc,
            "Style",
            Command::Replace {
                objects: vec![(id, object)],
            },
        )
        .unwrap();
    assert_golden("image-object-styles", &render::flatten(&doc, 1.0).unwrap());
}

#[test]
fn transparent_canvas_on_screen() {
    // Mockup 10 on screen: transparent canvas extension as a checkerboard, rounded image
    // corners, 150 % zoom.
    let mut doc = Document::new(test_card(120, 80));
    let mut history = History::new();
    let image = BaseImage {
        bitmap: doc.image().bitmap.clone(),
        corner_radius: 12.0,
    };
    history
        .execute(&mut doc, "Corner Radius", Command::SetImage(image))
        .unwrap();
    history
        .execute(
            &mut doc,
            "Extend Canvas",
            Command::SetCanvas(Canvas {
                rect: IRect::new(-20, -10, 160, 100),
                background: Background::Transparent,
            }),
        )
        .unwrap();
    let zoom = 1.5;
    let out = Bitmap::draw(240, 150, |cr| {
        render::render(
            cr,
            &doc,
            &Viewport::export(&doc, zoom),
            &Options::SCREEN_LIGHT,
        )
    })
    .unwrap();
    assert_golden("transparent-canvas-screen", &out);
    // The same document exports with transparent pixels where the checkerboard is.
    let export = render::flatten(&doc, 1.0).unwrap();
    assert_eq!(export.pixel(2, 2).map(|p| p.a), Some(0));
    assert_eq!(export.pixel(21, 11).map(|p| p.a), Some(0), "rounded corner");
    assert_eq!(export.pixel(80, 50).map(|p| p.a), Some(255));
}

#[test]
fn group_object() {
    // Mockup 06: a group of two objects. Here two inserted images, grouped, then the group
    // turned by 20° as a whole and moved: the members turn around the group's centre.
    let mut doc = Document::new(test_card(240, 150));
    let mut history = History::new();
    for centre in [Point::new(70.0, 60.0), Point::new(150.0, 90.0)] {
        let insert = Image::insert_command(&doc, test_card(160, 100), centre);
        history.execute(&mut doc, "Insert Image", insert).unwrap();
    }
    let ids = doc.scene().ids();
    let (group, cmd) = group_command(&doc, &ids).unwrap();
    history.execute(&mut doc, "Group", cmd).unwrap();
    let mut object = doc.scene().get(group).unwrap().object.clone();
    let centre = object.arrange().rect.centre();
    let angle = 20f64.to_radians();
    // Drag the rotation handle to 20° clockwise of straight up.
    let to = Point::new(
        centre.x + 100.0 * angle.sin(),
        centre.y - 100.0 * angle.cos(),
    );
    object.drag_handle(HandleId::Rotate, to, Default::default());
    object.translate(10.0, -5.0);
    history
        .execute(
            &mut doc,
            "Rotate Group",
            Command::Replace {
                objects: vec![(group, object)],
            },
        )
        .unwrap();
    assert_golden("group-object", &render::flatten(&doc, 1.0).unwrap());
}

/// Text goldens compare a 4 × 4 averaged image with a wide tolerance: glyph antialiasing and
/// hinting differ between FreeType versions, but a wrong colour, outline, size, place or
/// missing text still shows. The text uses DejaVu Sans, which Ubuntu and Debian install with
/// fontconfig; without it, only the ink is checked.
fn assert_golden_coarse(name: &str, actual: &Bitmap) {
    const CELL: i32 = 4;
    let path = golden_path(name);
    if std::env::var_os("ANNOTA_UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &*actual.png().unwrap()).unwrap();
        return;
    }
    if !has_family("DejaVu Sans") {
        eprintln!("{name}: DejaVu Sans is not installed; only the ink is checked");
        return;
    }
    let golden = Bitmap::from_png(std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        (actual.width(), actual.height()),
        (golden.width(), golden.height())
    );
    let average = |b: &Bitmap, cx: i32, cy: i32| {
        let mut sum = [0u32; 4];
        for y in cy * CELL..(cy + 1) * CELL {
            for x in cx * CELL..(cx + 1) * CELL {
                let p = b.pixel(x, y).unwrap();
                for (s, v) in sum.iter_mut().zip([p.r, p.g, p.b, p.a]) {
                    *s += u32::from(v);
                }
            }
        }
        sum.map(|s| s / (CELL * CELL) as u32)
    };
    let (cw, ch) = (golden.width() / CELL, golden.height() / CELL);
    let mut off = 0;
    for cy in 0..ch {
        for cx in 0..cw {
            let (a, g) = (average(actual, cx, cy), average(&golden, cx, cy));
            if a.iter().zip(&g).any(|(a, g)| a.abs_diff(*g) > 24) {
                off += 1;
            }
        }
    }
    assert!(
        off * 100 <= (cw * ch) as usize * 2,
        "{name}: {off} of {} cells differ",
        cw * ch
    );
}

fn has_family(family: &str) -> bool {
    use pango::prelude::*;
    pangocairo::FontMap::new()
        .list_families()
        .iter()
        .any(|f| f.name().eq_ignore_ascii_case(family))
}

/// Adds `objects` to `doc` as one step.
fn add(doc: &mut Document, objects: Vec<Box<dyn Annotation>>) {
    let ids = doc.scene().fresh_ids(objects.len());
    let base = doc.scene().len();
    let nodes = objects
        .into_iter()
        .zip(ids)
        .enumerate()
        .map(|(k, (o, id))| (base + k, Node::new(id, o)))
        .collect();
    History::new()
        .execute(
            doc,
            "Add",
            Command::Insert {
                nodes,
                resources: Vec::new(),
            },
        )
        .unwrap();
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

#[test]
fn arrow_and_line_objects() {
    // Mockups 02–04: the default arrow, a curved one, double-ended dashed, the other end
    // styles, a plain dotted line; tool defaults otherwise (Coral, shadow on).
    let mut doc = Document::new(test_card(360, 220));
    let mut curved = Arrow::new(p(20.0, 200.0), p(170.0, 120.0));
    curved.middle = p(60.0, 120.0);
    let mut double = Arrow::new(p(200.0, 30.0), p(340.0, 30.0));
    double.width = 6.0;
    double.start_style = EndStyle::Triangle;
    double
        .set_property("line_style", &V::Choice("dashed".into()))
        .unwrap();
    let mut ends = Arrow::new(p(200.0, 80.0), p(340.0, 80.0));
    ends.start_style = EndStyle::Circle;
    ends.end_style = EndStyle::Open;
    ends.color = palette::AZURE;
    let mut bars = Arrow::new(p(200.0, 130.0), p(340.0, 130.0));
    bars.start_style = EndStyle::Bar;
    bars.end_style = EndStyle::Square;
    bars.end_size = 1.5;
    bars.color = palette::VIOLET;
    bars.shadow.enabled = false;
    let mut line = Arrow::line(p(200.0, 180.0), p(340.0, 190.0));
    line.color = palette::AMBER;
    line.width = 4.0;
    line.set_property("line_style", &V::Choice("dotted".into()))
        .unwrap();
    add(
        &mut doc,
        vec![
            Box::new(Arrow::new(p(20.0, 30.0), p(160.0, 90.0))),
            Box::new(curved),
            Box::new(double),
            Box::new(ends),
            Box::new(bars),
            Box::new(line),
        ],
    );
    assert_golden("arrow-objects", &render::flatten(&doc, 1.0).unwrap());
}

#[test]
fn pen_objects() {
    // Mockup 05: freehand strokes with the defaults (10 px, smoothing 60) and a wider, unsmoothed
    // one in Lime.
    let mut doc = Document::new(test_card(320, 200));
    let mut wave = Pen::new(p(20.0, 60.0));
    let mut zigzag = Pen::new(p(20.0, 150.0));
    for i in 1..=56 {
        let x = 20.0 + f64::from(i) * 5.0;
        wave.points
            .push(p(x, 60.0 + 25.0 * (f64::from(i) * 0.25).sin()));
        zigzag
            .points
            .push(p(x, if i % 8 < 4 { 130.0 } else { 170.0 }));
    }
    zigzag.color = palette::LIME;
    zigzag.width = 16.0;
    zigzag.smoothing = 0.0;
    add(&mut doc, vec![Box::new(wave), Box::new(zigzag)]);
    assert_golden("pen-objects", &render::flatten(&doc, 1.0).unwrap());
}

#[test]
fn shape_objects() {
    // Mockups 06, 07: the default rounded frame (fill None), an ellipse, a filled rectangle with
    // a translucent fill, a polygon, and a turned dashed rectangle.
    let mut doc = Document::new(test_card(360, 240));
    let frame = Shape::new(Rect::new(20.0, 20.0, 150.0, 70.0));
    let mut ellipse = Shape::new(Rect::new(200.0, 20.0, 140.0, 70.0));
    ellipse.kind = ShapeKind::Ellipse;
    ellipse.outline_color = palette::TEAL;
    let mut filled = Shape::new(Rect::new(20.0, 120.0, 150.0, 90.0));
    filled.kind = ShapeKind::Rectangle;
    filled.outline_color = palette::AZURE;
    filled.outline_width = 4.0;
    filled.fill = palette::AZURE;
    filled.fill_opacity = 0.18;
    filled.shadow.enabled = false;
    let mut polygon = Shape::polygon(
        &[
            p(200.0, 130.0),
            p(300.0, 115.0),
            p(340.0, 170.0),
            p(260.0, 225.0),
        ],
        &Shape::new(Rect::default()),
    )
    .unwrap();
    polygon.fill = palette::CORAL;
    polygon.fill_opacity = 0.14;
    let mut turned = Shape::new(Rect::new(110.0, 100.0, 120.0, 40.0));
    turned.kind = ShapeKind::Rectangle;
    turned.rotation = -12.0;
    turned.outline_color = palette::VIOLET;
    turned
        .set_property("line_style", &V::Choice("dashed".into()))
        .unwrap();
    add(
        &mut doc,
        vec![
            Box::new(frame),
            Box::new(ellipse),
            Box::new(filled),
            Box::new(polygon),
            Box::new(turned),
        ],
    );
    assert_golden("shape-objects", &render::flatten(&doc, 1.0).unwrap());
}

#[test]
fn text_objects() {
    // Mockup 08: the text defaults (34 px bold, Coral with a Paper outline) on two lines, and a
    // centred, italic, underlined Ink text without outline, turned.
    let mut doc = Document::new(test_card(360, 220));
    let mut text = Text::new(p(16.0, 20.0));
    text.font_family = "DejaVu Sans".into();
    text.text = "Runs nightly\non mains power".into();
    let mut note = Text::new(p(60.0, 140.0));
    note.font_family = "DejaVu Sans".into();
    note.text = "Centred\nnote".into();
    note.font_size = 24.0;
    note.alignment = Alignment::Center;
    note.italic = true;
    note.underline = true;
    note.color = palette::INK;
    note.outline_width = 0.0;
    note.rotation = -8.0;
    let ink_box = text.arrange().rect;
    add(&mut doc, vec![Box::new(text), Box::new(note)]);
    let out = render::flatten(&doc, 1.0).unwrap();
    // The glyphs are there, in Coral, with Paper around them.
    let coral = (0..out.height())
        .flat_map(|y| (0..out.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| out.pixel(x, y) == Some(palette::CORAL))
        .count();
    assert!(coral > 400, "{coral}");
    assert!(
        ink_box.width > 150.0 && ink_box.height > 70.0,
        "{ink_box:?}"
    );
    assert_golden_coarse("text-objects", &out);
}

#[test]
fn callout_objects() {
    // Mockups 09, 10: the default callout (rounded, Coral, white text) with two tails; an
    // outlined white rectangle; an ellipse; a cloud. Glyphs are masked.
    let mut doc = Document::new(test_card(420, 300));
    let mut rounded = Callout::new(Rect::new(20.0, 20.0, 200.0, 60.0), p(60.0, 150.0));
    rounded.tails.push(p(260.0, 140.0));
    rounded.text = "Both paths".into();
    let mut boxed = Callout::new(Rect::new(250.0, 20.0, 150.0, 60.0), p(380.0, 130.0));
    boxed.shape = CalloutShape::Rectangle;
    boxed.fill = palette::PAPER;
    boxed.outline_width = 4.0;
    boxed.text_color = palette::INK;
    boxed.text = "Check".into();
    let mut ellipse = Callout::new(Rect::new(30.0, 180.0, 160.0, 80.0), p(10.0, 290.0));
    ellipse.shape = CalloutShape::Ellipse;
    ellipse.fill = palette::TEAL;
    ellipse.text = "Free".into();
    let mut cloud = Callout::new(Rect::new(230.0, 180.0, 170.0, 80.0), p(330.0, 150.0));
    cloud.shape = CalloutShape::Cloud;
    cloud.fill = palette::AMBER;
    cloud.text_color = palette::INK;
    cloud.text = "Pay".into();
    let masks: Vec<Rect> = [&rounded, &boxed, &ellipse, &cloud]
        .iter()
        .map(|c| {
            use annota::tools::text::EditableText;
            c.text_block().bounds().rect.inflate(4.0)
        })
        .collect();
    add(
        &mut doc,
        vec![
            Box::new(rounded),
            Box::new(boxed),
            Box::new(ellipse),
            Box::new(cloud),
        ],
    );
    assert_golden_masked(
        "callout-objects",
        &render::flatten(&doc, 1.0).unwrap(),
        &masks,
    );
}

#[test]
fn step_objects() {
    // Mockups 11, 12: the default circle markers 1, 2; a separate sequence of rounded squares
    // A, B in Azure; a square; a pin with a typed value. Labels are masked.
    let mut doc = Document::new(test_card(360, 200));
    let mut markers: Vec<Step> = vec![
        Step::new(p(40.0, 50.0), 1, 1),
        Step::new(p(40.0, 140.0), 1, 2),
    ];
    for (k, at) in [p(130.0, 50.0), p(130.0, 140.0)].into_iter().enumerate() {
        let mut s = Step::new(at, 2, k as u32 + 1);
        s.shape = StepShape::RoundedSquare;
        s.color = palette::AZURE;
        s.counting = Counting::UpperLetters;
        markers.push(s);
    }
    let mut square = Step::new(p(220.0, 50.0), 3, 1);
    square.shape = StepShape::Square;
    square.color = palette::TEAL;
    square.counting = Counting::Roman;
    markers.push(square);
    let mut pin = Step::new(p(300.0, 60.0), 3, 2);
    pin.shape = StepShape::Pin;
    pin.color = palette::VIOLET;
    pin.value = Some("7".into());
    markers.push(pin);
    let masks: Vec<Rect> = markers
        .iter()
        .map(|s| {
            Rect::new(
                s.x - 0.8 * s.radius,
                s.y - 0.8 * s.radius,
                1.6 * s.radius,
                1.6 * s.radius,
            )
        })
        .collect();
    add(
        &mut doc,
        markers
            .into_iter()
            .map(|s| Box::new(s) as Box<dyn Annotation>)
            .collect(),
    );
    assert_golden_masked("step-objects", &render::flatten(&doc, 1.0).unwrap(), &masks);
}

#[test]
fn highlighter_objects() {
    // Mockup 14: the default highlight (45 % yellow, multiply) over dark and light parts of the
    // capture, and a green one; the dark header stays dark.
    let mut doc = Document::new(test_card(320, 200));
    let mut green = Highlighter::new(Rect::new(30.0, 120.0, 260.0, 40.0));
    green.color = palette::LIME;
    green.opacity = 0.4;
    add(
        &mut doc,
        vec![
            Box::new(Highlighter::new(Rect::new(20.0, 10.0, 200.0, 60.0))),
            Box::new(green),
        ],
    );
    let out = render::flatten(&doc, 1.0).unwrap();
    // Under the highlight the Ink header is still Ink-dark.
    let header = out.pixel(100, 15).unwrap();
    assert!(header.r < 40 && header.g < 40, "{header:?}");
    assert_golden("highlighter-objects", &out);
}

#[test]
fn redaction_objects() {
    // Mockup 15: blur, the default pixelate, a solid box and a blurred ellipse over fine detail.
    let detail = Bitmap::draw(320, 200, |cr| {
        cr.set_source_rgb(0.96, 0.95, 0.93);
        cr.paint()?;
        for row in 0..12 {
            for col in 0..40 {
                if (row * 7 + col * 3) % 5 < 3 {
                    palette::INK.set_source(cr);
                    cr.rectangle(
                        f64::from(col) * 8.0 + 2.0,
                        f64::from(row) * 16.0 + 6.0,
                        5.0,
                        7.0,
                    );
                    cr.fill()?;
                }
            }
        }
        Ok(())
    })
    .unwrap();
    let mut doc = Document::new(detail);
    let mut blur = Redaction::new(Rect::new(10.0, 10.0, 140.0, 60.0));
    blur.kind = RedactionKind::Blur;
    let pixelate = Redaction::new(Rect::new(170.0, 10.0, 140.0, 60.0));
    let mut solid = Redaction::new(Rect::new(10.0, 110.0, 140.0, 40.0));
    solid.kind = RedactionKind::Solid;
    let mut ellipse = Redaction::new(Rect::new(180.0, 100.0, 120.0, 90.0));
    ellipse.kind = RedactionKind::Blur;
    ellipse.shape = RedactionShape::Ellipse;
    add(
        &mut doc,
        vec![
            Box::new(blur),
            Box::new(pixelate),
            Box::new(solid),
            Box::new(ellipse),
        ],
    );
    assert_golden("redaction-objects", &render::flatten(&doc, 1.0).unwrap());
}
