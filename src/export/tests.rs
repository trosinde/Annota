//! Export tests: round trips through real decoders, the WYSIWYG scenario, encoder settings,
//! PDF structure and the PDF's raster/vector split.

use super::pdf::{draw_layered, page_layout, raster_positions, A4, LETTER, PAGE_MARGIN};
use super::raster::straight_rgba;
use super::*;
use crate::document::annotation::Annotation;
use crate::document::bitmap::{png_size, Bitmap};
use crate::document::color::palette;
use crate::document::command::{Command, History};
use crate::document::geometry::{IRect, Point, Rect};
use crate::document::scene::tests::TestBox;
use crate::document::scene::Node;
use crate::document::{Background, Canvas};
use crate::render::{self, Options, Viewport};
use crate::tools::arrow::Arrow;
use crate::tools::group::Group;
use crate::tools::highlighter::Highlighter;
use crate::tools::redaction::{Redaction, RedactionKind};
use crate::tools::shape::Shape;
use crate::tools::step::Step;
use crate::tools::text::Text;

// ---- documents ----

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

fn set_canvas(doc: &mut Document, rect: IRect, background: Background) {
    History::new()
        .execute(
            doc,
            "Canvas",
            Command::SetCanvas(Canvas { rect, background }),
        )
        .unwrap();
}

fn text(at: Point, s: &str) -> Text {
    let mut t = Text::new(at);
    t.text = s.into();
    t.font_size = 14.0;
    t
}

/// A 160 × 100 capture with one of most object types, the canvas extended 20 px to the right
/// in Paper, so every exported pixel is opaque.
fn rich_document() -> Document {
    let mut doc = Document::with_title(test_card(160, 100), "Checkout flow");
    set_canvas(
        &mut doc,
        IRect::new(0, 0, 180, 100),
        Background::Color {
            color: palette::PAPER,
        },
    );
    let mut shape = Shape::new(Rect::new(20.0, 40.0, 60.0, 30.0));
    shape.fill = palette::AMBER;
    shape.fill_opacity = 0.5;
    let mut redaction = Redaction::new(Rect::new(110.0, 20.0, 40.0, 20.0));
    redaction.kind = RedactionKind::Pixelate;
    add(
        &mut doc,
        vec![
            Box::new(shape),
            Box::new(Highlighter::new(Rect::new(10.0, 75.0, 90.0, 12.0))),
            Box::new(Arrow::new(Point::new(30.0, 90.0), Point::new(150.0, 50.0))),
            Box::new(text(Point::new(8.0, 2.0), "Annota")),
            Box::new(Step::new(Point::new(165.0, 80.0), 1, 1)),
            Box::new(redaction),
        ],
    );
    doc
}

/// A capture whose canvas extends into a transparent area with a translucent shape on it.
fn translucent_document() -> Document {
    let mut doc = Document::new(test_card(60, 40));
    set_canvas(&mut doc, IRect::new(0, 0, 120, 40), Background::Transparent);
    let mut shape = Shape::new(Rect::new(50.0, 5.0, 35.0, 30.0));
    shape.fill = palette::AZURE;
    shape.fill_opacity = 0.4;
    add(&mut doc, vec![Box::new(shape)]);
    doc
}

// ---- pixels ----

fn assert_same_pixels(actual: &Bitmap, expected: &Bitmap) {
    assert_eq!(
        (actual.width(), actual.height()),
        (expected.width(), expected.height())
    );
    for y in 0..expected.height() {
        for x in 0..expected.width() {
            assert_eq!(actual.pixel(x, y), expected.pixel(x, y), "pixel {x},{y}");
        }
    }
}

/// Mean absolute difference per channel between two RGB(A) buffers of equal layout.
fn mean_error(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    let sum: u64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    sum as f64 / a.len() as f64
}

fn decode_jpeg(bytes: &[u8]) -> (u16, u16, Vec<u8>) {
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    let pixels = decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    assert_eq!(info.pixel_format, jpeg_decoder::PixelFormat::RGB24);
    (info.width, info.height, pixels)
}

