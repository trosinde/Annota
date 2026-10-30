//! Pointer gestures on the canvas with the Select tool (spec editor-canvas: Object selection
//! and manipulation, Snapping and guides; mockups 02–06).
//!
//! A gesture is pressed, moved and released. On press it decides what it does, from the top:
//!
//! 1. a **handle** of the current selection under the pointer (nearest within reach, in
//!    screen pixels): resize, rotate or a type-specific handle. One object is edited through
//!    its own [`Annotation::drag_handle`]; several are edited as a transient [`Group`], so a
//!    multi-selection resizes and rotates as one box;
//! 2. an **object** under the pointer: it is selected per the modifiers (see `selection`) and
//!    the selection is moved. Pressing an object that is already part of a multi-selection
//!    keeps the selection (to move all of it) and, if the pointer is released without moving,
//!    selects only that object;
//! 3. **nothing**: a rubber band.
//!
//! While the pointer moves, the gesture keeps *preview copies* of the edited objects, computed
//! from their state at the press (handles get the start state, as `drag_handle` requires); the
//! document is untouched and the canvas draws the copies instead. Movement below
//! [`DRAG_THRESHOLD`] screen pixels is a click. On release the copies become **one**
//! `Command::Replace` — one undo step per gesture, however many motion events it had.

use crate::document::annotation::{box_handles, Annotation, Compass, HandleId, Modifiers};
use crate::document::geometry::{Arrange, Point, Rect};
use crate::document::scene::{Node, ObjectId};
use crate::document::Document;
use crate::render::Viewport;
use crate::tools::group::Group;

use super::ops::{Action, Edit, Subject};
use super::selection::{
    handle_at, is_selectable, selection_frame, SelectMode, Selection, SelectionFrame,
};
use super::snap::{self, Guide, Targets, SNAP_DISTANCE};

/// Pointer travel (screen pixels) below which a press and release is a click.
pub const DRAG_THRESHOLD: f64 = 3.0;
/// How far from an object (screen pixels) a press still hits it.
pub const HIT_TOLERANCE: f64 = 4.0;

/// Live values for the measurement chips.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feedback {
    None,
    /// Moving. `position` is the top-left of the (single) object's box (sidebar X/Y);
    /// `delta` the distance moved so far. `single` is false for groups and multi-selections,
    /// which show the distance instead (mockup 06).
    Move {
        position: Point,
        delta: Point,
        single: bool,
    },
    Resize {
        width: f64,
        height: f64,
    },
    /// Rotating; `stepped` while Shift snaps to 15° steps (mockup 03).
    Rotate {
        degrees: f64,
        stepped: bool,
    },
    Band(Rect),
}

#[derive(Clone, Debug)]
enum Kind {
    Move,
    Handle {
        handle: HandleId,
        /// Where the handle was drawn at the press (document space).
        anchor: Point,
        /// The selection as one box when several objects are selected.
        multi: Option<Group>,
    },
    Band {
        mode: SelectMode,
        base: Selection,
    },
}

/// One gesture, from press to release.
#[derive(Debug)]
pub struct Drag {
    kind: Kind,
    start_screen: Point,
    start_doc: Point,
    /// The edited objects as they were at the press.
    originals: Vec<(ObjectId, Box<dyn Annotation>)>,
    /// Box of the originals at the press.
    start_frame: Arrange,
    targets: Targets,
    moved: bool,
    reduce_to: Option<ObjectId>,
    preview: Vec<(ObjectId, Box<dyn Annotation>)>,
    guides: Vec<Guide>,
    feedback: Feedback,
    /// Box of a multi-selection while one of its handles is dragged (it turns with a
    /// rotation, unlike the union of the members' boxes).
    live_frame: Option<Arrange>,
}

/// What is under the pointer, for the cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hover {
    /// A handle; `rotation` is the selection box's rotation (resize cursors follow it).
    Handle {
        handle: HandleId,
        rotation: f64,
    },
    Object,
    Nothing,
}

/// What a press at `screen` would grab.
pub fn hover(doc: &Document, selection: &Selection, viewport: &Viewport, screen: Point) -> Hover {
    if let Some(frame) = selection_frame(doc.scene(), selection, &[]) {
        if let Some(handle) = handle_at(&frame, viewport, screen) {
            return Hover::Handle {
                handle: handle.id,
                rotation: frame.frame.rotation,
            };
        }
    }
    match hit(doc, viewport, screen) {
        Some(_) => Hover::Object,
        None => Hover::Nothing,
    }
}

