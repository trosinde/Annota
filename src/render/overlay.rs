//! Editing aids drawn over the document on the canvas, never exported: the selection box and
//! its handles, snap guides and the rubber band (redline "Canvas" of add-annotation-editor).
//!
//! Everything is drawn in view pixels (the caller's cairo user space is the canvas view), so
//! lines stay 1 px and handles 10 px at every zoom level.
//!
//! * Selection: 1 px `--accent-bg` box; 8 square handles 10 × 10, white, 1.5 px accent
//!   border, radius 2; the rotation handle a 14 px circle 24 px above the top centre on a 1 px
//!   stem. Multi-selection: a thin box per object (accent at 50 %) and the union box with the
//!   handles. A group: the same box, dashed.
//! * Snap guides: 1 px `--annota-violet`, across the canvas and 30 px beyond it (mockup 02).
//! * Rubber band: 1 px accent border, accent at 20 % inside (GTK's rubberband style).

use crate::document::annotation::{Handle, HandleKind};
use crate::document::color::Rgba;
use crate::document::geometry::{Arrange, IRect, Point, Rect};
use crate::editing::selection::{visible_handles, SelectionFrame, HANDLE_SIZE, ROTATE_HANDLE_SIZE};
use crate::editing::snap::{Axis, Guide};

use super::Viewport;

/// Kit `--accent-bg`.
pub const ACCENT: Rgba = Rgba::opaque(0x35, 0x84, 0xE4);
/// Kit `--accent-a50`: thin boxes of the members of a multi-selection.
pub const ACCENT_HALF: Rgba = Rgba::new(0x35, 0x84, 0xE4, 0x80);
/// Kit `--annota-violet`: snap guides.
pub const VIOLET: Rgba = Rgba::opaque(0x8B, 0x5C, 0xF6);
/// How far guides reach beyond the canvas, in view pixels (mockup 02).
pub const GUIDE_OVERSHOOT: f64 = 30.0;
/// Dash pattern of a group's box, in view pixels.
pub const DASH: [f64; 2] = [3.0, 3.0];
const HANDLE_BORDER: f64 = 1.5;
const HANDLE_RADIUS: f64 = 2.0;

/// The middle of the view pixel containing `v`, so 1 px lines are crisp.
fn crisp(v: f64) -> f64 {
    v.floor() + 0.5
}

/// The corners of `arrange`'s box in view pixels.
fn corners(viewport: &Viewport, arrange: &Arrange) -> [Point; 4] {
    arrange
        .rect
        .corners()
        .map(|p| viewport.to_device(arrange.to_document(p)))
}

fn box_path(cr: &cairo::Context, viewport: &Viewport, arrange: &Arrange) {
    let c = corners(viewport, arrange);
    if arrange.rotation == 0.0 {
        let (x0, y0) = (crisp(c[0].x), crisp(c[0].y));
        let (x1, y1) = (crisp(c[2].x), crisp(c[2].y));
        cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    } else {
        cr.move_to(c[0].x, c[0].y);
        for p in &c[1..] {
            cr.line_to(p.x, p.y);
        }
        cr.close_path();
    }
}

fn stroke_box(
    cr: &cairo::Context,
    viewport: &Viewport,
    arrange: &Arrange,
    color: Rgba,
    dashed: bool,
) -> Result<(), cairo::Error> {
    box_path(cr, viewport, arrange);
    color.set_source(cr);
    cr.set_line_width(1.0);
    cr.set_dash(if dashed { &DASH } else { &[] }, 0.0);
    let stroked = cr.stroke();
    cr.set_dash(&[], 0.0);
    stroked
}

