//! Direct manipulation of the canvas itself with the Select tool (spec editor-canvas: Canvas
//! extension, Image operations — rounded corners; redline "Canvas", mockups 09 and 10):
//!
//! * **Canvas edges.** Near a canvas edge a grabber (12 × 36, at the edge's midpoint) appears;
//!   dragging the edge outward extends the canvas, inward shrinks it back — but not into the
//!   image. While dragging, a dashed line marks the image edge and a chip says how far the
//!   canvas reaches beyond it ("+400 px").
//! * **Space around the image** (keyboard path, CLAUDE.md §7): the sidebar's Canvas group
//!   enters how far each canvas edge reaches beyond the image ([`extension`], [`extended`]);
//!   the same limits as dragging apply.
//! * **Rounded corners.** One handle at the image's top-left inner corner, offset by the radius
//!   (`HandleKind::Radius`), shown when nothing is selected and the pointer is near it. Dragging
//!   it along the diagonal sets the radius ("Corner radius 20 px").
//!
//! Like object gestures, the document is untouched during the drag (the canvas draws the
//! preview values) and the release is one [`Edit`].

use crate::document::annotation::{Handle, HandleId, HandleKind};
use crate::document::bitmap::MAX_SIDE;
use crate::document::geometry::{IRect, Point, Rect};
use crate::document::Document;
use crate::render::Viewport;

use super::drag::DRAG_THRESHOLD;
use super::image_ops::{self, image_rect};
use super::ops::Edit;

/// Grabber size along and across the edge, in screen pixels (redline: 12 × 36, radius 6).
pub const GRABBER_LONG: f64 = 36.0;
pub const GRABBER_SHORT: f64 = 12.0;
/// How far outside a canvas edge (screen pixels) the pointer still finds it; inside, only a
/// sliver, so objects at the canvas edge stay easy to click.
pub const EDGE_REACH_OUT: f64 = 10.0;
pub const EDGE_REACH_IN: f64 = 4.0;
/// Pointer distance (screen pixels) at which the corner-radius handle appears and is grabbed.
pub const RADIUS_REACH: f64 = 9.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Top, Side::Right, Side::Bottom];

    /// Whether the side moves along x.
    pub fn horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
}

/// How far `canvas` reaches beyond `image` on `side`, in pixels ("+400 px"; negative where the
/// canvas ends inside the image).
pub fn extension(canvas: IRect, image: IRect, side: Side) -> i32 {
    match side {
        Side::Right => canvas.right() - image.right(),
        Side::Bottom => canvas.bottom() - image.bottom(),
        Side::Left => image.x - canvas.x,
        Side::Top => image.y - canvas.y,
    }
}

/// `canvas` with `side` reaching `extra` pixels beyond `image`, the other edges where they are.
/// As when dragging, the edge does not go into the image (unless the canvas already ended inside
/// it there) and the canvas keeps at least 1 pixel; it is also never wider or higher than a
/// bitmap can be ([`MAX_SIDE`]), so it can always be exported.
pub fn extended(canvas: IRect, image: IRect, side: Side, extra: i32) -> IRect {
    let current = extension(canvas, image, side);
    let extra = extra.max(current.min(0));
    let mut c = canvas;
    // The far edge (fixed) and the extent the moving edge may span from it.
    let clamp_span = |span: i64| span.clamp(1, i64::from(MAX_SIDE)) as i32;
    match side {
        Side::Right => {
            let right = i64::from(image.right()) + i64::from(extra);
            c.width = clamp_span(right - i64::from(c.x));
        }
        Side::Bottom => {
            let bottom = i64::from(image.bottom()) + i64::from(extra);
            c.height = clamp_span(bottom - i64::from(c.y));
        }
        Side::Left => {
            let x = i64::from(image.x) - i64::from(extra);
            let right = c.right();
            c.width = clamp_span(i64::from(right) - x);
            c.x = right - c.width;
        }
        Side::Top => {
            let y = i64::from(image.y) - i64::from(extra);
            let bottom = c.bottom();
            c.height = clamp_span(i64::from(bottom) - y);
            c.y = bottom - c.height;
        }
    }
    c
}

/// The canvas with every side's extension set (Left, Top, Right, Bottom, as in [`Side::ALL`]):
/// what the sidebar's Canvas entries ask for.
pub fn with_extensions(doc: &Document, extensions: [i32; 4]) -> IRect {
    let image = image_rect(doc);
    Side::ALL
        .iter()
        .zip(extensions)
        .fold(doc.canvas().rect, |canvas, (&side, extra)| {
            extended(canvas, image, side, extra)
        })
}

