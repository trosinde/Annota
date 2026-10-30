//! The one cairo renderer shared by the canvas and export (CLAUDE.md §2: pixel-identical
//! WYSIWYG).
//!
//! [`render`] is a pure function of (document, cairo context, [`Viewport`], [`Options`]); it
//! does not touch widgets. The canvas calls it from its draw function, export and thumbnails
//! through [`flatten`]. Objects outside the context's clip are skipped, using the scene's
//! spatial index for large documents — this is how the canvas limits redraw work, since GTK 4
//! has no partial invalidation of a drawing area.

pub mod blur;
pub mod overlay;
pub mod shadow;
pub mod view;

use std::collections::HashMap;

use crate::document::annotation::{Annotation, RenderContext, Target};
use crate::document::bitmap::{Bitmap, BitmapError};
use crate::document::color::Rgba;
use crate::document::geometry::{IRect, Point, Rect};
use crate::document::scene::ObjectId;
use crate::document::{Background, Document};

/// Maps document space to device space: `device = doc * zoom + origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub zoom: f64,
    pub origin: Point,
}

impl Viewport {
    pub fn new(zoom: f64, origin: Point) -> Self {
        Self { zoom, origin }
    }

    /// The export mapping: the canvas' top-left corner at device (0, 0), `scale` px per px.
    pub fn export(doc: &Document, scale: f64) -> Self {
        let c = doc.canvas().rect;
        Self::new(
            scale,
            Point::new(-f64::from(c.x) * scale, -f64::from(c.y) * scale),
        )
    }

    pub fn to_document(&self, device: Point) -> Point {
        Point::new(
            (device.x - self.origin.x) / self.zoom,
            (device.y - self.origin.y) / self.zoom,
        )
    }

    pub fn to_device(&self, doc: Point) -> Point {
        Point::new(
            doc.x * self.zoom + self.origin.x,
            doc.y * self.zoom + self.origin.y,
        )
    }
}

/// Screen-only appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub target: Target,
    /// Checkerboard colours for a transparent canvas on screen (kit `--canvas-checker-a/b`).
    pub checker: (Rgba, Rgba),
}

impl Options {
    pub const EXPORT: Options = Options {
        target: Target::Export,
        checker: (Rgba::TRANSPARENT, Rgba::TRANSPARENT),
    };

    pub const SCREEN_LIGHT: Options = Options {
        target: Target::Screen,
        checker: (
            Rgba::opaque(0xF6, 0xF5, 0xF4),
            Rgba::opaque(0xE8, 0xE7, 0xE5),
        ),
    };

    pub const SCREEN_DARK: Options = Options {
        target: Target::Screen,
        checker: (
            Rgba::opaque(0x2A, 0x2A, 0x2E),
            Rgba::opaque(0x23, 0x23, 0x27),
        ),
    };
}

/// Size of one checkerboard square in screen pixels (kit: 16 px tile, 8 px squares).
pub const CHECKER_SQUARE: f64 = 8.0;

/// Edited copies the canvas draws instead of objects of the document while a gesture is in
/// progress (drag, resize, rotate), before they are committed as one command.
#[derive(Clone, Copy, Debug)]
pub struct Preview<'a> {
    pub objects: &'a [(ObjectId, Box<dyn Annotation>)],
    /// Also draw the originals, faded, where they were (moves; mockups 02 and 06).
    pub ghost: bool,
    /// The canvas rectangle while a canvas edge is dragged (mockup 10).
    pub canvas: Option<IRect>,
    /// The image's corner radius while its handle is dragged (mockup 09).
    pub corner_radius: Option<f64>,
}

impl<'a> Preview<'a> {
    /// Edited copies of objects; the canvas as it is.
    pub fn objects(objects: &'a [(ObjectId, Box<dyn Annotation>)], ghost: bool) -> Self {
        Self {
            objects,
            ghost,
            canvas: None,
            corner_radius: None,
        }
    }
}

/// Opacity of the original under a moved object (mockups 02, 06: "Ghost shows old position").
pub const GHOST_OPACITY: f64 = 0.35;

/// Draws the document: canvas background, original image, then every object bottom to top,
/// all clipped to the canvas. Only objects touching the current clip are drawn.
pub fn render(
    cr: &cairo::Context,
    doc: &Document,
    viewport: &Viewport,
    options: &Options,
) -> Result<(), cairo::Error> {
    render_preview(cr, doc, viewport, options, None)
}