/// CSS cursor name for a resize handle on a box rotated by `rotation` degrees.
pub fn resize_cursor(compass: Compass, rotation: f64) -> &'static str {
    const NAMES: [&str; 8] = [
        "e-resize",
        "se-resize",
        "s-resize",
        "sw-resize",
        "w-resize",
        "nw-resize",
        "n-resize",
        "ne-resize",
    ];
    let (dx, dy) = compass.direction();
    let angle = dy.atan2(dx).to_degrees() + rotation;
    let step = (angle / 45.0).round().rem_euclid(8.0) as usize;
    NAMES.get(step).copied().unwrap_or("default")
}

/// The topmost selectable object under `screen`.
fn hit(doc: &Document, viewport: &Viewport, screen: Point) -> Option<ObjectId> {
    let scene = doc.scene();
    let at = viewport.to_document(screen);
    scene
        .hit_test(at, HIT_TOLERANCE / viewport.zoom)
        .filter(|&id| {
            scene
                .get(id)
                .is_some_and(|n| is_selectable(n.object.as_ref()))
        })
}

fn union_frame(objects: &[(ObjectId, Box<dyn Annotation>)]) -> Arrange {
    match objects {
        [(_, one)] => one.arrange(),
        many => Arrange::new(
            many.iter()
                .fold(Rect::NONE, |acc, (_, o)| acc.union(&o.arrange().bounds())),
            0.0,
        ),
    }
}

impl Drag {
    /// A press at `screen` (view coordinates). Updates the selection for clicks and returns
    /// the gesture, or `None` if the press does nothing more (Ctrl+click that deselected).
    pub fn press(
        doc: &Document,
        selection: &mut Selection,
        viewport: &Viewport,
        screen: Point,
        modifiers: Modifiers,
    ) -> Option<Drag> {
        let scene = doc.scene();
        let mode = SelectMode::from_modifiers(modifiers);
        let start_doc = viewport.to_document(screen);
        let originals = |selection: &Selection| -> Vec<(ObjectId, Box<dyn Annotation>)> {
            selection
                .ids()
                .iter()
                .filter_map(|&id| Some((id, scene.get(id)?.object.clone())))
                .collect()
        };
        let new = |kind: Kind, originals: Vec<(ObjectId, Box<dyn Annotation>)>| {
            let exclude: Vec<ObjectId> = originals.iter().map(|(id, _)| *id).collect();
            Drag {
                kind,
                start_screen: screen,
                start_doc,
                start_frame: union_frame(&originals),
                targets: Targets::new(doc, &exclude),
                originals,
                moved: false,
                reduce_to: None,
                preview: Vec::new(),
                guides: Vec::new(),
                feedback: Feedback::None,
                live_frame: None,
            }
        };

        // 1. A handle of the selection (Ctrl+click is for toggling objects).
        if mode != SelectMode::Toggle {
            let handle = selection_frame(scene, selection, &[])
                .and_then(|frame| handle_at(&frame, viewport, screen));
            if let Some(handle) = handle {
                let originals = originals(selection);
                let multi = (originals.len() > 1).then(|| {
                    Group::new(
                        originals
                            .iter()
                            .map(|(id, o)| Node::new(*id, o.clone()))
                            .collect(),
                    )
                });
                // Where the handle is drawn, which includes the rotation handle's screen offset.
                let anchor =
                    viewport.to_document(handle.screen_position(viewport.zoom, viewport.origin));
                let kind = Kind::Handle {
                    handle: handle.id,
                    anchor,
                    multi,
                };
                return Some(new(kind, originals));
            }
        }

        // 2. An object.
        if let Some(id) = hit(doc, viewport, screen) {
            let mut reduce_to = None;
            if mode == SelectMode::Replace && selection.contains(id) {
                if selection.len() > 1 {
                    reduce_to = Some(id);
                }
            } else {
                selection.click(scene, Some(id), mode);
            }
            if !selection.contains(id) {
                return None;
            }
            let mut drag = new(Kind::Move, originals(selection));
            drag.reduce_to = reduce_to;
            return Some(drag);
        }

        // 3. Empty canvas: rubber band; a plain press deselects at once.
        let base = selection.clone();
        if mode == SelectMode::Replace {
            selection.clear();
        }
        Some(new(Kind::Band { mode, base }, Vec::new()))
    }

