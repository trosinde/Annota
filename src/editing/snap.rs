//! Snapping and guides on the canvas (spec editor-canvas: Snapping and guides; mockup 02).
//!
//! While objects are moved, the moving box snaps with its left edge, centre or right edge (and
//! top, middle, bottom) to the canvas edges, the canvas centre and the edges and centres of the
//! other objects, when one of them is within [`SNAP_DISTANCE`] *screen* pixels. Each axis
//! snaps on its own to the nearest line; at equal distance the canvas centre beats a canvas
//! edge, which beats an object centre, which beats an object edge. Every line the box lines up
//! with after snapping becomes a guide, and the chip names the snapped-to target ("Canvas
//! centre"). A dragged resize handle snaps the same way, as a point. Holding Alt suspends
//! snapping, as on the capture overlay (`capture::snap`); `Ctrl+;` turns it off.
//!
//! Everything here is plain arithmetic in document space; the tolerance is
//! `SNAP_DISTANCE / zoom`.

use crate::document::geometry::{Point, Rect};
use crate::document::scene::ObjectId;
use crate::document::Document;

use super::selection::is_selectable;

/// Snap distance in screen pixels (spec).
pub const SNAP_DISTANCE: f64 = 6.0;

/// Positions closer than this (document units) count as the same line.
const SAME: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// A vertical line at x = `pos`: snaps horizontal positions.
    Vertical,
    /// A horizontal line at y = `pos`: snaps vertical positions.
    Horizontal,
}

/// What a guide lines up with; the order is the priority at equal distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    CanvasCentre,
    CanvasEdge,
    ObjectCentre,
    ObjectEdge,
}

/// A guide line to draw, in document space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub axis: Axis,
    pub pos: f64,
    pub target: Target,
}

/// The lines a drag can snap to, collected once when it starts.
#[derive(Clone, Debug, Default)]
pub struct Targets {
    /// x positions of vertical lines.
    vertical: Vec<(f64, Target)>,
    /// y positions of horizontal lines.
    horizontal: Vec<(f64, Target)>,
}

impl Targets {
    /// The canvas and every selectable object except `exclude` (the objects being dragged).
    pub fn new(doc: &Document, exclude: &[ObjectId]) -> Self {
        let mut targets = Self::default();
        let canvas = doc.canvas().rect.to_rect();
        targets.add_box(canvas, Target::CanvasEdge, Target::CanvasCentre);
        for node in doc.scene().nodes() {
            if exclude.contains(&node.id) || !is_selectable(node.object.as_ref()) {
                continue;
            }
            let b = node.object.arrange().bounds();
            targets.add_box(b, Target::ObjectEdge, Target::ObjectCentre);
        }
        targets
    }

    fn add_box(&mut self, r: Rect, edge: Target, centre: Target) {
        if r.is_none() {
            return;
        }
        let c = r.centre();
        self.vertical
            .extend([(r.x, edge), (c.x, centre), (r.right(), edge)]);
        self.horizontal
            .extend([(r.y, edge), (c.y, centre), (r.bottom(), edge)]);
    }
}

/// The correction to apply and the guides to show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snap {
    pub dx: f64,
    pub dy: f64,
    pub guides: Vec<Guide>,
}

impl Snap {
    /// The targets the guides name, most important first, each once (chip text).
    pub fn targets(guides: &[Guide]) -> Vec<Target> {
        let mut t: Vec<Target> = guides.iter().map(|g| g.target).collect();
        t.sort_unstable();
        t.dedup();
        t
    }
}

/// Snaps one axis: `candidates` are the moving positions (edges, centre). Returns the
/// correction and the lines that coincide after it, or `None` if nothing is in reach.
fn snap_axis(
    lines: &[(f64, Target)],
    candidates: &[f64],
    tolerance: f64,
) -> Option<(f64, Vec<(f64, Target)>)> {
    let mut best: Option<(f64, Target, f64)> = None; // (distance, target, correction)
    for &(pos, target) in lines {
        for &c in candidates {
            let d = pos - c;
            if d.abs() > tolerance {
                continue;
            }
            let better = match best {
                None => true,
                Some((bd, bt, _)) => {
                    d.abs() < bd - SAME || ((d.abs() - bd).abs() <= SAME && target < bt)
                }
            };
            if better {
                best = Some((d.abs(), target, d));
            }
        }
    }
    let (_, _, correction) = best?;
    let mut hits: Vec<(f64, Target)> = lines
        .iter()
        .filter(|(pos, _)| {
            candidates
                .iter()
                .any(|c| (c + correction - pos).abs() <= SAME)
        })
        .copied()
        .collect();
    // One guide per line: the most important target there.
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    hits.dedup_by(|later, earlier| (later.0 - earlier.0).abs() <= SAME);
    Some((correction, hits))
}