/// Straight RGBA via libwebp's own decoder.
fn decode_webp(bytes: &[u8]) -> (i32, i32, Vec<u8>) {
    let (mut w, mut h) = (0, 0);
    // SAFETY: test-only FFI; libwebp reads `bytes` and returns a buffer of w × h × 4 bytes that
    // is copied and freed with `WebPFree`.
    unsafe {
        let out = libwebp_sys::WebPDecodeRGBA(bytes.as_ptr(), bytes.len(), &mut w, &mut h);
        assert!(!out.is_null(), "not a WebP image");
        let pixels = std::slice::from_raw_parts(out, (w * h * 4) as usize).to_vec();
        libwebp_sys::WebPFree(out.cast());
        (w, h, pixels)
    }
}

/// Straight RGBA, with the colour of fully transparent pixels zeroed (it is not visible and
/// encoders may change it).
fn visible(mut rgba: Vec<u8>) -> Vec<u8> {
    for px in rgba.chunks_exact_mut(4) {
        if px[3] == 0 {
            px.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    rgba
}

// ---- PNG / WYSIWYG ----

#[test]
fn png_export_matches_the_canvas_at_100_percent() {
    // Scenario "WYSIWYG": the canvas draws the document at 100 % inside a larger widget,
    // offset, with the screen options; the exported PNG must match it pixel by pixel.
    let doc = rich_document();
    let png = encode(&doc, &Format::Png, 1.0).unwrap();
    let exported = Bitmap::from_png(png).unwrap();
    let (ox, oy) = (23, 17);
    let canvas = Bitmap::draw(260, 160, |cr| {
        cr.set_source_rgb(0.87, 0.87, 0.85);
        cr.paint()?;
        render::render(
            cr,
            &doc,
            &Viewport::new(1.0, Point::new(f64::from(ox), f64::from(oy))),
            &Options::SCREEN_LIGHT,
        )
    })
    .unwrap();
    assert_eq!((exported.width(), exported.height()), (180, 100));
    for y in 0..exported.height() {
        for x in 0..exported.width() {
            assert_eq!(
                exported.pixel(x, y),
                canvas.pixel(x + ox, y + oy),
                "pixel {x},{y}"
            );
        }
    }
    // The objects are drawn: the export is not just the capture.
    let capture = &doc.image().bitmap;
    let changed = (0..capture.height())
        .flat_map(|y| (0..capture.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| exported.pixel(x, y) != capture.pixel(x, y))
        .count();
    assert!(changed > 1000, "{changed} pixels show objects");
}

#[test]
fn png_keeps_translucent_pixels() {
    let doc = translucent_document();
    let flat = render::flatten(&doc, 1.0).unwrap();
    let exported = Bitmap::from_png(encode(&doc, &Format::Png, 1.0).unwrap()).unwrap();
    assert_same_pixels(&exported, &flat);
    let p = exported.pixel(70, 20).unwrap();
    assert!(p.a > 0 && p.a < 255, "translucent: {p:?}");
    assert_eq!(exported.pixel(115, 2).unwrap().a, 0, "transparent canvas");
}

#[test]
fn scale_sets_the_output_size() {
    let doc = rich_document();
    for (scale, size) in [(0.5, (90, 50)), (2.0, (360, 200)), (0.25, (45, 25))] {
        let png = encode(&doc, &Format::Png, scale).unwrap();
        assert_eq!(png_size(&png), Some(size), "{scale}");
    }
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        for format in [
            Format::Png,
            Format::Jpeg { quality: 85 },
            Format::Pdf(PdfOptions::default()),
        ] {
            assert!(
                matches!(encode(&doc, &format, bad), Err(ExportError::Scale(_))),
                "{bad} {format:?}"
            );
        }
    }
}

// ---- JPG ----

#[test]
fn jpeg_round_trip_is_close_to_the_canvas() {
    let doc = rich_document();
    let flat = render::flatten(&doc, 1.0).unwrap();
    let source = super::raster::rgb_on_white(&flat).unwrap();
    let bytes = encode(&doc, &Format::Jpeg { quality: 95 }, 1.0).unwrap();
    assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF], "JPEG signature");
    let (w, h, pixels) = decode_jpeg(&bytes);
    assert_eq!((w, h), (180, 100));
    let error = mean_error(&pixels, &source);
    assert!(error < 2.5, "mean error {error}");
}

