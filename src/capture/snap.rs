//! Snapping the selection to window edges (spec: Region capture, "snap to window edges when
//! the pointer is within 8 px").
//!
//! Wayland gives applications no access to other windows' geometry, and Annota only talks to
//! the screen through xdg-desktop-portal (CLAUDE.md §2). So window edges are found in the frozen
//! screenshot itself: an edge is a long, straight run of strong contrast between two adjacent
//! pixel columns (or rows) — window borders, panels and dialog frames, but not text, whose
//! contrast runs are short. This works identically on Wayland and X11.

use super::frame::Frame;

/// Snap distance in image px (spec).
pub const SNAP_DISTANCE: i32 = 8;
/// Minimum luma difference between neighbouring pixels to count as an edge.
const CONTRAST: i32 = 24;
/// Minimum length of a straight edge. Shorter runs (text, icons) never snap.
const MIN_RUN: i32 = 48;
/// Pixels of missing contrast tolerated inside one run (anti-aliasing, rounded corners).
const MAX_GAP: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// An edge between two columns, at x = `pos`.
    Vertical,
    /// An edge between two rows, at y = `pos`.
    Horizontal,
}

/// A detected straight edge from `start` to `end` along its axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub axis: Axis,
    pub pos: i32,
    pub start: i32,
    pub end: i32,
}

impl Edge {
    pub fn len(&self) -> i32 {
        self.end - self.start
    }
}

/// Result of snapping one point.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snap {
    pub point: (i32, i32),
    pub edges: Vec<Edge>,
}

/// Snaps `p` (image px) to the nearest vertical and horizontal edge within 8 px.
pub fn snap_point(frame: &Frame, p: (i32, i32)) -> Snap {
    let mut snap = Snap {
        point: p,
        edges: Vec::new(),
    };
    if let Some(edge) = nearest_edge(frame, Axis::Vertical, p.0, p.1) {
        snap.point.0 = edge.pos;
        snap.edges.push(edge);
    }
    if let Some(edge) = nearest_edge(frame, Axis::Horizontal, p.1, p.0) {
        snap.point.1 = edge.pos;
        snap.edges.push(edge);
    }
    snap
}

/// Luma difference across the boundary at `pos` (between `pos - 1` and `pos`), sampled at
/// `along` on the other axis.
fn contrast(frame: &Frame, axis: Axis, pos: i32, along: i32) -> i32 {
    let (a, b) = match axis {
        Axis::Vertical => (frame.luma(pos - 1, along), frame.luma(pos, along)),
        Axis::Horizontal => (frame.luma(along, pos - 1), frame.luma(along, pos)),
    };
    (i32::from(a) - i32::from(b)).abs()
}

fn nearest_edge(frame: &Frame, axis: Axis, pos: i32, along: i32) -> Option<Edge> {
    let (limit, along_limit) = match axis {
        Axis::Vertical => (frame.width(), frame.height()),
        Axis::Horizontal => (frame.height(), frame.width()),
    };
    // Nearest boundary wins; at equal distance, the longer edge.
    (0..=SNAP_DISTANCE).find_map(|offset| {
        [pos - offset, pos + offset]
            .into_iter()
            .filter(|&candidate| candidate > 0 && candidate < limit)
            .filter_map(|candidate| edge_through(frame, axis, candidate, along, along_limit))
            .max_by_key(Edge::len)
    })
}

/// The straight edge at boundary `pos` passing within 8 px of `along`, if long enough.
fn edge_through(frame: &Frame, axis: Axis, pos: i32, along: i32, along_limit: i32) -> Option<Edge> {
    let seed = (0..=SNAP_DISTANCE)
        .flat_map(|d| [along - d, along + d])
        .find(|&a| a >= 0 && a < along_limit && contrast(frame, axis, pos, a) >= CONTRAST)?;
    let extend = |step: i32| -> i32 {
        let mut last = seed;
        let mut cursor = seed + step;
        let mut gap = 0;
        while cursor >= 0 && cursor < along_limit {
            if contrast(frame, axis, pos, cursor) >= CONTRAST {
                last = cursor;
                gap = 0;
            } else {
                gap += 1;
                if gap > MAX_GAP {
                    break;
                }
            }
            cursor += step;
        }
        last
    };
    let start = extend(-1);
    let end = extend(1) + 1;
    (end - start >= MIN_RUN).then_some(Edge {
        axis,
        pos,
        start,
        end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::frame::Rgb;

    /// Grey desktop with a white "window" at x 200..600, y 100..400.
    fn desktop() -> Frame {
        let mut frame = Frame::filled(
            1000,
            700,
            Rgb {
                r: 0x60,
                g: 0x60,
                b: 0x68,
            },
        )
        .unwrap();
        frame
            .draw_with(|cr| {
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.rectangle(200.0, 100.0, 400.0, 300.0);
                cr.fill()?;
                // A line of "text": short dark runs that must not snap.
                cr.set_source_rgb(0.1, 0.1, 0.1);
                cr.rectangle(260.0, 200.0, 120.0, 10.0);
                cr.fill()
            })
            .unwrap();
        frame
    }

    #[test]
    fn snaps_within_8_px() {
        let frame = desktop();
        let snap = snap_point(&frame, (205, 300));
        assert_eq!(snap.point, (200, 300));
        assert!(!snap.edges.is_empty());
        let snap = snap_point(&frame, (192, 300));
        assert_eq!(snap.point, (200, 300));
        let snap = snap_point(&frame, (607, 300));
        assert_eq!(snap.point, (600, 300));
    }

    #[test]
    fn no_snap_beyond_8_px() {
        let frame = desktop();
        let snap = snap_point(&frame, (180, 300));
        assert_eq!(snap.point, (180, 300));
        assert!(snap.edges.is_empty());
    }

    #[test]
    fn corner_snaps_on_both_axes() {
        let frame = desktop();
        let snap = snap_point(&frame, (604, 405));
        assert_eq!(snap.point, (600, 400));
        assert_eq!(snap.edges.len(), 2);
        let vertical = snap.edges[0];
        assert_eq!(vertical.axis, Axis::Vertical);
        assert_eq!((vertical.start, vertical.end), (100, 400));
    }

    #[test]
    fn text_does_not_snap() {
        let frame = desktop();
        // Next to the left end of the text bar (x = 260), inside the window.
        let snap = snap_point(&frame, (263, 205));
        assert_eq!(snap.point.0, 263);
    }
}