/// What a press on the canvas (outside the selection's handles) can grab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Edge(Side),
    Radius,
}

/// The corner-radius handle of the image for a radius of `radius`: at (r, r) from its corner.
pub fn radius_handle(radius: f64) -> Handle {
    Handle::new(
        HandleId::Custom(0),
        HandleKind::Radius,
        Point::new(radius, radius),
    )
}

/// The grabber of `side` in view pixels.
pub fn grabber_rect(viewport: &Viewport, canvas: IRect, side: Side) -> Rect {
    let r = canvas.to_rect();
    let a = viewport.to_device(Point::new(r.x, r.y));
    let b = viewport.to_device(Point::new(r.right(), r.bottom()));
    let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let (long, short) = (GRABBER_LONG, GRABBER_SHORT);
    match side {
        Side::Left => Rect::new(a.x - short / 2.0, my - long / 2.0, short, long),
        Side::Right => Rect::new(b.x - short / 2.0, my - long / 2.0, short, long),
        Side::Top => Rect::new(mx - long / 2.0, a.y - short / 2.0, long, short),
        Side::Bottom => Rect::new(mx - long / 2.0, b.y - short / 2.0, long, short),
    }
}

/// The canvas edge or the corner-radius handle under `screen` (view pixels), if any.
/// `selection_empty`: the radius handle is only offered when nothing is selected.
pub fn target_at(
    doc: &Document,
    viewport: &Viewport,
    screen: Point,
    selection_empty: bool,
) -> Option<Target> {
    if selection_empty {
        let handle = radius_handle(doc.image().corner_radius);
        let at = handle.screen_position(viewport.zoom, viewport.origin);
        if at.distance(screen) <= RADIUS_REACH {
            return Some(Target::Radius);
        }
    }
    let r = doc.canvas().rect.to_rect();
    let a = viewport.to_device(Point::new(r.x, r.y));
    let b = viewport.to_device(Point::new(r.right(), r.bottom()));
    let along_x = screen.x >= a.x - EDGE_REACH_OUT && screen.x <= b.x + EDGE_REACH_OUT;
    let along_y = screen.y >= a.y - EDGE_REACH_OUT && screen.y <= b.y + EDGE_REACH_OUT;
    // Outward distance from each edge (positive outside the canvas).
    let reach = |outward: f64| (-EDGE_REACH_IN..=EDGE_REACH_OUT).contains(&outward);
    let candidates = [
        (Side::Left, a.x - screen.x, along_y),
        (Side::Right, screen.x - b.x, along_y),
        (Side::Top, a.y - screen.y, along_x),
        (Side::Bottom, screen.y - b.y, along_x),
    ];
    candidates
        .iter()
        .filter(|(_, d, along)| *along && reach(*d))
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(side, _, _)| Target::Edge(*side))
}

/// Live values for the canvas chips.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feedback {
    None,
    /// The canvas reaches `extra` pixels beyond the image on `side` ("+400 px").
    Extend {
        side: Side,
        extra: i32,
    },
    /// "Corner radius 20 px".
    Radius(f64),
}

/// A drag of a canvas edge or of the corner-radius handle.
#[derive(Clone, Debug)]
pub struct CanvasDrag {
    target: Target,
    start_screen: Point,
    start_canvas: IRect,
    image: IRect,
    max_radius: f64,
    moved: bool,
    canvas: IRect,
    radius: f64,
}