#[test]
fn jpeg_quality_is_passed_to_the_encoder() {
    let flat = render::flatten(&rich_document(), 1.0).unwrap();
    let source = super::raster::rgb_on_white(&flat).unwrap();
    let low = encode_jpeg(&flat, 10).unwrap();
    let high = encode_jpeg(&flat, 95).unwrap();
    assert!(low.len() < high.len(), "{} < {}", low.len(), high.len());
    let (low_error, high_error) = (
        mean_error(&decode_jpeg(&low).2, &source),
        mean_error(&decode_jpeg(&high).2, &source),
    );
    assert!(low_error > high_error, "{low_error} > {high_error}");
    for bad in [0, 101, 255] {
        assert!(matches!(encode_jpeg(&flat, bad), Err(ExportError::Quality(q)) if q == bad));
        assert!(matches!(
            encode(&rich_document(), &Format::Jpeg { quality: bad }, 1.0),
            Err(ExportError::Quality(_))
        ));
    }
    assert!(encode_jpeg(&flat, 1).is_ok() && encode_jpeg(&flat, 100).is_ok());
}

#[test]
fn jpeg_chroma_is_not_subsampled() {
    // 4:4:4 at the dialog's default quality: every component of the frame header samples 1×1.
    let bytes = encode_jpeg(&render::flatten(&rich_document(), 1.0).unwrap(), 85).unwrap();
    let sof = bytes
        .windows(2)
        .position(|m| m == [0xFF, 0xC0] || m == [0xFF, 0xC2])
        .expect("frame header");
    let components = bytes[sof + 9] as usize;
    assert_eq!(components, 3);
    for c in 0..components {
        assert_eq!(bytes[sof + 10 + c * 3 + 1], 0x11, "component {c}");
    }
}

#[test]
fn jpeg_puts_transparency_on_white() {
    let doc = translucent_document();
    let (w, _, pixels) = decode_jpeg(&encode(&doc, &Format::Jpeg { quality: 100 }, 1.0).unwrap());
    let at = |x: usize, y: usize| &pixels[(y * w as usize + x) * 3..][..3];
    assert!(at(115, 2).iter().all(|&c| c >= 250), "{:?}", at(115, 2));
    // The translucent blue shape over nothing: lighter than the shape's colour, bluish.
    let p = at(70, 20);
    assert!(p[2] > p[0], "{p:?}");
}

// ---- WebP ----

#[test]
fn webp_lossless_is_pixel_exact() {
    for doc in [rich_document(), translucent_document()] {
        let flat = render::flatten(&doc, 1.0).unwrap();
        let compression = WebPCompression::Lossless;
        let bytes = encode(&doc, &Format::WebP { compression }, 1.0).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
        assert!(bytes.windows(4).any(|c| c == b"VP8L"), "lossless bitstream");
        let (w, h, pixels) = decode_webp(&bytes);
        assert_eq!((w, h), (flat.width(), flat.height()));
        assert!(
            visible(pixels) == visible(straight_rgba(&flat).unwrap()),
            "lossless WebP differs from the canvas"
        );
    }
}

#[test]
fn webp_lossy_quality_is_passed_to_the_encoder() {
    let doc = translucent_document();
    let flat = render::flatten(&doc, 1.0).unwrap();
    let source = visible(straight_rgba(&flat).unwrap());
    let lossy = |quality| encode_webp(&flat, WebPCompression::Lossy { quality }).unwrap();
    let (low, high) = (lossy(5), lossy(95));
    assert!(low.len() < high.len(), "{} < {}", low.len(), high.len());
    assert!(high.windows(4).any(|c| c == b"VP8 "), "lossy bitstream");
    assert!(high.windows(4).any(|c| c == b"ALPH"), "alpha kept");
    let (w, h, decoded) = decode_webp(&high);
    assert_eq!((w, h), (120, 40));
    let decoded = visible(decoded);
    // Lossy WebP subsamples chroma (VP8 is 4:2:0), so hard coloured edges cost a few levels
    // even at 95; the quality shows in the error.
    let low_error = mean_error(&visible(decode_webp(&low).2), &source);
    let high_error = mean_error(&decoded, &source);
    assert!(high_error < low_error, "{high_error} < {low_error}");
    assert!(high_error < 8.0, "{high_error}");
    // Alpha is lossless in lossy WebP.
    assert!(decoded
        .chunks_exact(4)
        .zip(source.chunks_exact(4))
        .all(|(a, b)| a[3] == b[3]));
    for bad in [0, 101] {
        let compression = WebPCompression::Lossy { quality: bad };
        assert!(matches!(
            encode_webp(&flat, compression),
            Err(ExportError::Quality(_))
        ));
        assert!(matches!(
            encode(&doc, &Format::WebP { compression }, 1.0),
            Err(ExportError::Quality(_))
        ));
    }
}

