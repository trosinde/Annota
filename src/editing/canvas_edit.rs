//! Direct manipulation of the canvas itself with the Select tool (spec editor-canvas: Canvas
//! extension, Image operations — rounded corners; redline "Canvas", mockups 09 and 10):
//!
//! * **Canvas edges.** Near a canvas edge a grabber (12 × 36, at the edge's midpoint) appears;
//!   dragging the edge outward extends the canvas, inward shrinks it back — but not into the
//!   image. While dragging, a dashed line marks the image edge and a chip says how far the
//!   canvas reaches beyond it ("+400 px").
//! * **Rounded corners.** One handle at the image's top-left inner corner, offset by the radius
//!   (`HandleKind::Radius`), shown when nothing is selected and the pointer is near it. Dragging
//!   it along the diagonal sets the radius ("Corner radius 20 px").
//!
//! Like object gestures, the document is untouched during the drag (the canvas draws the
//! preview values) and the release is one [`Edit`].

use crate::document::annotation::{Handle, HandleId, HandleKind};
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
            Target::Edge(side) => {
                let (c, i) = (self.canvas, self.image);
                let extra = match side {
                    Side::Right => c.right() - i.right(),
                    Side::Bottom => c.bottom() - i.bottom(),
                    Side::Left => i.x - c.x,
                    Side::Top => i.y - c.y,
                };
                Feedback::Extend { side, extra }
            }
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