    /// Pointer moved to `screen`. `snapping` is the editor's switch (Ctrl+;); Alt suspends
    /// it for as long as it is held.
    pub fn motion(
        &mut self,
        doc: &Document,
        selection: &mut Selection,
        viewport: &Viewport,
        screen: Point,
        modifiers: Modifiers,
        snapping: bool,
    ) {
        if !self.moved {
            if screen.distance(self.start_screen) < DRAG_THRESHOLD {
                return;
            }
            self.moved = true;
        }
        let at = viewport.to_document(screen);
        let (dx, dy) = (at.x - self.start_doc.x, at.y - self.start_doc.y);
        let snapping = snapping && !modifiers.alt;
        let tolerance = SNAP_DISTANCE / viewport.zoom;
        self.guides.clear();
        match &self.kind {
            Kind::Move => {
                // Whole document pixels, then onto a guide.
                let (mut dx, mut dy) = (dx.round(), dy.round());
                if snapping {
                    let moved = self.start_frame.bounds().translate(dx, dy);
                    let s = snap::snap_rect(&self.targets, moved, tolerance);
                    dx += s.dx;
                    dy += s.dy;
                    self.guides = s.guides;
                }
                self.preview = self
                    .originals
                    .iter()
                    .map(|(id, o)| {
                        let mut o = o.clone();
                        o.translate(dx, dy);
                        (*id, o)
                    })
                    .collect();
                let single =
                    matches!(self.originals.as_slice(), [(_, o)] if !o.as_any().is::<Group>());
                let rect = self.start_frame.rect.translate(dx, dy);
                self.feedback = Feedback::Move {
                    position: Point::new(rect.x, rect.y),
                    delta: Point::new(dx, dy),
                    single,
                };
            }
            Kind::Handle {
                handle,
                anchor,
                multi,
            } => {
                let mut to = Point::new(anchor.x + dx, anchor.y + dy);
                let axes = match handle {
                    HandleId::Resize(compass) => {
                        let (x, y) = compass.direction();
                        Some((x != 0.0, y != 0.0))
                    }
                    HandleId::Custom(_) => Some((true, true)),
                    HandleId::Rotate => None,
                };
                // Positions only snap on an upright box: a rotated box's edges are not
                // parallel to the guides.
                if let (Some((x, y)), true, true) =
                    (axes, snapping, self.start_frame.rotation == 0.0)
                {
                    let s = snap::snap_point(&self.targets, to, x, y, tolerance);
                    to = Point::new(to.x + s.dx, to.y + s.dy);
                    self.guides = s.guides;
                }
                let frame = match multi {
                    Some(group) => {
                        let mut group = group.clone();
                        group.drag_handle(*handle, to, modifiers);
                        // The transient group's box, rotation included.
                        let frame = group.arrange();
                        self.live_frame = Some(frame);
                        self.preview = group
                            .children
                            .into_iter()
                            .map(|n| (n.id, n.object))
                            .collect();
                        frame
                    }
                    None => {
                        self.preview = self
                            .originals
                            .iter()
                            .map(|(id, o)| {
                                let mut o = o.clone();
                                o.drag_handle(*handle, to, modifiers);
                                (*id, o)
                            })
                            .collect();
                        union_frame(&self.preview)
                    }
                };
                self.feedback = match handle {
                    HandleId::Rotate => Feedback::Rotate {
                        degrees: frame.rotation,
                        stepped: modifiers.shift,
                    },
                    HandleId::Resize(_) => Feedback::Resize {
                        width: frame.rect.width,
                        height: frame.rect.height,
                    },
                    HandleId::Custom(_) => Feedback::None,
                };
            }
            Kind::Band { mode, base } => {
                let rect = Rect::from_points(self.start_doc, at);
                selection.band(doc.scene(), base, rect, *mode);
                self.feedback = Feedback::Band(rect);
            }
        }
    }