#[test]
fn webp_refuses_images_above_its_size_limit() {
    let wide = Bitmap::filled(16_384, 2, palette::PAPER).unwrap();
    assert!(matches!(
        encode_webp(&wide, WebPCompression::Lossless),
        Err(ExportError::TooLarge {
            format: "WebP",
            max: 16_383,
            width: 16_384,
            height: 2
        })
    ));
    let ok = Bitmap::filled(16_383, 2, palette::PAPER).unwrap();
    assert!(encode_webp(&ok, WebPCompression::Lossy { quality: 50 }).is_ok());
}

// ---- PDF ----

/// The PDF as text with every Flate stream inflated: cairo writes PDF 1.7 with page, info and
/// font dictionaries inside compressed object streams.
fn pdf_text(bytes: &[u8]) -> String {
    use std::io::Read;
    let find = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).position(|w| w == needle);
    let mut out = String::from_utf8_lossy(bytes).into_owned();
    let mut rest = bytes;
    while let Some(start) = find(rest, b"stream") {
        let mut body = &rest[start + b"stream".len()..];
        body = body
            .strip_prefix(b"\r\n")
            .or_else(|| body.strip_prefix(b"\n"))
            .unwrap_or(body);
        let Some(end) = find(body, b"endstream") else {
            break;
        };
        let mut inflated = Vec::new();
        if flate2::read::ZlibDecoder::new(&body[..end])
            .read_to_end(&mut inflated)
            .is_ok()
        {
            out.push_str(&String::from_utf8_lossy(&inflated));
        }
        rest = &body[end + b"endstream".len()..];
    }
    out
}

/// Number of page objects (`/Type /Page`, not `/Type /Pages`).
fn page_count(pdf: &str) -> usize {
    count(pdf, "/Type /Page") - count(pdf, "/Type /Pages")
}