/// [`render`] with `preview` copies in place of their objects, each at its object's stacking
/// position. Screen only; export always draws the document itself.
pub fn render_preview(
    cr: &cairo::Context,
    doc: &Document,
    viewport: &Viewport,
    options: &Options,
    preview: Option<&Preview<'_>>,
) -> Result<(), cairo::Error> {
    cr.save()?;
    let result = render_inner(cr, doc, viewport, options, preview);
    cr.restore()?;
    result
}

fn draw_object(
    cr: &cairo::Context,
    object: &dyn Annotation,
    ctx: &RenderContext<'_>,
) -> Result<(), cairo::Error> {
    cr.save()?;
    let drawn = object.render(cr, ctx);
    cr.restore()?;
    drawn
}

fn render_inner(
    cr: &cairo::Context,
    doc: &Document,
    viewport: &Viewport,
    options: &Options,
    preview: Option<&Preview<'_>>,
) -> Result<(), cairo::Error> {
    if !(viewport.zoom.is_finite() && viewport.zoom > 0.0) {
        return Ok(());
    }
    let base = cr.matrix();
    cr.translate(viewport.origin.x, viewport.origin.y);
    cr.scale(viewport.zoom, viewport.zoom);
    let canvas = preview
        .and_then(|p| p.canvas)
        .unwrap_or(doc.canvas().rect)
        .to_rect();
    cr.rectangle(canvas.x, canvas.y, canvas.width, canvas.height);
    cr.clip();
    let (x0, y0, x1, y1) = cr.clip_extents()?;
    let visible = Rect::new(x0, y0, x1 - x0, y1 - y0);
    if visible.width <= 0.0 || visible.height <= 0.0 {
        return Ok(());
    }

    match doc.canvas().background {
        Background::Color { color } => {
            color.set_source(cr);
            cr.paint()?;
        }
        // An unknown background type (newer minor format) is drawn as transparent.
        Background::Transparent | Background::Unknown if options.target == Target::Screen => {
            let corner = viewport.to_device(Point::new(canvas.x, canvas.y));
            paint_checker(cr, base, corner, options.checker)?;
        }
        Background::Transparent | Background::Unknown => {}
    }

    let radius = preview
        .and_then(|p| p.corner_radius)
        .unwrap_or(doc.image().corner_radius);
    paint_image(cr, doc, viewport.zoom, radius)?;

    let ctx = RenderContext {
        document: doc,
        zoom: viewport.zoom,
        target: options.target,
    };
    let scene = doc.scene();
    let mut positions = scene.query(&visible);
    let replaced: HashMap<ObjectId, &dyn Annotation> = preview
        .map(|p| {
            p.objects
                .iter()
                .map(|(id, object)| (*id, object.as_ref()))
                .collect()
        })
        .unwrap_or_default();
    if !replaced.is_empty() {
        // A moved copy may be visible where its original is not.
        positions.extend(replaced.keys().filter_map(|id| scene.position(*id)));
        positions.sort_unstable();
        positions.dedup();
    }
    let ghost = preview.is_some_and(|p| p.ghost);
    for position in positions {
        let Some(node) = scene.nodes().get(position) else {
            continue;
        };
        let Some(copy) = replaced.get(&node.id) else {
            draw_object(cr, node.object.as_ref(), &ctx)?;
            continue;
        };
        let original_visible = scene
            .bounds_at(position)
            .is_some_and(|b| b.intersects(&visible));
        if ghost && original_visible {
            cr.push_group();
            let drawn = draw_object(cr, node.object.as_ref(), &ctx);
            cr.pop_group_to_source()?;
            drawn?;
            cr.paint_with_alpha(GHOST_OPACITY)?;
        }
        if copy.bounds().intersects(&visible) {
            draw_object(cr, *copy, &ctx)?;
        }
    }
    Ok(())
}

/// Filter for scaled bitmaps: crisp pixels when zoomed in (pixel inspection), smooth
/// otherwise. At 100 % both give the source pixels, so canvas and export match.
pub fn filter_for(zoom: f64) -> cairo::Filter {
    if zoom >= 2.0 {
        cairo::Filter::Nearest
    } else {
        cairo::Filter::Good
    }
}