fn guides(axis: Axis, lines: Vec<(f64, Target)>) -> impl Iterator<Item = Guide> {
    lines
        .into_iter()
        .map(move |(pos, target)| Guide { axis, pos, target })
}

/// Snaps a moving box (`rect`, document space) by its edges and centre. `tolerance` is in
/// document units.
pub fn snap_rect(targets: &Targets, rect: Rect, tolerance: f64) -> Snap {
    let c = rect.centre();
    let mut snap = Snap::default();
    if let Some((dx, lines)) = snap_axis(&targets.vertical, &[rect.x, c.x, rect.right()], tolerance)
    {
        snap.dx = dx;
        snap.guides.extend(guides(Axis::Vertical, lines));
    }
    if let Some((dy, lines)) = snap_axis(
        &targets.horizontal,
        &[rect.y, c.y, rect.bottom()],
        tolerance,
    ) {
        snap.dy = dy;
        snap.guides.extend(guides(Axis::Horizontal, lines));
    }
    snap
}

/// Snaps a dragged point (a resize handle) on the axes it moves along.
pub fn snap_point(targets: &Targets, p: Point, x: bool, y: bool, tolerance: f64) -> Snap {
    let mut snap = Snap::default();
    if x {
        if let Some((dx, lines)) = snap_axis(&targets.vertical, &[p.x], tolerance) {
            snap.dx = dx;
            snap.guides.extend(guides(Axis::Vertical, lines));
        }
    }
    if y {
        if let Some((dy, lines)) = snap_axis(&targets.horizontal, &[p.y], tolerance) {
            snap.dy = dy;
            snap.guides.extend(guides(Axis::Horizontal, lines));
        }
    }
    snap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::Command;
    use crate::document::scene::{tests::TestBox, Node};

    /// A 1600 × 1000 canvas with one object at (100, 100, 200, 100).
    fn doc() -> Document {
        let mut doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let id = doc.scene().fresh_ids(1)[0];
        Command::Insert {
            nodes: vec![(0, Node::new(id, TestBox::boxed(100.0, 100.0, 200.0, 100.0)))],
            resources: Vec::new(),
        }
        .apply(&mut doc)
        .unwrap();
        doc
    }

    #[test]
    fn centre_snap() {
        // Spec "Centre snap" / mockup 02: a 440 × 66 box dragged near the canvas centre
        // (800, 500) snaps onto it on both axes, with a guide per axis naming the centre.
        let doc = doc();
        let targets = Targets::new(&doc, &[]);
        let zoom = 0.5;
        let tolerance = SNAP_DISTANCE / zoom; // 6 screen px = 12 document px at 50 %
        let moving = Rect::new(800.0 - 220.0 + 9.0, 500.0 - 33.0 - 7.0, 440.0, 66.0);
        let snap = snap_rect(&targets, moving, tolerance);
        assert_eq!((snap.dx, snap.dy), (-9.0, 7.0));
        let snapped = moving.translate(snap.dx, snap.dy);
        assert_eq!(snapped.centre(), Point::new(800.0, 500.0));
        assert_eq!(
            snap.guides,
            vec![
                Guide {
                    axis: Axis::Vertical,
                    pos: 800.0,
                    target: Target::CanvasCentre
                },
                Guide {
                    axis: Axis::Horizontal,
                    pos: 500.0,
                    target: Target::CanvasCentre
                },
            ]
        );
        assert_eq!(Snap::targets(&snap.guides), vec![Target::CanvasCentre]);
    }

    #[test]
    fn distance_is_six_screen_pixels() {
        let doc = doc();
        let targets = Targets::new(&doc, &[]);
        let moving = |dx: f64| Rect::new(800.0 - 220.0 + dx, 600.0, 440.0, 66.0);
        // At 100 %: 6 px snaps, 7 px does not.
        assert_eq!(snap_rect(&targets, moving(6.0), 6.0).dx, -6.0);
        assert!(snap_rect(&targets, moving(7.0), 6.0)
            .guides
            .iter()
            .all(|g| g.axis != Axis::Vertical));
        // At 200 %, 6 screen px are 3 document px.
        assert_eq!(
            snap_rect(&targets, moving(4.0), SNAP_DISTANCE / 2.0).dx,
            0.0
        );
        assert_eq!(
            snap_rect(&targets, moving(3.0), SNAP_DISTANCE / 2.0).dx,
            -3.0
        );
    }

    #[test]
    fn object_edges_and_centres() {
        let doc = doc();
        let targets = Targets::new(&doc, &[]);
        // Left edge 3 px right of the object's right edge (x = 300).
        let snap = snap_rect(&targets, Rect::new(303.0, 400.0, 50.0, 50.0), 6.0);
        assert_eq!(snap.dx, -3.0);
        assert_eq!(snap.guides[0].target, Target::ObjectEdge);
        assert_eq!(snap.guides[0].pos, 300.0);
        // Centre 2 px below the object's middle (y = 150).
        let snap = snap_rect(&targets, Rect::new(900.0, 127.0, 50.0, 50.0), 6.0);
        assert_eq!(snap.dy, -2.0);
        assert_eq!(snap.guides[0].target, Target::ObjectCentre);
        // Canvas edge.
        let snap = snap_rect(&targets, Rect::new(1546.0, 700.0, 50.0, 50.0), 6.0);
        assert_eq!(snap.dx, 4.0);
        assert_eq!(snap.guides[0].target, Target::CanvasEdge);
        assert_eq!(snap.guides[0].pos, 1600.0);
    }

    #[test]
    fn nearest_wins_then_priority() {
        let doc = doc();
        let targets = Targets::new(&doc, &[]);
        // Left edge 2 px from the object's centre line (x = 200), right edge 5 px from the
        // canvas centre (x = 800): the nearer one wins.
        let snap = snap_rect(&targets, Rect::new(202.0, 600.0, 593.0, 10.0), 6.0);
        assert_eq!(snap.dx, -2.0);
        assert_eq!(snap.guides[0].target, Target::ObjectCentre);
        // Equal distance: the canvas centre beats an object edge.
        let snap = snap_rect(&targets, Rect::new(297.0, 600.0, 506.0, 10.0), 6.0);
        // left 297 → 300 (object edge, +3); right 803 → 800 (canvas centre, -3)
        assert_eq!(snap.dx, -3.0);
        assert_eq!(Snap::targets(&snap.guides), vec![Target::CanvasCentre]);
    }

    #[test]
    fn coinciding_lines_all_become_guides() {
        // An object whose right edge is the canvas centre: both lines show when a box snaps
        // there; one guide per position, named by the more important target.
        let mut doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let id = doc.scene().fresh_ids(1)[0];
        Command::Insert {
            nodes: vec![(0, Node::new(id, TestBox::boxed(700.0, 0.0, 100.0, 10.0)))],
            resources: Vec::new(),
        }
        .apply(&mut doc)
        .unwrap();
        let targets = Targets::new(&doc, &[]);
        let snap = snap_rect(&targets, Rect::new(798.0, 500.0, 100.0, 10.0), 6.0);
        assert_eq!(snap.dx, 2.0);
        let vertical: Vec<&Guide> = snap
            .guides
            .iter()
            .filter(|g| g.axis == Axis::Vertical)
            .collect();
        assert_eq!(vertical.len(), 1);
        assert_eq!(vertical[0].target, Target::CanvasCentre);
        assert_eq!(vertical[0].pos, 800.0);
    }

    #[test]
    fn dragged_objects_are_not_targets() {
        let doc = doc();
        let id = doc.scene().ids()[0];
        let targets = Targets::new(&doc, &[id]);
        let snap = snap_rect(&targets, Rect::new(103.0, 103.0, 200.0, 100.0), 6.0);
        assert_eq!((snap.dx, snap.dy), (0.0, 0.0));
        assert!(snap.guides.is_empty());
    }

    #[test]
    fn handles_snap_as_points_on_their_axes() {
        let doc = doc();
        let targets = Targets::new(&doc, &[]);
        let p = Point::new(797.0, 504.0);
        let both = snap_point(&targets, p, true, true, 6.0);
        assert_eq!((both.dx, both.dy), (3.0, -4.0));
        let x_only = snap_point(&targets, p, true, false, 6.0);
        assert_eq!((x_only.dx, x_only.dy), (3.0, 0.0));
        assert_eq!(x_only.guides.len(), 1);
    }
}