/// The page's `/MediaBox [ 0 0 w h ]`.
fn media_box(pdf: &str) -> (f64, f64) {
    let at = pdf.find("/MediaBox").expect("MediaBox");
    let open = at + pdf[at..].find('[').unwrap();
    let close = open + pdf[open..].find(']').unwrap();
    let n: Vec<f64> = pdf[open + 1..close]
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(n.len(), 4);
    (n[2] - n[0], n[3] - n[1])
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn pdf_is_one_page_of_the_image_size() {
    let doc = rich_document();
    let bytes = encode(&doc, &Format::Pdf(PdfOptions::default()), 1.0).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let pdf = pdf_text(&bytes);
    assert!(String::from_utf8_lossy(&bytes)
        .trim_end()
        .ends_with("%%EOF"));
    assert_eq!(page_count(&pdf), 1);
    // 180 × 100 px at 96 px per inch.
    assert!(
        close(media_box(&pdf), (135.0, 75.0)),
        "{:?}",
        media_box(&pdf)
    );
    assert!(pdf.contains("Checkout flow"), "title metadata");
    assert!(pdf.contains("Annota"), "creator metadata");
    let doubled = pdf_text(&encode(&doc, &Format::Pdf(PdfOptions::default()), 2.0).unwrap());
    assert!(close(media_box(&doubled), (270.0, 150.0)));
}

#[test]
fn pdf_paper_sizes_follow_the_image_orientation() {
    let wide = rich_document();
    let tall = Document::new(test_card(100, 200));
    let cases = [
        (&wide, PageSize::A4, (A4.1, A4.0)),
        (&tall, PageSize::A4, A4),
        (&wide, PageSize::Letter, (LETTER.1, LETTER.0)),
        (&tall, PageSize::Letter, LETTER),
    ];
    for (doc, page, expected) in cases {
        let options = PdfOptions {
            vectors: true,
            page,
        };
        let pdf = pdf_text(&encode(doc, &Format::Pdf(options), 1.0).unwrap());
        assert!(close(media_box(&pdf), expected), "{page:?}");
    }
}

#[test]
fn paper_layout_centres_and_only_shrinks() {
    // Small: natural size (0.75 pt per px), centred.
    let small = page_layout(400, 300, PageSize::A4);
    assert_eq!(small.points_per_pixel, 0.75);
    assert!(close(small.page, (A4.1, A4.0)));
    assert!(close(
        small.origin,
        ((A4.1 - 300.0) / 2.0, (A4.0 - 225.0) / 2.0)
    ));
    // Large: shrunk into the margins, the limiting side touching them.
    let large = page_layout(4000, 1000, PageSize::Letter);
    let width = 4000.0 * large.points_per_pixel;
    assert!((width - (LETTER.1 - 2.0 * PAGE_MARGIN)).abs() < 1e-6);
    assert!((large.origin.0 - PAGE_MARGIN).abs() < 1e-6);
    assert!(1000.0 * large.points_per_pixel < LETTER.0 - 2.0 * PAGE_MARGIN);
    // Fit to image: the page is the image.
    let fit = page_layout(400, 300, PageSize::FitToImage);
    assert_eq!((fit.page, fit.origin), ((300.0, 225.0), (0.0, 0.0)));
}

#[test]
fn pdf_keeps_text_as_vectors_unless_switched_off() {
    let mut doc = Document::new(test_card(160, 100));
    add(
        &mut doc,
        vec![Box::new(text(Point::new(10.0, 40.0), "Vector"))],
    );
    let vector = pdf_text(&encode(&doc, &Format::Pdf(PdfOptions::default()), 1.0).unwrap());
    assert!(vector.contains("/FontFile"), "embedded font");
    assert_eq!(
        count(&vector, "/Subtype /Image"),
        1,
        "one bitmap: the capture"
    );
    let options = PdfOptions {
        vectors: false,
        page: PageSize::FitToImage,
    };
    let flat = pdf_text(&encode(&doc, &Format::Pdf(options), 1.0).unwrap());
    assert!(!flat.contains("/FontFile"), "text is pixels");
    assert_eq!(count(&flat, "/Subtype /Image"), 1);
}

#[test]
fn pdf_keeps_redacted_content_out_of_the_file() {
    // Text under a solid redaction must not survive as a vector (it could be selected or
    // extracted under the box), and the original image is only there as the redacted result:
    // the page has one bitmap. Text above the redaction stays a vector.
    let mut doc = Document::new(test_card(160, 100));
    let mut solid = Redaction::new(Rect::new(0.0, 30.0, 160.0, 40.0));
    solid.kind = RedactionKind::Solid;
    add(
        &mut doc,
        vec![
            Box::new(text(Point::new(10.0, 40.0), "Secret")),
            Box::new(solid),
        ],
    );
    let pdf = pdf_text(&encode(&doc, &Format::Pdf(PdfOptions::default()), 1.0).unwrap());
    assert!(
        !pdf.contains("/FontFile"),
        "redacted text is not in the file"
    );
    assert_eq!(count(&pdf, "/Subtype /Image"), 1);
    add(
        &mut doc,
        vec![Box::new(text(Point::new(10.0, 75.0), "Note"))],
    );
    assert_eq!(raster_positions(&doc), vec![true, true, false]);
    let pdf = pdf_text(&encode(&doc, &Format::Pdf(PdfOptions::default()), 1.0).unwrap());
    assert!(pdf.contains("/FontFile"), "the note above stays text");
}

#[test]
fn raster_split_covers_everything_below_bitmap_objects() {
    let mut doc = Document::new(Bitmap::filled(300, 100, palette::PAPER).unwrap());
    let boxed = |x: f64| TestBox::boxed(x, 10.0, 20.0, 20.0);
    let redaction = |x: f64| -> Box<dyn Annotation> {
        Box::new(Redaction::new(Rect::new(x, 10.0, 20.0, 20.0)))
    };
    // `add` takes the first 7 fresh ids for the top-level objects; the members take 8 and 9.
    let ids = doc.scene().fresh_ids(9);
    let group = Group::new(vec![
        Node::new(ids[7], boxed(250.0)),
        Node::new(ids[8], redaction(270.0)),
    ]);
    add(
        &mut doc,
        vec![
            boxed(5.0),      // 0: below 1, overlaps it → bitmap (transitively)
            boxed(20.0),     // 1: below the redaction, overlaps it → bitmap
            boxed(150.0),    // 2: overlaps nothing in the bitmap → vector
            redaction(35.0), // 3: redaction → bitmap
            boxed(45.0),     // 4: above the redaction → vector
            Box::new(group), // 5: contains a redaction → bitmap
            boxed(200.0),    // 6: vector
        ],
    );
    assert_eq!(
        raster_positions(&doc),
        vec![true, true, false, true, false, true, false]
    );
}

#[test]
fn layered_pdf_drawing_matches_the_flattened_image() {
    // The PDF's bitmap + vector layers, drawn onto pixels, give exactly the export bitmap:
    // splitting the stack changes nothing visible.
    let doc = rich_document();
    let positions = raster_positions(&doc);
    assert!(
        positions.contains(&true) && positions.contains(&false),
        "{positions:?}"
    );
    for scale in [1.0, 0.5] {
        let flat = render::flatten(&doc, scale).unwrap();
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, flat.width(), flat.height())
                .unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            draw_layered(&cr, &doc, scale).unwrap();
        }
        surface.flush();
        let stride = surface.stride() as usize;
        let (w, h) = (surface.width(), surface.height());
        let data = surface.take_data().unwrap().to_vec();
        let layered = Bitmap::from_argb32(w, h, stride, data).unwrap();
        assert_same_pixels(&layered, &flat);
    }
}