impl CanvasDrag {
    pub fn press(
        doc: &Document,
        viewport: &Viewport,
        screen: Point,
        selection_empty: bool,
    ) -> Option<Self> {
        let target = target_at(doc, viewport, screen, selection_empty)?;
        let canvas = doc.canvas().rect;
        Some(Self {
            target,
            start_screen: screen,
            start_canvas: canvas,
            image: image_rect(doc),
            max_radius: image_ops::max_corner_radius(doc),
            moved: false,
            canvas,
            radius: doc.image().corner_radius,
        })
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn is_dragging(&self) -> bool {
        self.moved
    }

    /// The pointer moved to `screen`.
    pub fn motion(&mut self, viewport: &Viewport, screen: Point) {
        if !self.moved {
            if screen.distance(self.start_screen) < DRAG_THRESHOLD {
                return;
            }
            self.moved = true;
        }
        let at = viewport.to_document(screen);
        match self.target {
            Target::Radius => {
                // Along the diagonal from the corner.
                let r = ((at.x + at.y) / 2.0).round();
                self.radius = r.clamp(0.0, self.max_radius);
            }
            Target::Edge(side) => {
                let s = self.start_canvas;
                let (image, x, y) = (self.image, at.x.round() as i32, at.y.round() as i32);
                let mut c = s;
                match side {
                    Side::Right => {
                        // Not into the image (unless the canvas already ended inside it).
                        let min = image.right().min(s.right()).max(s.x + 1);
                        c.width = x.max(min) - s.x;
                    }
                    Side::Bottom => {
                        let min = image.bottom().min(s.bottom()).max(s.y + 1);
                        c.height = y.max(min) - s.y;
                    }
                    Side::Left => {
                        let max = image.x.max(s.x).min(s.right() - 1);
                        c.x = x.min(max);
                        c.width = s.right() - c.x;
                    }
                    Side::Top => {
                        let max = image.y.max(s.y).min(s.bottom() - 1);
                        c.y = y.min(max);
                        c.height = s.bottom() - c.y;
                    }
                }
                self.canvas = c;
            }
        }
    }

    /// The canvas to draw while an edge is dragged.
    pub fn preview_canvas(&self) -> Option<IRect> {
        (self.moved && matches!(self.target, Target::Edge(_))).then_some(self.canvas)
    }

    /// The corner radius to draw while its handle is dragged.
    pub fn preview_radius(&self) -> Option<f64> {
        (self.moved && self.target == Target::Radius).then_some(self.radius)
    }

    pub fn feedback(&self) -> Feedback {
        if !self.moved {
            return Feedback::None;
        }
        match self.target {
            Target::Radius => Feedback::Radius(self.radius),
            Target::Edge(side) => Feedback::Extend {
                side,
                extra: extension(self.canvas, self.image, side),
            },
        }
    }

    /// The edit to execute on release (one undo step), if anything changed.
    pub fn release(self, doc: &Document) -> Option<Edit> {
        if !self.moved {
            return None;
        }
        match self.target {
            Target::Radius => image_ops::set_corner_radius(doc, self.radius),
            Target::Edge(_) => image_ops::set_canvas_rect(doc, self.canvas),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::History;
    use crate::editing::ops::Action;

    /// 1600 × 1000 at 50 %, document (0, 0) at view (44, 44) (mockup 10's start).
    fn setup() -> (Document, History, Viewport) {
        let doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        (
            doc,
            History::new(),
            Viewport::new(0.5, Point::new(44.0, 44.0)),
        )
    }

    #[test]
    fn dragging_the_right_edge_extends_the_canvas() {
        // Spec "Extend for callout" through the gesture: the right edge dragged 200 px out.
        let (mut doc, mut history, vp) = setup();
        let edge = vp.to_device(Point::new(1600.0, 500.0));
        assert_eq!(
            target_at(&doc, &vp, Point::new(edge.x + 5.0, edge.y), true),
            Some(Target::Edge(Side::Right))
        );
        let mut drag = CanvasDrag::press(&doc, &vp, edge, true).unwrap();
        drag.motion(&vp, vp.to_device(Point::new(1800.0, 520.0)));
        assert_eq!(drag.preview_canvas(), Some(IRect::new(0, 0, 1800, 1000)));
        assert_eq!(
            drag.feedback(),
            Feedback::Extend {
                side: Side::Right,
                extra: 200
            }
        );
        assert_eq!(doc.canvas().rect.width, 1600, "untouched until release");
        let edit = drag.release(&doc).unwrap();
        assert_eq!(edit.action, Action::ExtendCanvas);
        history.execute(&mut doc, "Extend", edit.command).unwrap();
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 1800, 1000));
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 1600, 1000));
    }

    #[test]
    fn edges_do_not_cut_into_the_image() {
        let (doc, _, vp) = setup();
        // Left edge dragged inward stops at the image edge; outward goes negative.
        let left = vp.to_device(Point::new(0.0, 500.0));
        let mut drag = CanvasDrag::press(&doc, &vp, left, true).unwrap();
        drag.motion(&vp, vp.to_device(Point::new(300.0, 500.0)));
        assert_eq!(drag.preview_canvas(), Some(IRect::new(0, 0, 1600, 1000)));
        drag.motion(&vp, vp.to_device(Point::new(-120.0, 500.0)));
        assert_eq!(drag.preview_canvas(), Some(IRect::new(-120, 0, 1720, 1000)));
        assert_eq!(
            drag.feedback(),
            Feedback::Extend {
                side: Side::Left,
                extra: 120
            }
        );
        // Far inside the canvas and away from the corner, nothing is grabbed.
        assert_eq!(
            target_at(&doc, &vp, vp.to_device(Point::new(800.0, 500.0)), true),
            None
        );
    }

    #[test]
    fn radius_handle_sets_rounded_corners() {
        let (mut doc, mut history, vp) = setup();
        let at = radius_handle(0.0).screen_position(vp.zoom, vp.origin);
        assert_eq!(target_at(&doc, &vp, at, true), Some(Target::Radius));
        // Only without a selection.
        assert_ne!(target_at(&doc, &vp, at, false), Some(Target::Radius));
        let mut drag = CanvasDrag::press(&doc, &vp, at, true).unwrap();
        drag.motion(&vp, vp.to_device(Point::new(20.0, 20.0)));
        assert_eq!(drag.feedback(), Feedback::Radius(20.0));
        assert_eq!(drag.preview_radius(), Some(20.0));
        let edit = drag.release(&doc).unwrap();
        assert_eq!(edit.action, Action::CornerRadius);
        history.execute(&mut doc, "Radius", edit.command).unwrap();
        assert_eq!(doc.image().corner_radius, 20.0);
        // The handle now sits 20 px in from the corner and grows no further than 500 px.
        let at = radius_handle(20.0).screen_position(vp.zoom, vp.origin);
        let mut drag = CanvasDrag::press(&doc, &vp, at, true).unwrap();
        drag.motion(&vp, vp.to_device(Point::new(2000.0, 2000.0)));
        assert_eq!(drag.preview_radius(), Some(500.0));
    }

    #[test]
    fn entered_extensions_extend_the_canvas() {
        // B3 (keyboard path): the sidebar's Canvas entries set how far each edge reaches
        // beyond the image, as one undo step, with the limits of dragging.
        let (mut doc, mut history, _) = setup();
        let image = image_rect(&doc);
        let canvas = doc.canvas().rect;
        assert_eq!(
            Side::ALL.map(|side| extension(canvas, image, side)),
            [0, 0, 0, 0]
        );
        let rect = with_extensions(&doc, [100, 0, 200, 50]);
        assert_eq!(rect, IRect::new(-100, 0, 1900, 1050));
        let edit = image_ops::set_canvas_rect(&doc, rect).unwrap();
        assert_eq!(edit.action, Action::ExtendCanvas);
        history.execute(&mut doc, "Extend", edit.command).unwrap();
        let canvas = doc.canvas().rect;
        assert_eq!(
            Side::ALL.map(|side| extension(canvas, image, side)),
            [100, 0, 200, 50]
        );
        // The same values again: no change, so no edit (no phantom undo step).
        let same = with_extensions(&doc, [100, 0, 200, 50]);
        assert!(image_ops::set_canvas_rect(&doc, same).is_none());
        // Not into the image: a negative extension stops at the image edge.
        assert_eq!(
            with_extensions(&doc, [-40, -1, 0, 0]),
            IRect::new(0, 0, 1600, 1000)
        );
        // Never larger than a bitmap can be.
        let huge = with_extensions(&doc, [i32::MAX, 0, i32::MAX, 0]);
        assert_eq!(huge.width, MAX_SIDE);
        assert_eq!(
            huge.right(),
            canvas.right(),
            "the left edge moved, the right one kept"
        );
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 1600, 1000));
    }

    #[test]
    fn a_canvas_inside_the_image_can_only_grow() {
        // Unless the canvas already ended inside the image there, as when dragging.
        let image = IRect::new(0, 0, 100, 80);
        let canvas = IRect::new(10, 0, 80, 80);
        assert_eq!(extension(canvas, image, Side::Left), -10);
        assert_eq!(extension(canvas, image, Side::Right), -10);
        assert_eq!(extended(canvas, image, Side::Left, -20), canvas);
        assert_eq!(
            extended(canvas, image, Side::Left, -4),
            IRect::new(4, 0, 86, 80)
        );
        assert_eq!(
            extended(canvas, image, Side::Right, 5),
            IRect::new(10, 0, 95, 80)
        );
    }

    #[test]
    fn grabbers_sit_on_the_edge_midpoints() {
        let vp = Viewport::new(1.0, Point::default());
        let canvas = IRect::new(0, 0, 200, 100);
        assert_eq!(
            grabber_rect(&vp, canvas, Side::Right),
            Rect::new(194.0, 32.0, 12.0, 36.0)
        );
        assert_eq!(
            grabber_rect(&vp, canvas, Side::Top),
            Rect::new(82.0, -6.0, 36.0, 12.0)
        );
    }
}
