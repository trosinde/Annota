//! PDF export (design.md: "PDF via cairo PdfSurface, vector annotations kept vector").
//!
//! The page is drawn by the shared renderer (`crate::render`) onto a cairo PDF surface, in two
//! layers:
//!
//! 1. **One bitmap** at the export resolution: the canvas background, the original image and
//!    every object that must not stay a vector — redactions, and everything below a bitmap
//!    object that overlaps it (see [`raster_positions`]). The bitmap covers exactly the
//!    canvas.
//! 2. **Vectors**: every other object, drawn by its own `Annotation::render` onto the PDF
//!    context in stacking order, so text stays text and shapes stay paths.
//!
//! Drawing the document straight onto the PDF would embed the whole original image (and any
//! text under a redaction) with the redaction box merely painted on top — anyone could lift
//! the redacted pixels out of the file. The split keeps everything a redaction covers inside
//! the bitmap, where only the redacted result exists. It also keeps image areas outside the
//! canvas (after a crop of the canvas) out of the file.

use crate::document::annotation::Annotation;
use crate::document::bitmap::Bitmap;
use crate::document::geometry::Rect;
use crate::document::Document;
use crate::render::{self, Layer, Options, Viewport};
use crate::tools::redaction::redactions_in;

use super::{check_scale, ExportError};

/// PDF points per output pixel at device scale 1: 96 pixels per inch, the CSS and GTK
/// reference resolution. A capture's own device scale (`Metadata::scale_factor`, 2 for a
/// 2× HiDPI capture kept at device pixels) divides this further in [`page_layout`], so the
/// same on-screen window prints at the same physical size whichever display it was captured
/// on, rather than a 2× capture printing twice as large as a 1× capture of the same window.
pub const POINTS_PER_PIXEL: f64 = 72.0 / 96.0;
/// Margin around the image on A4 and Letter pages, in points (half an inch).
pub const PAGE_MARGIN: f64 = 36.0;
/// A4 portrait in points (210 × 297 mm).
pub const A4: (f64, f64) = (595.275_590_551, 841.889_763_780);
/// US Letter portrait in points (8.5 × 11 in).
pub const LETTER: (f64, f64) = (612.0, 792.0);

/// Redline "Page size": Fit to image, A4, Letter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageSize {
    /// The page is the image.
    #[default]
    FitToImage,
    A4,
    Letter,
}

/// PDF settings (mockup 05).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdfOptions {
    /// "Keep annotations as vectors" (default on). Off: the page is the flattened bitmap, the
    /// same pixels as a PNG export.
    pub vectors: bool,
    pub page: PageSize,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            vectors: true,
            page: PageSize::FitToImage,
        }
    }
}

/// Where the image goes on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageLayout {
    /// Page width and height in points.
    pub page: (f64, f64),
    /// Top-left corner of the image on the page, in points.
    pub origin: (f64, f64),
    /// Points per output pixel.
    pub points_per_pixel: f64,
}

/// Places a `width` × `height` pixel image on a page, `doc_scale` device pixels per CSS pixel
/// (`Metadata::scale_factor`; 1.0 for a capture with no HiDPI scaling). Fit to image: the page
/// is the image at [`POINTS_PER_PIXEL`] `/ doc_scale`. A4/Letter: the page turns to the image's
/// orientation (landscape for a wide image), the image is centred within [`PAGE_MARGIN`] and
/// shrunk to fit, never enlarged.
pub fn page_layout(width: i32, height: i32, page: PageSize, doc_scale: f64) -> PageLayout {
    let (w, h) = (f64::from(width), f64::from(height));
    let ppp = POINTS_PER_PIXEL / doc_scale;
    let natural = (w * ppp, h * ppp);
    let paper = match page {
        PageSize::FitToImage => {
            return PageLayout {
                page: natural,
                origin: (0.0, 0.0),
                points_per_pixel: ppp,
            }
        }
        PageSize::A4 => A4,
        PageSize::Letter => LETTER,
    };
    let (short, long) = (paper.0.min(paper.1), paper.0.max(paper.1));
    let page = if w > h { (long, short) } else { (short, long) };
    let available = (page.0 - 2.0 * PAGE_MARGIN, page.1 - 2.0 * PAGE_MARGIN);
    let fit = (available.0 / natural.0)
        .min(available.1 / natural.1)
        .min(1.0);
    let k = ppp * fit;
    PageLayout {
        page,
        origin: ((page.0 - w * k) / 2.0, (page.1 - h * k) / 2.0),
        points_per_pixel: k,
    }
}

/// True if `object` is or contains (in a group, at any depth) a redaction. Groups expose their
/// members through `Annotation::children`; other types have none.
pub fn contains_redaction(object: &dyn Annotation) -> bool {
    !redactions_in(object).is_empty()
}