/// Rounded square of side `size` centred on `c`, turned by `degrees`.
fn square_path(cr: &cairo::Context, c: Point, size: f64, degrees: f64) -> Result<(), cairo::Error> {
    let h = size / 2.0;
    let r = HANDLE_RADIUS;
    cr.save()?;
    cr.translate(c.x, c.y);
    cr.rotate(degrees.to_radians());
    cr.new_sub_path();
    cr.arc(h - r, -h + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(h - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(
        -h + r,
        h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(
        -h + r,
        -h + r,
        r,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    cr.close_path();
    // The path keeps the coordinates it was built with.
    cr.restore()
}

fn fill_and_stroke(
    cr: &cairo::Context,
    fill: Rgba,
    border: Rgba,
    width: f64,
) -> Result<(), cairo::Error> {
    fill.set_source(cr);
    cr.fill_preserve()?;
    border.set_source(cr);
    cr.set_line_width(width);
    cr.stroke()
}

fn draw_handle(
    cr: &cairo::Context,
    viewport: &Viewport,
    handle: &Handle,
    rotation: f64,
) -> Result<(), cairo::Error> {
    let at = handle.screen_position(viewport.zoom, viewport.origin);
    let upright = rotation == 0.0;
    let c = if upright {
        Point::new(crisp(at.x), crisp(at.y))
    } else {
        at
    };
    match handle.kind {
        HandleKind::Resize => {
            square_path(cr, c, HANDLE_SIZE, rotation)?;
            fill_and_stroke(cr, Rgba::WHITE, ACCENT, HANDLE_BORDER)
        }
        HandleKind::Rotate => {
            // Stem from the box edge (the handle's anchor) to the circle.
            let base = viewport.to_device(handle.at);
            let base = if upright {
                Point::new(crisp(base.x), crisp(base.y))
            } else {
                base
            };
            ACCENT.set_source(cr);
            cr.set_line_width(1.0);
            cr.move_to(base.x, base.y);
            cr.line_to(c.x, c.y);
            cr.stroke()?;
            cr.new_sub_path();
            cr.arc(
                c.x,
                c.y,
                ROTATE_HANDLE_SIZE / 2.0,
                0.0,
                std::f64::consts::TAU,
            );
            fill_and_stroke(cr, Rgba::WHITE, ACCENT, HANDLE_BORDER)
        }
        HandleKind::Point => {
            cr.new_sub_path();
            cr.arc(c.x, c.y, HANDLE_SIZE / 2.0, 0.0, std::f64::consts::TAU);
            fill_and_stroke(cr, Rgba::WHITE, ACCENT, HANDLE_BORDER)
        }
        HandleKind::Curve => {
            square_path(cr, c, 12.0 / std::f64::consts::SQRT_2, 45.0)?;
            fill_and_stroke(cr, ACCENT, Rgba::WHITE, HANDLE_BORDER)
        }
        HandleKind::Radius => {
            cr.new_sub_path();
            cr.arc(c.x, c.y, HANDLE_SIZE / 2.0, 0.0, std::f64::consts::TAU);
            fill_and_stroke(cr, ACCENT, Rgba::WHITE, HANDLE_BORDER)
        }
        HandleKind::Add => {
            cr.new_sub_path();
            cr.arc(c.x, c.y, 9.0, 0.0, std::f64::consts::TAU);
            fill_and_stroke(cr, ACCENT, Rgba::WHITE, 2.0)?;
            Rgba::WHITE.set_source(cr);
            cr.set_line_width(2.0);
            cr.move_to(c.x - 4.0, c.y);
            cr.line_to(c.x + 4.0, c.y);
            cr.move_to(c.x, c.y - 4.0);
            cr.line_to(c.x, c.y + 4.0);
            cr.stroke()
        }
    }
}

/// The selection: member boxes, the box, then the handles on top.
pub fn draw_selection(
    cr: &cairo::Context,
    viewport: &Viewport,
    frame: &SelectionFrame,
) -> Result<(), cairo::Error> {
    cr.save()?;
    let result = (|| {
        for (arrange, group) in &frame.members {
            stroke_box(cr, viewport, arrange, ACCENT_HALF, *group)?;
        }
        if frame.boxed {
            stroke_box(cr, viewport, &frame.frame, ACCENT, frame.dashed)?;
        }
        for handle in visible_handles(frame, viewport.zoom) {
            draw_handle(cr, viewport, &handle, frame.frame.rotation)?;
        }
        Ok(())
    })();
    cr.restore()?;
    result
}

/// Snap guides across the canvas `canvas` (document pixels).
pub fn draw_guides(
    cr: &cairo::Context,
    viewport: &Viewport,
    canvas: IRect,
    guides: &[Guide],
) -> Result<(), cairo::Error> {
    if guides.is_empty() {
        return Ok(());
    }
    let r = canvas.to_rect();
    let top_left = viewport.to_device(Point::new(r.x, r.y));
    let bottom_right = viewport.to_device(Point::new(r.right(), r.bottom()));
    cr.save()?;
    VIOLET.set_source(cr);
    cr.set_line_width(1.0);
    for guide in guides {
        match guide.axis {
            Axis::Vertical => {
                let x = crisp(viewport.to_device(Point::new(guide.pos, 0.0)).x);
                cr.move_to(x, top_left.y - GUIDE_OVERSHOOT);
                cr.line_to(x, bottom_right.y + GUIDE_OVERSHOOT);
            }
            Axis::Horizontal => {
                let y = crisp(viewport.to_device(Point::new(0.0, guide.pos)).y);
                cr.move_to(top_left.x - GUIDE_OVERSHOOT, y);
                cr.line_to(bottom_right.x + GUIDE_OVERSHOOT, y);
            }
        }
    }
    let stroked = cr.stroke();
    cr.restore()?;
    stroked
}

/// Crop mode's dimming outside the frame (redline: `rgb(0 0 6 / 55%)`).
pub const CROP_DIM: Rgba = Rgba::new(0, 0, 6, 140);
/// Crop grips: L-shaped corners and edge bars 22 × 6 px, white, 1 px ink at 35 %, radius 2.
const GRIP_LONG: f64 = 22.0;
const GRIP_SHORT: f64 = 6.0;
/// How far the grips and the frame's outline reach outside the frame.
const GRIP_OUTSET: f64 = 4.0;

fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    super::shadow::rounded_rect(cr, x, y, w, h, r);
}

/// Crop mode (mockup 08): the canvas outside `frame` dimmed, a 2 px white frame, the
/// rule-of-thirds lines (1 px white at 60 %) and the grips.
pub fn draw_crop(
    cr: &cairo::Context,
    viewport: &Viewport,
    canvas: IRect,
    frame: IRect,
) -> Result<(), cairo::Error> {
    let device = |r: Rect| {
        let a = viewport.to_device(Point::new(r.x, r.y));
        let b = viewport.to_device(Point::new(r.right(), r.bottom()));
        (a.x.round(), a.y.round(), b.x.round(), b.y.round())
    };
    let (cx0, cy0, cx1, cy1) = device(canvas.to_rect());
    let (x0, y0, x1, y1) = device(frame.to_rect());
    cr.save()?;
    let result = (|| {
        // Dim: the canvas minus the frame.
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        cr.rectangle(cx0, cy0, cx1 - cx0, cy1 - cy0);
        cr.rectangle(x0, y0, x1 - x0, y1 - y0);
        CROP_DIM.set_source(cr);
        cr.fill()?;
        cr.set_fill_rule(cairo::FillRule::Winding);
        // Rule of thirds.
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.6);
        cr.set_line_width(1.0);
        for k in [1.0, 2.0] {
            let x = crisp(x0 + (x1 - x0) * k / 3.0);
            let y = crisp(y0 + (y1 - y0) * k / 3.0);
            cr.move_to(x, y0);
            cr.line_to(x, y1);
            cr.move_to(x0, y);
            cr.line_to(x1, y);
        }
        cr.stroke()?;
        // The frame: a thin dark line for contrast on white, then 2 px white outside.
        cr.set_source_rgba(0.0, 0.0, 6.0 / 255.0, 0.4);
        cr.rectangle(x0 - 2.5, y0 - 2.5, x1 - x0 + 5.0, y1 - y0 + 5.0);
        cr.stroke()?;
        Rgba::WHITE.set_source(cr);
        cr.set_line_width(2.0);
        cr.rectangle(x0 - 1.0, y0 - 1.0, x1 - x0 + 2.0, y1 - y0 + 2.0);
        cr.stroke()?;
        // Grips: an L at every corner, a bar at every edge midpoint.
        let (mx, my) = (((x0 + x1) / 2.0).round(), ((y0 + y1) / 2.0).round());
        let o = GRIP_OUTSET;
        let (l, s) = (GRIP_LONG, GRIP_SHORT);
        let bars = [
            // Corners: horizontal and vertical arm each.
            (x0 - o, y0 - o, l, s),
            (x0 - o, y0 - o, s, l),
            (x1 + o - l, y0 - o, l, s),
            (x1 + o - s, y0 - o, s, l),
            (x0 - o, y1 + o - s, l, s),
            (x0 - o, y1 + o - l, s, l),
            (x1 + o - l, y1 + o - s, l, s),
            (x1 + o - s, y1 + o - l, s, l),
            // Edges.
            (mx - l / 2.0, y0 - o, l, s),
            (mx - l / 2.0, y1 + o - s, l, s),
            (x0 - o, my - l / 2.0, s, l),
            (x1 + o - s, my - l / 2.0, s, l),
        ];
        for (x, y, w, h) in bars {
            rounded_rect(cr, x - 0.5, y - 0.5, w + 1.0, h + 1.0, 2.5);
            cr.set_source_rgba(0.0, 0.0, 6.0 / 255.0, 0.35);
            cr.fill()?;
            rounded_rect(cr, x, y, w, h, 2.0);
            Rgba::WHITE.set_source(cr);
            cr.fill()?;
        }
        Ok(())
    })();
    cr.restore()?;
    result
}

/// A canvas-edge grabber (redline: 12 × 36, radius 6, white with a 1.5 px accent border) at
/// `rect` (view pixels). While its edge is dragged, the edge itself is a 2 px accent line.
pub fn draw_canvas_grabber(
    cr: &cairo::Context,
    viewport: &Viewport,
    canvas: IRect,
    rect: Rect,
    active_edge: Option<crate::editing::canvas_edit::Side>,
) -> Result<(), cairo::Error> {
    use crate::editing::canvas_edit::Side;
    cr.save()?;
    let result = (|| {
        if let Some(side) = active_edge {
            let r = canvas.to_rect();
            let a = viewport.to_device(Point::new(r.x, r.y));
            let b = viewport.to_device(Point::new(r.right(), r.bottom()));
            let (a, b) = (
                Point::new(a.x.round(), a.y.round()),
                Point::new(b.x.round(), b.y.round()),
            );
            ACCENT.set_source(cr);
            match side {
                Side::Left => cr.rectangle(a.x, a.y, 2.0, b.y - a.y),
                Side::Right => cr.rectangle(b.x - 2.0, a.y, 2.0, b.y - a.y),
                Side::Top => cr.rectangle(a.x, a.y, b.x - a.x, 2.0),
                Side::Bottom => cr.rectangle(a.x, b.y - 2.0, b.x - a.x, 2.0),
            }
            cr.fill()?;
        }
        rounded_rect(
            cr,
            rect.x.round() + 0.75,
            rect.y.round() + 0.75,
            rect.width - 1.5,
            rect.height - 1.5,
            rect.width.min(rect.height) / 2.0 - 0.75,
        );
        fill_and_stroke(cr, Rgba::WHITE, ACCENT, HANDLE_BORDER)
    })();
    cr.restore()?;
    result
}

/// The dashed 1 px accent line at the image edge `side` while the canvas is extended there
/// (mockup 10).
pub fn draw_image_edge(
    cr: &cairo::Context,
    viewport: &Viewport,
    image: IRect,
    canvas: IRect,
    side: crate::editing::canvas_edit::Side,
) -> Result<(), cairo::Error> {
    use crate::editing::canvas_edit::Side;
    let i = image.to_rect();
    let c = canvas.to_rect();
    let (from, to) = match side {
        Side::Left => (Point::new(i.x, c.y), Point::new(i.x, c.bottom())),
        Side::Right => (
            Point::new(i.right(), c.y),
            Point::new(i.right(), c.bottom()),
        ),
        Side::Top => (Point::new(c.x, i.y), Point::new(c.right(), i.y)),
        Side::Bottom => (
            Point::new(c.x, i.bottom()),
            Point::new(c.right(), i.bottom()),
        ),
    };
    let (a, b) = (viewport.to_device(from), viewport.to_device(to));
    cr.save()?;
    ACCENT.set_source(cr);
    cr.set_line_width(1.0);
    cr.set_dash(&DASH, 0.0);
    if side.horizontal() {
        let x = crisp(a.x);
        cr.move_to(x, a.y);
        cr.line_to(x, b.y);
    } else {
        let y = crisp(a.y);
        cr.move_to(a.x, y);
        cr.line_to(b.x, y);
    }
    let stroked = cr.stroke();
    cr.restore()?;
    stroked
}

/// Loose handles (the image's corner-radius handle), drawn like selection handles.
pub fn draw_handles(
    cr: &cairo::Context,
    viewport: &Viewport,
    handles: &[Handle],
) -> Result<(), cairo::Error> {
    cr.save()?;
    let result = handles
        .iter()
        .try_for_each(|h| draw_handle(cr, viewport, h, 0.0));
    cr.restore()?;
    result
}

/// Rubber band `rect` (document space).
pub fn draw_band(cr: &cairo::Context, viewport: &Viewport, rect: Rect) -> Result<(), cairo::Error> {
    cr.save()?;
    let a = viewport.to_device(Point::new(rect.x, rect.y));
    let b = viewport.to_device(Point::new(rect.right(), rect.bottom()));
    let (x0, y0, x1, y1) = (crisp(a.x), crisp(a.y), crisp(b.x), crisp(b.y));
    cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    let (r, g, bl, _) = ACCENT.to_f64();
    cr.set_source_rgba(r, g, bl, 0.2);
    cr.fill_preserve()?;
    ACCENT.set_source(cr);
    cr.set_line_width(1.0);
    let stroked = cr.stroke();
    cr.restore()?;
    stroked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::geometry::Rect;
    use crate::document::scene::{tests::TestBox, Node, ObjectId, Scene};
    use crate::editing::selection::{selection_frame, Selection};
    use crate::editing::snap::Target;
    use crate::tools::group::Group;

    const CANVAS: Rgba = Rgba::opaque(0xDE, 0xDD, 0xDA);

    fn draw(f: impl Fn(&cairo::Context) -> Result<(), cairo::Error>) -> Bitmap {
        Bitmap::draw(200, 140, |cr| {
            CANVAS.set_source(cr);
            cr.paint()?;
            f(cr)
        })
        .unwrap()
    }

    fn frame_of(scene: &Scene, ids: &[ObjectId]) -> SelectionFrame {
        let mut sel = Selection::new();
        sel.set(scene, ids.iter().copied());
        selection_frame(scene, &sel, &[]).unwrap()
    }

    #[test]
    fn single_selection_box_and_handles() {
        let scene = Scene::try_from_nodes(vec![Node::new(
            ObjectId(1),
            TestBox::boxed(50.0, 50.0, 100.0, 60.0),
        )])
        .unwrap();
        let frame = frame_of(&scene, &[ObjectId(1)]);
        let vp = Viewport::new(1.0, Point::default());
        let out = draw(|cr| draw_selection(cr, &vp, &frame));
        // 1 px accent box along x = 50 (between handles).
        assert_eq!(out.pixel(50, 70), Some(ACCENT));
        assert_eq!(out.pixel(49, 70), Some(CANVAS));
        assert_eq!(out.pixel(51, 70), Some(CANVAS));
        // NW handle: white centre, accent border, 10 px wide around (50.5, 50.5).
        assert_eq!(out.pixel(52, 52), Some(Rgba::WHITE));
        assert_eq!(
            out.pixel(45, 52).map(|p| p.b > 200 && p.r < 120),
            Some(true)
        );
        assert_eq!(out.pixel(43, 50), Some(CANVAS));
        // Rotation handle: white circle 24 px above the top centre (100.5, 26.5) on a stem.
        assert_eq!(out.pixel(100, 26), Some(Rgba::WHITE));
        assert_eq!(out.pixel(100, 40), Some(ACCENT), "stem");
        assert_eq!(out.pixel(103, 40), Some(CANVAS));
    }

    #[test]
    fn group_box_is_dashed_and_multi_selection_has_member_boxes() {
        let group = Group::new(vec![
            Node::new(ObjectId(2), TestBox::boxed(20.0, 50.0, 40.0, 40.0)),
            Node::new(ObjectId(3), TestBox::boxed(120.0, 50.0, 40.0, 40.0)),
        ]);
        let scene = Scene::try_from_nodes(vec![Node::new(ObjectId(1), Box::new(group))]).unwrap();
        let vp = Viewport::new(1.0, Point::default());
        let out = draw(|cr| draw_selection(cr, &vp, &frame_of(&scene, &[ObjectId(1)])));
        // Along the top edge y = 50 between the corner and middle handles: dashes and gaps.
        let row: Vec<bool> = (30..60).map(|x| out.pixel(x, 50) == Some(ACCENT)).collect();
        assert!(
            row.iter().any(|on| *on) && row.iter().any(|on| !*on),
            "{row:?}"
        );

        let scene = Scene::try_from_nodes(vec![
            Node::new(ObjectId(1), TestBox::boxed(20.0, 50.0, 40.0, 40.0)),
            Node::new(ObjectId(2), TestBox::boxed(120.0, 70.0, 40.0, 20.0)),
        ])
        .unwrap();
        let out =
            draw(|cr| draw_selection(cr, &vp, &frame_of(&scene, &[ObjectId(1), ObjectId(2)])));
        // Member 2's own top edge at y = 70 in half-transparent accent, union box top solid.
        let thin = out.pixel(140, 70).unwrap();
        assert!(thin != CANVAS && thin != ACCENT, "{thin:?}");
        assert_eq!(out.pixel(80, 50), Some(ACCENT));
    }

    #[test]
    fn guides_cross_the_canvas_and_beyond() {
        let vp = Viewport::new(0.5, Point::new(40.0, 40.0));
        let canvas = IRect::new(0, 0, 200, 100);
        let guides = [Guide {
            axis: Axis::Vertical,
            pos: 100.0,
            target: Target::CanvasCentre,
        }];
        let out = draw(|cr| draw_guides(cr, &vp, canvas, &guides));
        // x = 40 + 50 = 90; canvas spans y 40..90, guide 10..120.
        assert_eq!(out.pixel(90, 15), Some(VIOLET));
        assert_eq!(out.pixel(90, 115), Some(VIOLET));
        assert_eq!(out.pixel(90, 125), Some(CANVAS));
        assert_eq!(out.pixel(91, 60), Some(CANVAS));
    }

    #[test]
    fn crop_mode_dims_outside_and_draws_frame_and_grips() {
        let vp = Viewport::new(1.0, Point::new(10.0, 10.0));
        let canvas = IRect::new(0, 0, 180, 120);
        let frame = IRect::new(40, 30, 90, 60);
        let out = draw(|cr| draw_crop(cr, &vp, canvas, frame));
        // Outside the frame, inside the canvas: dimmed.
        let dim = out.pixel(20, 20).unwrap();
        assert!(dim.r < CANVAS.r / 2, "{dim:?}");
        // Inside the frame between the thirds: untouched.
        assert_eq!(out.pixel(60, 50), Some(CANVAS));
        // A thirds line (x = 50 + 30) is lighter than the canvas around it.
        let third = out.pixel(80, 70).unwrap();
        assert!(third.r > CANVAS.r, "{third:?}");
        // The NW grip covers the frame corner, white.
        assert_eq!(out.pixel(50 - 2, 40 + 5), Some(Rgba::WHITE));
        // Outside the canvas: untouched.
        assert_eq!(out.pixel(195, 135), Some(CANVAS));
    }

    #[test]
    fn canvas_grabber_and_image_edge() {
        use crate::editing::canvas_edit::{grabber_rect, Side};
        let vp = Viewport::new(1.0, Point::new(10.0, 10.0));
        let canvas = IRect::new(0, 0, 150, 100);
        let rect = grabber_rect(&vp, canvas, Side::Right);
        let out = draw(|cr| {
            draw_canvas_grabber(cr, &vp, canvas, rect, Some(Side::Right))?;
            draw_image_edge(cr, &vp, IRect::new(0, 0, 100, 100), canvas, Side::Right)
        });
        // White grabber centred on the right edge (x = 160), accent edge line above it.
        assert_eq!(out.pixel(160, 60), Some(Rgba::WHITE));
        assert_eq!(out.pixel(159, 20), Some(ACCENT));
        // Dashed line at x = 110: dashes and gaps.
        let column: Vec<bool> = (10..40)
            .map(|y| out.pixel(110, y) == Some(ACCENT))
            .collect();
        assert!(column.iter().any(|on| *on) && column.iter().any(|on| !*on));
    }

    #[test]
    fn rubber_band_is_tinted() {
        let vp = Viewport::new(1.0, Point::default());
        let out = draw(|cr| draw_band(cr, &vp, Rect::new(10.0, 10.0, 50.0, 30.0)));
        assert_eq!(out.pixel(10, 20), Some(ACCENT));
        let inside = out.pixel(30, 20).unwrap();
        assert!(inside.b > CANVAS.b && inside.r < CANVAS.r, "{inside:?}");
    }
}