fn paint_base_image(cr: &cairo::Context, doc: &Document, zoom: f64) -> Result<(), cairo::Error> {
    paint_image(cr, doc, zoom, doc.image().corner_radius)
}

fn paint_image(
    cr: &cairo::Context,
    doc: &Document,
    zoom: f64,
    corner_radius: f64,
) -> Result<(), cairo::Error> {
    let image = doc.image();
    let (w, h) = (
        f64::from(image.bitmap.width()),
        f64::from(image.bitmap.height()),
    );
    cr.save()?;
    if corner_radius > 0.0 {
        shadow::rounded_rect(cr, 0.0, 0.0, w, h, corner_radius);
        cr.clip();
    }
    cr.set_source_surface(image.bitmap.surface(), 0.0, 0.0)?;
    cr.source().set_filter(filter_for(zoom));
    cr.rectangle(0.0, 0.0, w, h);
    let filled = cr.fill();
    cr.restore()?;
    filled
}

/// Checkerboard in the caller's (screen) pixels, anchored at the canvas corner: it scrolls
/// with the canvas but does not scale with zoom. `base` is the caller's transformation.
fn paint_checker(
    cr: &cairo::Context,
    base: cairo::Matrix,
    anchor: Point,
    colors: (Rgba, Rgba),
) -> Result<(), cairo::Error> {
    let tile = (2.0 * CHECKER_SQUARE) as i32;
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, tile, tile)?;
    {
        let t = cairo::Context::new(&surface)?;
        colors.0.set_source(&t);
        t.paint()?;
        colors.1.set_source(&t);
        t.rectangle(CHECKER_SQUARE, 0.0, CHECKER_SQUARE, CHECKER_SQUARE);
        t.rectangle(0.0, CHECKER_SQUARE, CHECKER_SQUARE, CHECKER_SQUARE);
        t.fill()?;
    }
    let pattern = cairo::SurfacePattern::create(&surface);
    pattern.set_extend(cairo::Extend::Repeat);
    pattern.set_filter(cairo::Filter::Nearest);
    pattern.set_matrix(cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, -anchor.x, -anchor.y));
    cr.save()?;
    cr.set_matrix(base);
    cr.set_source(&pattern)?;
    let painted = cr.paint();
    cr.restore()?;
    painted
}

/// Renders the canvas into a new bitmap at `scale` pixels per document pixel: export at 1.0,
/// thumbnails below. Objects are burnt in; editing aids are not drawn.
pub fn flatten(doc: &Document, scale: f64) -> Result<Bitmap, BitmapError> {
    let c = doc.canvas().rect;
    let width = (f64::from(c.width) * scale).round().max(1.0) as i32;
    let height = (f64::from(c.height) * scale).round().max(1.0) as i32;
    let viewport = Viewport::export(doc, scale);
    Bitmap::draw(width, height, |cr| {
        render(cr, doc, &viewport, &Options::EXPORT)
    })
}

/// Burns the objects `ids` into the original image (Flatten, Flatten All): a new bitmap of the
/// image with its rounded corners and those objects drawn in their stacking order, as they are
/// exported (no editing aids). It covers the image and the parts of the objects inside the
/// canvas, so a callout on an extended canvas keeps its pixels; outside the image and the
/// objects it is transparent, so the canvas background still shows there. Returns the bitmap
/// and where it lies in document space (whole pixels; it may start left of or above the old
/// image).
pub fn burn_in(doc: &Document, ids: &[ObjectId]) -> Result<(Bitmap, IRect), BitmapError> {
    let scene = doc.scene();
    let image = &doc.image().bitmap;
    let canvas = doc.canvas().rect.to_rect();
    let mut positions: Vec<usize> = ids.iter().filter_map(|id| scene.position(*id)).collect();
    positions.sort_unstable();
    positions.dedup();
    let mut area = Rect::new(
        0.0,
        0.0,
        f64::from(image.width()),
        f64::from(image.height()),
    );
    for &p in &positions {
        if let Some(visible) = scene.bounds_at(p).and_then(|b| b.intersection(&canvas)) {
            area = area.union(&visible);
        }
    }
    let rect = IRect::covering(&area);
    let ctx = RenderContext {
        document: doc,
        zoom: 1.0,
        target: Target::Export,
    };
    let bitmap = Bitmap::draw(rect.width, rect.height, |cr| {
        cr.translate(-f64::from(rect.x), -f64::from(rect.y));
        cr.rectangle(canvas.x, canvas.y, canvas.width, canvas.height);
        cr.clip();
        paint_base_image(cr, doc, 1.0)?;
        for &p in &positions {
            if let Some(node) = scene.nodes().get(p) {
                draw_object(cr, node.object.as_ref(), &ctx)?;
            }
        }
        Ok(())
    })?;
    Ok((bitmap, rect))
}