/// For each top-level object (by stacking position), whether it goes into the page's bitmap.
///
/// An object goes into the bitmap if it contains a redaction, or if it lies below (and its
/// bounds overlap) an object that goes into the bitmap: whatever a redaction covers must
/// exist only as the redacted pixels, and an object drawn as a vector on top of the bitmap
/// must not overlap anything that is above it in the document. Objects above all bitmap
/// objects they overlap stay vectors.
pub fn raster_positions(doc: &Document) -> Vec<bool> {
    let scene = doc.scene();
    let nodes = scene.nodes();
    let mut raster = vec![false; nodes.len()];
    let mut above: Vec<Rect> = Vec::new();
    for (position, node) in nodes.iter().enumerate().rev() {
        let bounds = scene
            .bounds_at(position)
            .unwrap_or_else(|| node.object.bounds());
        if contains_redaction(node.object.as_ref()) || above.iter().any(|r| r.intersects(&bounds)) {
            raster[position] = true;
            above.push(bounds);
        }
    }
    raster
}

/// Output size in pixels at `scale`, as [`render::flatten`] rounds it.
fn output_size(doc: &Document, scale: f64) -> (i32, i32) {
    let c = doc.canvas().rect;
    let width = (f64::from(c.width) * scale).round().max(1.0) as i32;
    let height = (f64::from(c.height) * scale).round().max(1.0) as i32;
    (width, height)
}

/// Draws the document onto `cr`, whose user space is output pixels (the canvas' top-left at
/// the origin, `scale` pixels per document pixel): the bitmap layer, then the vector objects.
pub fn draw_layered(cr: &cairo::Context, doc: &Document, scale: f64) -> Result<(), ExportError> {
    let (width, height) = output_size(doc, scale);
    let viewport = Viewport::export(doc, scale);
    let raster = raster_positions(doc);
    let in_bitmap = |p: usize| raster.get(p).copied().unwrap_or(false);
    let bitmap = Bitmap::draw(width, height, |bcr| {
        render::render_layer(
            bcr,
            doc,
            &viewport,
            &Options::EXPORT,
            &Layer {
                base: true,
                objects: &in_bitmap,
            },
        )
    })?;
    paint_bitmap(cr, &bitmap)?;
    let as_vector = |p: usize| !in_bitmap(p);
    render::render_layer(
        cr,
        doc,
        &viewport,
        &Options::EXPORT,
        &Layer {
            base: false,
            objects: &as_vector,
        },
    )?;
    Ok(())
}

fn paint_bitmap(cr: &cairo::Context, bitmap: &Bitmap) -> Result<(), cairo::Error> {
    cr.save()?;
    cr.set_source_surface(bitmap.surface(), 0.0, 0.0)?;
    cr.rectangle(
        0.0,
        0.0,
        f64::from(bitmap.width()),
        f64::from(bitmap.height()),
    );
    let filled = cr.fill();
    cr.restore()?;
    filled
}

/// The PDF of `doc`, one page, `scale` output pixels per document pixel.
pub fn encode_pdf(
    doc: &Document,
    scale: f64,
    options: &PdfOptions,
) -> Result<Vec<u8>, ExportError> {
    let scale = check_scale(scale)?;
    let (width, height) = output_size(doc, scale);
    let layout = page_layout(width, height, options.page, doc.metadata().scale_factor);
    let surface = cairo::PdfSurface::for_stream(layout.page.0, layout.page.1, Vec::<u8>::new())?;
    surface.set_metadata(cairo::PdfMetadata::Creator, "Annota")?;
    if let Some(title) = doc.metadata().title.as_deref().filter(|t| !t.is_empty()) {
        surface.set_metadata(cairo::PdfMetadata::Title, title)?;
    }
    let drawn = (|| {
        let cr = cairo::Context::new(&surface)?;
        cr.translate(layout.origin.0, layout.origin.1);
        cr.scale(layout.points_per_pixel, layout.points_per_pixel);
        cr.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
        cr.clip();
        if options.vectors {
            draw_layered(&cr, doc, scale)
        } else {
            let flat = render::flatten(doc, scale)?;
            paint_bitmap(&cr, &flat)?;
            Ok(())
        }
    })();
    let stream = surface.finish_output_stream();
    drawn?;
    surface.status()?;
    let stream = stream.map_err(|err| ExportError::Io(err.error))?;
    stream
        .downcast::<Vec<u8>>()
        .map(|bytes| *bytes)
        .map_err(|_| ExportError::Encoder {
            format: "PDF",
            reason: "cairo returned an unexpected output stream".into(),
        })
}