    /// Pointer released: the edit to execute (one undo step), if anything changed.
    pub fn release(self, doc: &Document, selection: &mut Selection) -> Option<Edit> {
        if !self.moved {
            if let Some(id) = self.reduce_to {
                selection.set(doc.scene(), [id]);
            }
            return None;
        }
        let action = match self.kind {
            Kind::Band { .. } => return None,
            Kind::Move => Action::Move,
            Kind::Handle { handle, .. } => match handle {
                HandleId::Resize(_) => Action::Resize,
                HandleId::Rotate => Action::Rotate,
                HandleId::Custom(_) => Action::Reshape,
            },
        };
        let unchanged = self
            .preview
            .iter()
            .zip(&self.originals)
            .all(|((_, p), (_, o))| p.to_json().ok() == o.to_json().ok());
        if self.preview.is_empty() || unchanged {
            return None;
        }
        let ids: Vec<ObjectId> = self.originals.iter().map(|(id, _)| *id).collect();
        Some(Edit {
            action,
            subject: Subject::of(doc.scene(), &ids),
            command: crate::document::command::Command::Replace {
                objects: self.preview,
            },
            select: None,
        })
    }

    /// Escape: drops the gesture; a rubber band gives back the selection it started from.
    pub fn cancel(self, selection: &mut Selection) {
        if let Kind::Band { base, .. } = self.kind {
            *selection = base;
        }
    }

    /// The selection frame to draw during the gesture: from the preview copies, and for a
    /// multi-selection whose handle is dragged, the turned or resized box of the whole.
    pub fn selection_frame(&self, doc: &Document, selection: &Selection) -> Option<SelectionFrame> {
        let mut frame = selection_frame(doc.scene(), selection, &self.preview)?;
        if let Some(live) = self.live_frame {
            frame.frame = live;
            frame.handles = box_handles(&live);
        }
        Some(frame)
    }

    /// The edited copies to draw instead of the originals.
    pub fn preview(&self) -> &[(ObjectId, Box<dyn Annotation>)] {
        &self.preview
    }

    /// Draw the originals faded underneath the copies (moves; mockups 02 and 06).
    pub fn shows_ghost(&self) -> bool {
        matches!(self.kind, Kind::Move) && self.moved
    }

    /// Guides to draw (snapping).
    pub fn guides(&self) -> &[Guide] {
        &self.guides
    }

    pub fn feedback(&self) -> Feedback {
        self.feedback
    }