/// A thumbnail whose longer side is at most `max_side` pixels (never enlarged).
pub fn thumbnail(doc: &Document, max_side: i32) -> Result<Bitmap, BitmapError> {
    let c = doc.canvas().rect;
    let longest = f64::from(c.width.max(c.height).max(1));
    let scale = (f64::from(max_side) / longest).min(1.0);
    flatten(doc, scale)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use serde::Serialize;

    use super::*;
    use crate::document::annotation::{Annotation, Property, PropertyError, PropertyValue};
    use crate::document::color::palette;
    use crate::document::command::{Command, History};
    use crate::document::geometry::{Arrange, IRect};
    use crate::document::scene::Node;
    use crate::document::{BaseImage, Canvas};
    use crate::tools::image::Image;

    fn base() -> Bitmap {
        // A 4-colour test card so any offset or scaling error shows.
        Bitmap::draw(64, 40, |cr| {
            for (i, c) in [palette::CORAL, palette::LIME, palette::AZURE, palette::INK]
                .iter()
                .enumerate()
            {
                c.set_source(cr);
                cr.rectangle((i % 2) as f64 * 32.0, (i / 2) as f64 * 20.0, 32.0, 20.0);
                cr.fill()?;
            }
            Ok(())
        })
        .unwrap()
    }

    fn with_image_object(doc: &mut Document) {
        let mut h = History::new();
        let pip = Bitmap::filled(30, 20, palette::AMBER).unwrap();
        let cmd = Image::insert_command(doc, pip, Point::new(40.0, 22.0));
        h.execute(doc, "Insert", cmd).unwrap();
        let id = doc.scene().ids()[0];
        let mut o = doc.scene().get(id).unwrap().object.clone();
        let mut a = o.arrange();
        a.rotation = 20.0;
        a.rect = Rect::new(30.0, 12.0, 24.0, 16.0);
        o.set_arrange(a);
        h.execute(
            doc,
            "Rotate",
            Command::Replace {
                objects: vec![(id, o)],
            },
        )
        .unwrap();
    }

    fn same(a: &Bitmap, b: &Bitmap) -> bool {
        a.width() == b.width()
            && a.height() == b.height()
            && (0..a.height()).all(|y| (0..a.width()).all(|x| a.pixel(x, y) == b.pixel(x, y)))
    }

    #[test]
    fn export_of_a_plain_capture_is_the_capture() {
        let doc = Document::new(base());
        let out = flatten(&doc, 1.0).unwrap();
        assert!(same(&out, &doc.image().bitmap));
    }

    #[test]
    fn canvas_at_100_percent_matches_export() {
        // CLAUDE.md §2: one renderer → pixel-identical WYSIWYG. Draw the document as the canvas
        // does (screen target, offset inside a larger widget) and compare to the export.
        let mut doc = Document::new(base());
        with_image_object(&mut doc);
        let mut h = History::new();
        h.execute(
            &mut doc,
            "Extend",
            Command::SetCanvas(Canvas {
                rect: IRect::new(-10, 0, 80, 40),
                background: Background::Color {
                    color: palette::TEAL,
                },
            }),
        )
        .unwrap();
        let export = flatten(&doc, 1.0).unwrap();
        let (ox, oy) = (37.0, 21.0);
        let widget = Bitmap::draw(200, 120, |cr| {
            cr.set_source_rgb(0.87, 0.87, 0.85); // canvas-bg around the canvas
            cr.paint()?;
            render(
                cr,
                &doc,
                &Viewport::new(1.0, Point::new(ox + 10.0, oy)),
                &Options::SCREEN_LIGHT,
            )
        })
        .unwrap();
        for y in 0..export.height() {
            for x in 0..export.width() {
                assert_eq!(
                    widget.pixel(x + ox as i32, y + oy as i32),
                    export.pixel(x, y),
                    "pixel {x},{y}"
                );
            }
        }
    }

    #[test]
    fn transparent_canvas_is_checkered_on_screen_only() {
        let mut doc = Document::new(base());
        let mut h = History::new();
        h.execute(
            &mut doc,
            "Extend",
            Command::SetCanvas(Canvas {
                rect: IRect::new(0, 0, 100, 40),
                background: Background::Transparent,
            }),
        )
        .unwrap();
        let export = flatten(&doc, 1.0).unwrap();
        assert_eq!(export.width(), 100);
        assert_eq!(
            export.pixel(80, 10).unwrap().a,
            0,
            "exported as transparent"
        );
        assert_eq!(export.pixel(10, 10), Some(palette::CORAL));
        let screen = Bitmap::draw(100, 40, |cr| {
            render(
                cr,
                &doc,
                &Viewport::export(&doc, 1.0),
                &Options::SCREEN_LIGHT,
            )
        })
        .unwrap();
        let (a, b) = Options::SCREEN_LIGHT.checker;
        assert_eq!(screen.pixel(64, 0), Some(a));
        assert_eq!(screen.pixel(72, 0), Some(b));
        assert_eq!(screen.pixel(72, 8), Some(a));
    }

    #[test]
    fn coloured_canvas_fills_the_extension() {
        // Spec "Extend for callout": the new area is filled with the canvas colour.
        let mut doc = Document::new(base());
        let mut h = History::new();
        let paper = Background::Color {
            color: palette::PAPER,
        };
        h.execute(
            &mut doc,
            "Extend",
            Command::SetCanvas(Canvas {
                rect: IRect::new(0, 0, 264, 40),
                background: paper,
            }),
        )
        .unwrap();
        let out = flatten(&doc, 1.0).unwrap();
        assert_eq!((out.width(), out.height()), (264, 40));
        assert_eq!(out.pixel(200, 20), Some(palette::PAPER));
        assert_eq!(out.pixel(40, 30), Some(palette::INK));
    }

    #[test]
    fn rounded_image_corners_show_the_background() {
        let mut doc = Document::new(base());
        let mut h = History::new();
        let image = BaseImage {
            bitmap: doc.image().bitmap.clone(),
            corner_radius: 10.0,
        };
        h.execute(&mut doc, "Corner Radius", Command::SetImage(image))
            .unwrap();
        let out = flatten(&doc, 1.0).unwrap();
        assert_eq!(out.pixel(0, 0).unwrap().a, 0);
        assert_eq!(out.pixel(10, 10), Some(palette::CORAL));
    }

    #[test]
    fn zoomed_render_scales_the_image() {
        let doc = Document::new(base());
        let out = Bitmap::draw(128, 80, |cr| {
            render(
                cr,
                &doc,
                &Viewport::new(2.0, Point::default()),
                &Options::EXPORT,
            )
        })
        .unwrap();
        // At 200 % pixels are doubled (nearest filter): the quadrant edge moves to x = 64.
        assert_eq!(out.pixel(63, 10), Some(palette::CORAL));
        assert_eq!(out.pixel(64, 10), Some(palette::LIME));
        assert_eq!(out.pixel(10, 41), Some(palette::AZURE));
    }

    #[test]
    fn thumbnail_fits_the_longer_side() {
        let doc = Document::new(Bitmap::filled(2000, 1000, palette::PAPER).unwrap());
        let t = thumbnail(&doc, 512).unwrap();
        assert_eq!((t.width(), t.height()), (512, 256));
    }

    /// Counts how often it is drawn.
    #[derive(Clone, Debug, Serialize)]
    struct Counted {
        rect: Rect,
        #[serde(skip)]
        renders: Rc<Cell<usize>>,
    }

    impl Annotation for Counted {
        fn type_name(&self) -> &'static str {
            "counted"
        }
        fn bounds(&self) -> Rect {
            self.rect
        }
        fn hit_test(&self, p: Point, _: f64) -> bool {
            self.rect.contains(p)
        }
        fn render(&self, _: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
            self.renders.set(self.renders.get() + 1);
            Ok(())
        }
        fn arrange(&self) -> Arrange {
            Arrange::new(self.rect, 0.0)
        }
        fn set_arrange(&mut self, a: Arrange) {
            self.rect = a.rect;
        }
        fn properties(&self) -> Vec<Property> {
            Vec::new()
        }
        fn set_property(&mut self, k: &str, _: &PropertyValue) -> Result<(), PropertyError> {
            Err(PropertyError::Unknown(k.into()))
        }
    }

    #[test]
    fn preview_copies_replace_objects_in_their_stacking_place() {
        // During a drag the canvas draws edited copies (and, for moves, faded originals).
        let mut doc = Document::new(Bitmap::filled(100, 40, palette::PAPER).unwrap());
        let ids = doc.scene().fresh_ids(2);
        let boxed = |x: f64| crate::document::scene::tests::TestBox::boxed(x, 0.0, 20.0, 20.0);
        Command::Insert {
            nodes: vec![
                (0, Node::new(ids[0], boxed(0.0))),
                (1, Node::new(ids[1], boxed(60.0))),
            ],
            resources: Vec::new(),
        }
        .apply(&mut doc)
        .unwrap();
        let moved = vec![(ids[0], boxed(30.0))];
        let draw = |ghost: bool| {
            Bitmap::draw(100, 40, |cr| {
                let preview = Preview::objects(&moved, ghost);
                // The view shows only x 25..100: the original at 0..20 is culled, the copy is
                // not.
                cr.rectangle(25.0, 0.0, 75.0, 40.0);
                cr.clip();
                render_preview(
                    cr,
                    &doc,
                    &Viewport::new(1.0, Point::default()),
                    &Options::EXPORT,
                    Some(&preview),
                )
            })
            .unwrap()
        };
        let out = draw(false);
        assert_eq!(out.pixel(40, 10), Some(Rgba::opaque(0, 0, 0)), "copy");
        assert_eq!(
            out.pixel(70, 10),
            Some(Rgba::opaque(0, 0, 0)),
            "other object"
        );
        assert_eq!(out.pixel(10, 10).map(|p| p.a), Some(0), "outside the clip");
        // Ghost: the original faded where it was (visible part only).
        let doc_view = Bitmap::draw(100, 40, |cr| {
            let preview = Preview::objects(&moved, true);
            render_preview(
                cr,
                &doc,
                &Viewport::new(1.0, Point::default()),
                &Options::EXPORT,
                Some(&preview),
            )
        })
        .unwrap();
        let ghost = doc_view.pixel(10, 10).unwrap();
        assert!(ghost.r > 120 && ghost.r < 200, "faded: {ghost:?}");
        assert_eq!(doc_view.pixel(40, 10), Some(Rgba::opaque(0, 0, 0)));
        let _ = draw(true);
        // The document itself is unchanged and exports as before.
        assert_eq!(
            flatten(&doc, 1.0).unwrap().pixel(40, 10),
            Some(palette::PAPER)
        );
    }

    #[test]
    fn only_visible_objects_are_drawn() {
        // 300 objects (spatial-index path) on a 3000 px wide canvas; the view shows 100 px.
        let mut doc = Document::new(Bitmap::filled(3000, 100, palette::PAPER).unwrap());
        let renders = Rc::new(Cell::new(0));
        let ids = doc.scene().fresh_ids(300);
        let nodes = ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let object = Counted {
                    rect: Rect::new(i as f64 * 10.0, 10.0, 8.0, 8.0),
                    renders: renders.clone(),
                };
                (i, Node::new(id, Box::new(object)))
            })
            .collect();
        Command::Insert {
            nodes,
            resources: Vec::new(),
        }
        .apply(&mut doc)
        .unwrap();
        Bitmap::draw(100, 100, |cr| {
            render(
                cr,
                &doc,
                &Viewport::new(1.0, Point::new(-1000.0, 0.0)),
                &Options::EXPORT,
            )
        })
        .unwrap();
        // x 1000..1100 touches objects 100..=110 (each 8 px wide at 10 px pitch).
        assert_eq!(renders.get(), 11);
        renders.set(0);
        flatten(&doc, 1.0).unwrap();
        assert_eq!(renders.get(), 300, "export draws everything");
    }
}
