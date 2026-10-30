//! Golden-image tests of the shared renderer (CLAUDE.md §7: every object type has one).
//!
//! Each test renders a small synthetic document and compares it with
//! `tests/render/golden/<name>.png`. Run with `ANNOTA_UPDATE_GOLDEN=1` to (re)write the golden
//! files after an intended change, then look at them before committing.

use std::path::PathBuf;

use annota::document::bitmap::Bitmap;
use annota::document::color::palette;
use annota::document::command::{Command, History};
use annota::document::geometry::{IRect, Point, Rect};
use annota::document::{Background, BaseImage, Canvas, Document};
use annota::render::{self, Options, Viewport};
use annota::tools::image::Image;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/render/golden")
        .join(format!("{name}.png"))
}

/// Compares with the golden image. Antialiasing may differ by a few levels between pixman
/// versions (Ubuntu 24.04 vs. newer), so small differences on a few edge pixels are accepted.
fn assert_golden(name: &str, actual: &Bitmap) {
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
    for y in 0..golden.height() {
        for x in 0..golden.width() {
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