    /// Whether the pointer has moved far enough to be a drag.
    pub fn is_dragging(&self) -> bool {
        self.moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::{Command, History};
    use crate::document::scene::tests::TestBox;
    use crate::editing::ops;
    use crate::editing::snap::Target;

    const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ctrl: false,
        alt: false,
    };
    const CTRL: Modifiers = Modifiers {
        shift: false,
        ctrl: true,
        alt: false,
    };
    const ALT: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: true,
    };

    /// 1600 × 1000 canvas at 50 % (mockup 02: "Fit · 50 %"), document (0, 0) at view (44, 44).
    fn viewport() -> Viewport {
        Viewport::new(0.5, Point::new(44.0, 44.0))
    }

    fn screen(doc: Point) -> Point {
        viewport().to_device(doc)
    }

    /// Objects as TestBoxes: `rects` in document space.
    fn doc_with(rects: &[Rect]) -> (Document, History, Vec<ObjectId>) {
        let mut doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let mut history = History::new();
        for r in rects {
            let id = doc.scene().fresh_ids(1)[0];
            let node = Node::new(id, TestBox::boxed(r.x, r.y, r.width, r.height));
            let at = doc.scene().len();
            history
                .execute(
                    &mut doc,
                    "Add",
                    Command::Insert {
                        nodes: vec![(at, node)],
                        resources: Vec::new(),
                    },
                )
                .unwrap();
        }
        let ids = doc.scene().ids();
        (doc, history, ids)
    }

    fn rect(doc: &Document, id: ObjectId) -> Rect {
        doc.scene().get(id).unwrap().object.arrange().rect
    }

    /// Press at `from`, move through `path` (all document points), release.
    fn gesture(
        doc: &Document,
        sel: &mut Selection,
        from: Point,
        path: &[Point],
        modifiers: Modifiers,
    ) -> (Option<Edit>, Vec<Feedback>, Vec<Vec<Guide>>) {
        let vp = viewport();
        let Some(mut drag) = Drag::press(doc, sel, &vp, screen(from), modifiers) else {
            return (None, Vec::new(), Vec::new());
        };
        let mut feedback = Vec::new();
        let mut guides = Vec::new();
        for p in path {
            drag.motion(doc, sel, &vp, screen(*p), modifiers, true);
            feedback.push(drag.feedback());
            guides.push(drag.guides().to_vec());
        }
        (drag.release(doc, sel), feedback, guides)
    }

    fn execute(doc: &mut Document, history: &mut History, sel: &mut Selection, edit: Edit) {
        history.execute(doc, "edit", edit.command).unwrap();
        sel.sync(doc.scene());
    }

    #[test]
    fn click_selects_and_empty_click_clears() {
        let (doc, _, ids) = doc_with(&[Rect::new(100.0, 100.0, 200.0, 100.0)]);
        let mut sel = Selection::new();
        let (edit, _, _) = gesture(&doc, &mut sel, Point::new(150.0, 150.0), &[], NONE);
        assert!(edit.is_none());
        assert_eq!(sel.ids(), &[ids[0]]);
        // A small wobble below the threshold is still a click.
        let (edit, _, _) = gesture(
            &doc,
            &mut sel,
            Point::new(150.0, 150.0),
            &[Point::new(152.0, 151.0)],
            NONE,
        );
        assert!(edit.is_none(), "1 screen px at 50 % is no move");
        gesture(&doc, &mut sel, Point::new(900.0, 900.0), &[], NONE);
        assert!(sel.is_empty());
        // Press within a few screen pixels of the object still hits it.
        gesture(&doc, &mut sel, Point::new(306.0, 150.0), &[], NONE);
        assert_eq!(sel.len(), 1);
    }

    #[test]
    fn a_drag_is_one_undoable_move() {
        // Spec "Undo move" through the gesture: many motion events, one undo step.
        let (mut doc, mut history, ids) = doc_with(&[Rect::new(100.0, 100.0, 200.0, 100.0)]);
        let mut sel = Selection::new();
        let path: Vec<Point> = (1..=40)
            .map(|k| Point::new(150.0 + 7.0 * f64::from(k), 150.0 + 3.0 * f64::from(k)))
            .collect();
        let (edit, feedback, _) = gesture(&doc, &mut sel, Point::new(150.0, 150.0), &path, NONE);
        let edit = edit.unwrap();
        assert_eq!(edit.action, ops::Action::Move);
        assert_eq!(edit.subject, ops::Subject::One("test-box"));
        assert_eq!(
            rect(&doc, ids[0]).x,
            100.0,
            "the document is untouched until the edit runs"
        );
        assert_eq!(
            feedback.last(),
            Some(&Feedback::Move {
                position: Point::new(380.0, 220.0),
                delta: Point::new(280.0, 120.0),
                single: true
            })
        );
        let before = history.undo_len();
        execute(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(history.undo_len(), before + 1);
        assert_eq!(rect(&doc, ids[0]), Rect::new(380.0, 220.0, 200.0, 100.0));
        history.undo(&mut doc).unwrap();
        assert_eq!(rect(&doc, ids[0]), Rect::new(100.0, 100.0, 200.0, 100.0));
    }

    #[test]
    fn centre_snap_and_alt() {
        // Spec "Centre snap" (mockup 02): dragged near the canvas centre, the object snaps
        // onto it and guides name the centre; Alt suspends snapping.
        let (doc, _, _) = doc_with(&[Rect::new(100.0, 100.0, 440.0, 66.0)]);
        let mut sel = Selection::new();
        // Box centre starts at (320, 133); move it to (808, 496): 8 and 4 document px off,
        // i.e. 4 and 2 screen px at 50 % — within 6 screen px.
        let to = Point::new(150.0 + 488.0, 120.0 + 363.0);
        let (edit, feedback, guides) =
            gesture(&doc, &mut sel, Point::new(150.0, 120.0), &[to], NONE);
        let Command::Replace { objects } = edit.unwrap().command else {
            panic!("replace expected");
        };
        assert_eq!(
            objects[0].1.arrange().rect.centre(),
            Point::new(800.0, 500.0)
        );
        let targets = Snap::targets(&guides[0]);
        assert_eq!(targets, vec![Target::CanvasCentre]);
        assert_eq!(guides[0].len(), 2, "one guide per axis");
        assert!(
            matches!(feedback[0], Feedback::Move { position, .. } if position == Point::new(580.0, 467.0))
        );
        // With Alt: exactly where the pointer put it, no guides.
        let (edit, _, guides) = gesture(&doc, &mut sel, Point::new(150.0, 120.0), &[to], ALT);
        let Command::Replace { objects } = edit.unwrap().command else {
            panic!("replace expected");
        };
        assert_eq!(
            objects[0].1.arrange().rect.centre(),
            Point::new(808.0, 496.0)
        );
        assert!(guides[0].is_empty());
    }

    use crate::editing::snap::Snap;

    #[test]
    fn snapping_switched_off() {
        let (doc, _, _) = doc_with(&[Rect::new(100.0, 100.0, 440.0, 66.0)]);
        let mut sel = Selection::new();
        let vp = viewport();
        let from = screen(Point::new(150.0, 120.0));
        let mut drag = Drag::press(&doc, &mut sel, &vp, from, NONE).unwrap();
        drag.motion(
            &doc,
            &mut sel,
            &vp,
            screen(Point::new(638.0, 483.0)),
            NONE,
            false,
        );
        assert!(drag.guides().is_empty());
    }

    #[test]
    fn group_moves_together() {
        // Spec "Group moves together": two objects grouped, the user drags one of them, both
        // move — as one undo step named after the group (mockup 06: "Undo Move Group").
        let (mut doc, mut history, ids) = doc_with(&[
            Rect::new(100.0, 100.0, 50.0, 50.0),
            Rect::new(400.0, 300.0, 50.0, 50.0),
        ]);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let edit = ops::group(&doc, &sel).unwrap();
        history.execute(&mut doc, "Group", edit.command).unwrap();
        sel.clear();
        // Drag the second object.
        let (edit, feedback, _) = gesture(
            &doc,
            &mut sel,
            Point::new(420.0, 320.0),
            &[Point::new(980.0, 650.0)],
            ALT,
        );
        let gid = sel.ids()[0];
        let edit = edit.unwrap();
        assert_eq!(edit.subject, ops::Subject::One("group"));
        assert!(matches!(
            feedback[0],
            Feedback::Move { delta, single: false, .. } if delta == Point::new(560.0, 330.0)
        ));
        execute(&mut doc, &mut history, &mut sel, edit);
        let group = doc.scene().get(gid).unwrap();
        let rects: Vec<Rect> = group
            .object
            .children()
            .iter()
            .map(|c| c.object.arrange().rect)
            .collect();
        assert_eq!(
            rects,
            vec![
                Rect::new(660.0, 430.0, 50.0, 50.0),
                Rect::new(960.0, 630.0, 50.0, 50.0)
            ]
        );
        history.undo(&mut doc).unwrap();
        let group = doc.scene().get(gid).unwrap();
        assert_eq!(
            group.object.children()[0].object.arrange().rect,
            Rect::new(100.0, 100.0, 50.0, 50.0)
        );
    }

    #[test]
    fn pressing_a_selected_object_moves_the_whole_selection() {
        let (doc, _, ids) = doc_with(&[
            Rect::new(100.0, 100.0, 50.0, 50.0),
            Rect::new(400.0, 300.0, 50.0, 50.0),
            Rect::new(700.0, 300.0, 50.0, 50.0),
        ]);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[0], ids[1]]);
        let (edit, _, _) = gesture(
            &doc,
            &mut sel,
            Point::new(120.0, 120.0),
            &[Point::new(160.0, 120.0)],
            ALT,
        );
        let Command::Replace { objects } = edit.unwrap().command else {
            panic!("replace expected");
        };
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[1].1.arrange().rect.x, 440.0);
        assert_eq!(sel.len(), 2);
        // Clicked without moving: only that object stays selected.
        gesture(&doc, &mut sel, Point::new(120.0, 120.0), &[], NONE);
        assert_eq!(sel.ids(), &[ids[0]]);
        // Shift+press adds and drags everything.
        let (edit, _, _) = gesture(
            &doc,
            &mut sel,
            Point::new(720.0, 320.0),
            &[Point::new(720.0, 340.0)],
            Modifiers { alt: true, ..SHIFT },
        );
        assert_eq!(sel.ids(), &[ids[0], ids[2]]);
        let Command::Replace { objects } = edit.unwrap().command else {
            panic!("replace expected");
        };
        assert_eq!(objects.len(), 2);
        // Ctrl+press on a selected object deselects it and starts nothing.
        let (edit, _, _) = gesture(
            &doc,
            &mut sel,
            Point::new(720.0, 320.0),
            &[Point::new(720.0, 400.0)],
            CTRL,
        );
        assert!(edit.is_none());
        assert_eq!(sel.ids(), &[ids[0]]);
    }

    #[test]
    fn resize_handle_resizes_and_snaps_to_the_canvas_edge() {
        let (doc, _, ids) = doc_with(&[Rect::new(1000.0, 400.0, 400.0, 200.0)]);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        // East handle at (1400, 500); drag to 1596: 4 doc px = 2 screen px from the edge.
        let (edit, feedback, guides) = gesture(
            &doc,
            &mut sel,
            Point::new(1400.0, 500.0),
            &[Point::new(1596.0, 520.0)],
            NONE,
        );
        let edit = edit.unwrap();
        assert_eq!(edit.action, ops::Action::Resize);
        assert_eq!(
            feedback[0],
            Feedback::Resize {
                width: 600.0,
                height: 200.0
            }
        );
        assert_eq!(guides[0][0].target, Target::CanvasEdge);
        let Command::Replace { objects } = edit.command else {
            panic!("replace expected");
        };
        assert_eq!(
            objects[0].1.arrange().rect,
            Rect::new(1000.0, 400.0, 600.0, 200.0)
        );
    }

    /// One image object (a type that rotates) at `r`.
    fn doc_with_image(r: Rect) -> (Document, Vec<ObjectId>) {
        let mut doc = Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap());
        let mut history = History::new();
        let pip = Bitmap::filled(38, 10, Rgba::WHITE).unwrap();
        let cmd = crate::tools::image::Image::insert_command(&doc, pip, r.centre());
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let id = doc.scene().ids()[0];
        let mut o = doc.scene().get(id).unwrap().object.clone();
        o.set_arrange(Arrange::new(r, 0.0));
        let place = Command::Replace {
            objects: vec![(id, o)],
        };
        history.execute(&mut doc, "Place", place).unwrap();
        (doc, vec![id])
    }

    #[test]
    fn rotate_snapping() {
        // Spec "Rotate snapping": with Shift the angle is a multiple of 15° (mockup 03:
        // live chip "−15° · Shift: 15° steps").
        let (doc, ids) = doc_with_image(Rect::new(1000.0, 200.0, 380.0, 100.0));
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let vp = viewport();
        let frame = selection_frame(doc.scene(), &sel, &[]).unwrap();
        let handle = frame
            .handles
            .iter()
            .find(|h| h.id == HandleId::Rotate)
            .unwrap();
        let grab = handle.screen_position(vp.zoom, vp.origin);
        let mut drag = Drag::press(&doc, &mut sel, &vp, grab, SHIFT).unwrap();
        // Pointer 17° counter-clockwise from straight up, around the centre (1190, 250).
        let c = Point::new(1190.0, 250.0);
        let a = (-17f64).to_radians();
        let to = vp.to_device(Point::new(c.x + 300.0 * a.sin(), c.y - 300.0 * a.cos()));
        drag.motion(&doc, &mut sel, &vp, to, SHIFT, true);
        assert_eq!(
            drag.feedback(),
            Feedback::Rotate {
                degrees: -15.0,
                stepped: true
            }
        );
        let edit = drag.release(&doc, &mut sel).unwrap();
        assert_eq!(edit.action, ops::Action::Rotate);
        let Command::Replace { objects } = edit.command else {
            panic!("replace expected");
        };
        let turned = objects[0].1.arrange();
        assert_eq!(turned.rotation, -15.0);
        assert_eq!(
            turned.rect,
            Rect::new(1000.0, 200.0, 380.0, 100.0),
            "turns in place"
        );
        // Without Shift the angle follows the pointer.
        let mut drag = Drag::press(&doc, &mut sel, &vp, grab, NONE).unwrap();
        drag.motion(&doc, &mut sel, &vp, to, NONE, true);
        let Feedback::Rotate { degrees, stepped } = drag.feedback() else {
            panic!("rotate feedback expected");
        };
        assert!((degrees + 17.0).abs() < 1e-6 && !stepped);
    }

    #[test]
    fn multi_selection_rotates_as_one_box() {
        let (doc, _, ids) = doc_with(&[
            Rect::new(100.0, 100.0, 100.0, 100.0),
            Rect::new(300.0, 100.0, 100.0, 100.0),
        ]);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let vp = viewport();
        // Union box (100, 100)–(400, 200), centre (250, 150); rotation handle above (250, 100).
        let grab = Point::new(vp.to_device(Point::new(250.0, 100.0)).x, 44.0 + 50.0 - 24.0);
        let mut drag = Drag::press(&doc, &mut sel, &vp, grab, NONE).unwrap();
        // A quarter turn: pointer right of the centre.
        drag.motion(
            &doc,
            &mut sel,
            &vp,
            screen(Point::new(600.0, 150.0)),
            NONE,
            true,
        );
        let frame = drag.selection_frame(&doc, &sel).unwrap();
        assert!((frame.frame.rotation - 90.0).abs() < 1e-6, "the box turns");
        let centres: Vec<Point> = drag
            .preview()
            .iter()
            .map(|(_, o)| o.arrange().rect.centre())
            .collect();
        // (150, 150) and (350, 150) turned a quarter around (250, 150).
        assert!((centres[0].x - 250.0).abs() < 1e-6 && (centres[0].y - 50.0).abs() < 1e-6);
        assert!((centres[1].x - 250.0).abs() < 1e-6 && (centres[1].y - 250.0).abs() < 1e-6);
        let edit = drag.release(&doc, &mut sel).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (ops::Action::Rotate, ops::Subject::Many(2))
        );
    }

    #[test]
    fn rubber_band_selects_live_and_escape_restores() {
        let (doc, _, ids) = doc_with(&[
            Rect::new(100.0, 100.0, 50.0, 50.0),
            Rect::new(300.0, 100.0, 50.0, 50.0),
            Rect::new(900.0, 100.0, 50.0, 50.0),
        ]);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[2]]);
        let vp = viewport();
        let mut drag =
            Drag::press(&doc, &mut sel, &vp, screen(Point::new(50.0, 50.0)), NONE).unwrap();
        assert!(sel.is_empty(), "a plain press on the canvas deselects");
        drag.motion(
            &doc,
            &mut sel,
            &vp,
            screen(Point::new(400.0, 200.0)),
            NONE,
            true,
        );
        assert_eq!(sel.ids(), &[ids[0], ids[1]]);
        assert_eq!(
            drag.feedback(),
            Feedback::Band(Rect::new(50.0, 50.0, 350.0, 150.0))
        );
        drag.cancel(&mut sel);
        assert_eq!(sel.ids(), &[ids[2]]);
        // Shift+band adds.
        let (edit, _, _) = gesture(
            &doc,
            &mut sel,
            Point::new(250.0, 50.0),
            &[Point::new(400.0, 200.0)],
            SHIFT,
        );
        assert!(edit.is_none(), "selection is not an undo step");
        assert_eq!(sel.ids(), &[ids[1], ids[2]]);
    }

    #[test]
    fn hover_and_cursors() {
        let (doc, _, ids) = doc_with(&[Rect::new(100.0, 100.0, 200.0, 100.0)]);
        let mut sel = Selection::new();
        let vp = viewport();
        assert_eq!(
            hover(&doc, &sel, &vp, screen(Point::new(150.0, 150.0))),
            Hover::Object
        );
        assert_eq!(
            hover(&doc, &sel, &vp, screen(Point::new(800.0, 800.0))),
            Hover::Nothing
        );
        sel.set(doc.scene(), ids);
        assert_eq!(
            hover(&doc, &sel, &vp, screen(Point::new(300.0, 200.0))),
            Hover::Handle {
                handle: HandleId::Resize(Compass::SE),
                rotation: 0.0
            }
        );
        assert_eq!(resize_cursor(Compass::SE, 0.0), "se-resize");
        assert_eq!(resize_cursor(Compass::N, 0.0), "n-resize");
        assert_eq!(resize_cursor(Compass::N, 90.0), "e-resize");
        assert_eq!(resize_cursor(Compass::E, -45.0), "ne-resize");
        assert_eq!(resize_cursor(Compass::W, 170.0), "e-resize");
    }
}