// ---- files ----

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("annota-export-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn export_writes_the_file() {
    let dir = temp_dir("write");
    let doc = rich_document();
    let formats = [
        Format::Png,
        Format::Jpeg { quality: 85 },
        Format::WebP {
            compression: WebPCompression::Lossy { quality: 80 },
        },
        Format::Pdf(PdfOptions::default()),
    ];
    for format in formats {
        let path = dir.join(format!("Checkout flow.{}", format.extension()));
        export(&doc, &path, &format, 1.0).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            bytes,
            encode(&doc, &format, 1.0).unwrap(),
            "{}",
            format.name()
        );
    }
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names.len(), 4, "no temporary files left: {names:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn unwritable_folder_is_a_permission_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("ro");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let doc = rich_document();
    let exported = export(&doc, &dir.join("a.png"), &Format::Png, 1.0);
    let saved = save_annota(&doc, &dir.join("a.annota"));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    // Running as root ignores the permission; only check when writing really failed.
    for result in [exported, saved] {
        if let Err(err) = result {
            assert_eq!(
                err.io_kind(),
                Some(std::io::ErrorKind::PermissionDenied),
                "{err}"
            );
        }
    }
}

#[test]
fn save_annota_keeps_the_document_editable() {
    let dir = temp_dir("annota");
    let doc = rich_document();
    let path = dir.join("Checkout flow.annota");
    save_annota(&doc, &path).unwrap();
    let back = crate::document::archive::load(&path).unwrap();
    assert_eq!(back.object_count(), doc.object_count());
    assert_eq!(back.canvas(), doc.canvas());
    assert_eq!(back.metadata().title.as_deref(), Some("Checkout flow"));
    assert_same_pixels(
        &render::flatten(&back, 1.0).unwrap(),
        &render::flatten(&doc, 1.0).unwrap(),
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn formats_name_their_files() {
    let cases = [
        (Format::Png, "png", "PNG"),
        (Format::Jpeg { quality: 85 }, "jpg", "JPG"),
        (
            Format::WebP {
                compression: WebPCompression::Lossless,
            },
            "webp",
            "WebP",
        ),
        (Format::Pdf(PdfOptions::default()), "pdf", "PDF"),
    ];
    for (format, extension, name) in cases {
        assert_eq!((format.extension(), format.name()), (extension, name));
    }
    assert_eq!(
        PdfOptions::default(),
        PdfOptions {
            vectors: true,
            page: PageSize::FitToImage
        }
    );
    assert_eq!((DEFAULT_JPEG_QUALITY, DEFAULT_WEBP_QUALITY), (85, 80));
}
