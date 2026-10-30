//! One-shot editing operations on the selection: nudge (arrow keys), delete, duplicate,
//! stacking order and grouping (spec editor-canvas: Object selection and manipulation,
//! Z-order and grouping). Each returns an [`Edit`]: one `Command` (one undo step) plus what the
//! editor needs to name it ("Undo Move Group") and the selection afterwards.

use std::collections::HashSet;

use crate::document::annotation::Annotation;
use crate::document::bitmap::Bitmap;
use crate::document::command::Command;
use crate::document::geometry::{Arrange, Point};
use crate::document::scene::{Node, ObjectId, Scene};
use crate::document::Document;
use crate::tools::group::{group_command, ungroup_command, Group};
use crate::tools::image::Image;

use super::selection::Selection;

/// Arrow keys move the selection 1 px, with Shift 10 px (spec: Keyboard nudge).
pub const NUDGE: f64 = 1.0;
pub const NUDGE_LARGE: f64 = 10.0;
/// Copies made with Duplicate (Ctrl+D) are offset by this much, so they are visible.
pub const DUPLICATE_OFFSET: f64 = 10.0;

/// What an edit does; the editor turns it into the undo label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Move,
    Resize,
    Rotate,
    /// A type-specific handle (arrow end, callout tail …).
    Reshape,
    Delete,
    Duplicate,
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
    Group,
    Ungroup,
    /// A new object (an image dropped from the tray).
    Insert,
    // Operations on the whole document (`super::image_ops`).
    Crop,
    ResizeImage,
    RotateLeft,
    RotateRight,
    FlipHorizontally,
    FlipVertically,
    ExtendCanvas,
    CanvasBackground,
    CornerRadius,
    /// Flatten Selection (Ctrl+T); the subject is what was flattened.
    Flatten,
    /// Flatten All (Ctrl+Shift+T); the subject counts the objects.
    FlattenAll,
}

/// What the edit applies to, for the label ("Move Group", "Move 3 Objects").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    /// One object, by type tag.
    One(&'static str),
    Many(usize),
}

impl Subject {
    pub fn of(scene: &Scene, ids: &[ObjectId]) -> Subject {
        match ids {
            [one] => scene
                .get(*one)
                .map_or(Subject::Many(1), |n| Subject::One(n.object.type_name())),
            _ => Subject::Many(ids.len()),
        }
    }
}

/// One undo step.
#[derive(Debug)]
pub struct Edit {
    pub action: Action,
    pub subject: Subject,
    pub command: Command,
    /// The selection after the edit, if it changes (duplicates, a new group, ungrouped
    /// members); otherwise the editor keeps the selection and syncs it.
    pub select: Option<Vec<ObjectId>>,
}

/// Moves the selection by (dx, dy) — arrow keys: `NUDGE` or, with Shift, `NUDGE_LARGE`.
pub fn nudge(doc: &Document, selection: &Selection, dx: f64, dy: f64) -> Option<Edit> {
    if selection.is_empty() || (dx == 0.0 && dy == 0.0) {
        return None;
    }
    Some(Edit {
        action: Action::Move,
        subject: Subject::of(doc.scene(), selection.ids()),
        command: Command::translate(doc, selection.ids(), dx, dy),
        select: None,
    })
}

/// Delete (Del).
pub fn delete(doc: &Document, selection: &Selection) -> Option<Edit> {
    if selection.is_empty() {
        return None;
    }
    Some(Edit {
        action: Action::Delete,
        subject: Subject::of(doc.scene(), selection.ids()),
        command: Command::Remove {
            ids: selection.ids().to_vec(),
        },
        select: Some(Vec::new()),
    })
}

/// Duplicate (Ctrl+D): copies on top of everything, offset by [`DUPLICATE_OFFSET`], in the
/// originals' stacking order; the copies become the selection. Copies of groups get new ids
/// for their members too.
pub fn duplicate(doc: &Document, selection: &Selection) -> Option<Edit> {
    let scene = doc.scene();
    let originals: Vec<_> = selection
        .ids()
        .iter()
        .filter_map(|&id| scene.get(id))
        .collect();
    if originals.is_empty() {
        return None;
    }
    let mut next = scene.fresh_ids(1).first()?.0;
    let mut fresh = || {
        let id = ObjectId(next);
        next += 1;
        id
    };
    let mut nodes = Vec::with_capacity(originals.len());
    let mut copies = Vec::with_capacity(originals.len());
    for (k, original) in originals.iter().enumerate() {
        let mut node = (*original).clone();
        node.id = fresh();
        node.object.renumber_nested(&mut fresh);
        node.object.translate(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        copies.push(node.id);
        nodes.push((scene.len() + k, node));
    }
    Some(Edit {
        action: Action::Duplicate,
        subject: Subject::of(scene, selection.ids()),
        command: Command::Insert {
            nodes,
            resources: Vec::new(),
        },
        select: Some(copies),
    })
}

/// Stacking changes (context menu; Ctrl+] / Ctrl+[ one step, with Shift to the front/back).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacking {
    ToFront,
    Forward,
    Backward,
    ToBack,
}

/// The new order (bottom to top) after moving `selected` within `order`. Selected objects
/// keep their order among themselves; one step moves each past one unselected neighbour.
pub fn restacked(order: &[ObjectId], selected: &HashSet<ObjectId>, how: Stacking) -> Vec<ObjectId> {
    let is_sel = |id: &ObjectId| selected.contains(id);
    match how {
        Stacking::ToFront => order
            .iter()
            .filter(|id| !is_sel(id))
            .chain(order.iter().filter(|id| is_sel(id)))
            .copied()
            .collect(),
        Stacking::ToBack => order
            .iter()
            .filter(|id| is_sel(id))
            .chain(order.iter().filter(|id| !is_sel(id)))
            .copied()
            .collect(),
        Stacking::Forward => {
            let mut v = order.to_vec();
            // From the top down, so a run of selected objects moves up as a block.
            for i in (0..v.len().saturating_sub(1)).rev() {
                if is_sel(&v[i]) && !is_sel(&v[i + 1]) {
                    v.swap(i, i + 1);
                }
            }
            v
        }
        Stacking::Backward => {
            let mut v = order.to_vec();
            for i in 1..v.len() {
                if is_sel(&v[i]) && !is_sel(&v[i - 1]) {
                    v.swap(i, i - 1);
                }
            }
            v
        }
    }
}

/// A stacking change as an edit, or `None` if nothing would move.
pub fn restack(doc: &Document, selection: &Selection, how: Stacking) -> Option<Edit> {
    if selection.is_empty() {
        return None;
    }
    let order = doc.scene().ids();
    let selected: HashSet<ObjectId> = selection.ids().iter().copied().collect();
    let next = restacked(&order, &selected, how);
    if next == order {
        return None;
    }
    let action = match how {
        Stacking::ToFront => Action::BringToFront,
        Stacking::Forward => Action::BringForward,
        Stacking::Backward => Action::SendBackward,
        Stacking::ToBack => Action::SendToBack,
    };
    Some(Edit {
        action,
        subject: Subject::of(doc.scene(), selection.ids()),
        command: Command::Reorder { order: next },
        select: None,
    })
}

/// Group (Ctrl+G); the new group becomes the selection.
pub fn group(doc: &Document, selection: &Selection) -> Option<Edit> {
    let (id, command) = group_command(doc, selection.ids())?;
    Some(Edit {
        action: Action::Group,
        subject: Subject::of(doc.scene(), selection.ids()),
        command,
        select: Some(vec![id]),
    })
}

/// Ungroup (Ctrl+Shift+G); the former members become the selection.
pub fn ungroup(doc: &Document, selection: &Selection) -> Option<Edit> {
    let (members, command) = ungroup_command(doc, selection.ids())?;
    Some(Edit {
        action: Action::Ungroup,
        subject: Subject::of(doc.scene(), selection.ids()),
        command,
        select: Some(members),
    })
}

/// The Arrange entries of the sidebar (X, Y, W, H, Rotation): places the selection at `target`.
/// One object takes `target` as its own placement. Several are placed as one box, like their
/// selection handles do (a transient [`Group`]); that box has no orientation of its own, so for
/// several objects only position and size apply. A group or several objects with a member
/// turned against the box at an angle other than a multiple of 90° scale uniformly
/// ([`Group::uniform_target`]): the other side follows the one that was entered. `None` if
/// nothing would change.
pub fn arrange(doc: &Document, selection: &Selection, target: Arrange) -> Option<Edit> {
    let scene = doc.scene();
    let originals: Vec<(ObjectId, Box<dyn Annotation>)> = selection
        .ids()
        .iter()
        .filter_map(|&id| Some((id, scene.get(id)?.object.clone())))
        .collect();
    let (current, target, objects) = match originals.as_slice() {
        [] => return None,
        [(id, object)] => {
            // A group with a member turned at an odd angle only scales uniformly.
            let target = match object.as_any().downcast_ref::<Group>() {
                Some(group) => group.uniform_target(target),
                None => target,
            };
            let mut copy = object.clone();
            copy.set_arrange(target);
            (object.arrange(), target, vec![(*id, copy)])
        }
        _ => {
            let mut group = Group::new(
                originals
                    .iter()
                    .map(|(id, o)| Node::new(*id, o.clone()))
                    .collect(),
            );
            let current = group.arrange();
            let target = group.uniform_target(Arrange::new(target.rect, current.rotation));
            group.set_arrange(target);
            let objects = group
                .children
                .into_iter()
                .map(|n| (n.id, n.object))
                .collect();
            (current, target, objects)
        }
    };
    let differs = |a: f64, b: f64| (a - b).abs() > 1e-9;
    let action = if differs(current.rotation, target.rotation) {
        Action::Rotate
    } else if differs(current.rect.width, target.rect.width)
        || differs(current.rect.height, target.rect.height)
    {
        Action::Resize
    } else if differs(current.rect.x, target.rect.x) || differs(current.rect.y, target.rect.y) {
        Action::Move
    } else {
        return None;
    };
    Some(Edit {
        action,
        subject: Subject::of(scene, selection.ids()),
        command: Command::Replace { objects },
        select: None,
    })
}

/// Inserts `bitmap` as a new image object centred on `centre` (a tray thumbnail dropped on the
/// canvas, spec "Picture in picture"); the new object becomes the selection.
pub fn insert_image(doc: &Document, bitmap: Bitmap, centre: Point) -> Option<Edit> {
    let command = Image::insert_command(doc, bitmap, centre);
    let id = match &command {
        Command::Insert { nodes, .. } => nodes.first()?.1.id,
        _ => return None,
    };
    Some(Edit {
        action: Action::Insert,
        subject: Subject::One(Image::TYPE),
        command,
        select: Some(vec![id]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::History;
    use crate::document::geometry::Rect;
    use crate::document::scene::tests::TestBox;

    fn doc_with(n: usize) -> (Document, History, Vec<ObjectId>) {
        let mut doc = Document::new(Bitmap::filled(400, 300, Rgba::WHITE).unwrap());
        let mut history = History::new();
        for k in 0..n {
            let id = doc.scene().fresh_ids(1)[0];
            let node = Node::new(id, TestBox::boxed(k as f64 * 50.0, 10.0, 20.0, 20.0));
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

    fn apply(doc: &mut Document, history: &mut History, sel: &mut Selection, edit: Edit) {
        history.execute(doc, "edit", edit.command).unwrap();
        match edit.select {
            Some(ids) => sel.set(doc.scene(), ids),
            None => sel.sync(doc.scene()),
        }
    }

    fn rect(doc: &Document, id: ObjectId) -> Rect {
        doc.scene().get(id).unwrap().object.arrange().rect
    }

    #[test]
    fn keyboard_nudge() {
        // Spec "Keyboard nudge": Shift+→ moves the selection 10 px right, → 1 px; one undo
        // step each.
        let (mut doc, mut history, ids) = doc_with(2);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let edit = nudge(&doc, &sel, NUDGE_LARGE, 0.0).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::Move, Subject::Many(2))
        );
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(rect(&doc, ids[0]).x, 10.0);
        assert_eq!(rect(&doc, ids[1]).x, 60.0);
        let edit = nudge(&doc, &sel, 0.0, -NUDGE).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(rect(&doc, ids[0]).y, 9.0);
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(rect(&doc, ids[0]), Rect::new(0.0, 10.0, 20.0, 20.0));
        assert!(nudge(&doc, &Selection::new(), 1.0, 0.0).is_none());
    }

    #[test]
    fn delete_and_undo() {
        let (mut doc, mut history, ids) = doc_with(3);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[1]]);
        let edit = delete(&doc, &sel).unwrap();
        assert_eq!(edit.subject, Subject::One("test-box"));
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(doc.scene().ids(), vec![ids[0], ids[2]]);
        assert!(sel.is_empty());
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.scene().ids(), ids);
    }

    #[test]
    fn duplicate_puts_offset_copies_on_top() {
        let (mut doc, mut history, ids) = doc_with(3);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[2], ids[0]]);
        let edit = duplicate(&doc, &sel).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        let all = doc.scene().ids();
        assert_eq!(all.len(), 5);
        assert_eq!(&all[..3], &ids[..]);
        assert_eq!(sel.ids(), &all[3..]);
        assert_eq!(rect(&doc, all[3]), Rect::new(10.0, 20.0, 20.0, 20.0));
        assert_eq!(rect(&doc, all[4]), Rect::new(110.0, 20.0, 20.0, 20.0));
    }

    #[test]
    fn duplicating_a_group_gives_its_members_new_ids() {
        let (mut doc, mut history, ids) = doc_with(2);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let edit = group(&doc, &sel).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        let gid = sel.ids()[0];
        let edit = duplicate(&doc, &sel).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        let copy = sel.ids()[0];
        assert_ne!(copy, gid);
        let members = doc.scene().get(copy).unwrap().object.nested_ids();
        assert_eq!(members.len(), 2);
        assert!(members
            .iter()
            .all(|m| !ids.contains(m) && *m != gid && *m != copy));
        assert!(members.iter().all(|m| doc.scene().owner(*m) == Some(copy)));
        // Ungrouping the copy gives two more, independent objects.
        let edit = ungroup(&doc, &sel).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(sel.ids(), &members[..]);
        assert_eq!(doc.scene().len(), 3);
        let group = doc.scene().get(gid).unwrap();
        assert!(group.object.as_any().is::<Group>());
    }

    #[test]
    fn stacking_order() {
        let a = ObjectId(1);
        let b = ObjectId(2);
        let c = ObjectId(3);
        let d = ObjectId(4);
        let order = [a, b, c, d];
        let sel = |ids: &[ObjectId]| ids.iter().copied().collect::<HashSet<_>>();
        assert_eq!(
            restacked(&order, &sel(&[b]), Stacking::ToFront),
            vec![a, c, d, b]
        );
        assert_eq!(
            restacked(&order, &sel(&[c]), Stacking::ToBack),
            vec![c, a, b, d]
        );
        assert_eq!(
            restacked(&order, &sel(&[b]), Stacking::Forward),
            vec![a, c, b, d]
        );
        assert_eq!(
            restacked(&order, &sel(&[b]), Stacking::Backward),
            vec![b, a, c, d]
        );
        // A run moves as a block, keeping its order.
        assert_eq!(
            restacked(&order, &sel(&[b, c]), Stacking::Forward),
            vec![a, d, b, c]
        );
        assert_eq!(
            restacked(&order, &sel(&[a, c]), Stacking::ToFront),
            vec![b, d, a, c]
        );
        // At the top already: nothing moves.
        assert_eq!(
            restacked(&order, &sel(&[d]), Stacking::Forward),
            order.to_vec()
        );
        assert_eq!(
            restacked(&order, &sel(&[a]), Stacking::Backward),
            order.to_vec()
        );
    }

    #[test]
    fn restack_is_one_undo_step_and_noop_is_none() {
        let (mut doc, mut history, ids) = doc_with(3);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[0]]);
        assert!(restack(&doc, &sel, Stacking::ToBack).is_none());
        let edit = restack(&doc, &sel, Stacking::ToFront).unwrap();
        assert_eq!(edit.action, Action::BringToFront);
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(doc.scene().ids(), vec![ids[1], ids[2], ids[0]]);
        let edit = restack(&doc, &sel, Stacking::Backward).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(doc.scene().ids(), vec![ids[1], ids[0], ids[2]]);
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.scene().ids(), ids);
    }

    #[test]
    fn group_and_ungroup_select_the_result() {
        let (mut doc, mut history, ids) = doc_with(3);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[0], ids[1]]);
        let edit = group(&doc, &sel).unwrap();
        assert_eq!(edit.subject, Subject::Many(2));
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(sel.len(), 1);
        assert_eq!(Subject::of(doc.scene(), sel.ids()), Subject::One("group"));
        let edit = ungroup(&doc, &sel).unwrap();
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(sel.ids(), &ids[..2]);
        // Undo regroups; the selection drops the members, which are nested again.
        history.undo(&mut doc).unwrap();
        sel.sync(doc.scene());
        assert!(sel.is_empty());
    }

    #[test]
    fn arrange_places_one_object() {
        let (mut doc, mut history, ids) = doc_with(2);
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[1]]);
        let at = |x, y, w, h, r| Arrange::new(Rect::new(x, y, w, h), r);
        // Unchanged values are no edit.
        assert!(arrange(&doc, &sel, at(50.0, 10.0, 20.0, 20.0, 0.0)).is_none());
        let edit = arrange(&doc, &sel, at(70.0, 15.0, 20.0, 20.0, 0.0)).unwrap();
        assert_eq!(edit.action, Action::Move);
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(rect(&doc, ids[1]), Rect::new(70.0, 15.0, 20.0, 20.0));
        let edit = arrange(&doc, &sel, at(70.0, 15.0, 40.0, 20.0, 0.0)).unwrap();
        assert_eq!(edit.action, Action::Resize);
        apply(&mut doc, &mut history, &mut sel, edit);
        let edit = arrange(&doc, &sel, at(70.0, 15.0, 40.0, 20.0, 30.0)).unwrap();
        assert_eq!(edit.action, Action::Rotate);
        // (The test box has no rotation of its own; images and groups keep it.)
        apply(&mut doc, &mut history, &mut sel, edit);
        // One undo step each.
        for _ in 0..3 {
            history.undo(&mut doc).unwrap();
        }
        assert_eq!(rect(&doc, ids[1]), Rect::new(50.0, 10.0, 20.0, 20.0));
        assert!(arrange(&doc, &Selection::new(), at(0.0, 0.0, 1.0, 1.0, 0.0)).is_none());
    }

    #[test]
    fn arrange_places_several_as_one_box() {
        // Boxes at x 0 and 50 (20 wide): one box 0..70; moving it to x 100 moves both, and
        // doubling its width doubles their sizes and gaps. Rotation does not apply.
        let (mut doc, mut history, ids) = doc_with(2);
        let mut sel = Selection::new();
        sel.set(doc.scene(), ids.clone());
        let edit = arrange(
            &doc,
            &sel,
            Arrange::new(Rect::new(100.0, 10.0, 140.0, 20.0), 45.0),
        )
        .unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::Resize, Subject::Many(2))
        );
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(rect(&doc, ids[0]), Rect::new(100.0, 10.0, 40.0, 20.0));
        assert_eq!(rect(&doc, ids[1]), Rect::new(200.0, 10.0, 40.0, 20.0));
        assert!(doc
            .scene()
            .nodes()
            .iter()
            .all(|n| n.object.arrange().rotation == 0.0));
    }

    #[test]
    fn arranging_a_group_with_a_turned_member_scales_uniformly() {
        // Phase-2a review, sidebar path: typing a width for a group (or several objects) with
        // a member at 45° to the box changed the height in a way nobody asked for and missed
        // the width. Now the box scales uniformly: exactly the typed width, the ratio kept.
        let (mut doc, mut history, _) = doc_with(0);
        for (x, rotation) in [(20.0, 0.0), (200.0, 45.0)] {
            let pip = Bitmap::filled(40, 20, Rgba::WHITE).unwrap();
            let edit = insert_image(&doc, pip, Point::new(x, 100.0)).unwrap();
            history.execute(&mut doc, "Insert", edit.command).unwrap();
            let id = *doc.scene().ids().last().unwrap();
            let mut o = doc.scene().get(id).unwrap().object.clone();
            let mut a = o.arrange();
            a.rotation = rotation;
            o.set_arrange(a);
            let place = Command::Replace {
                objects: vec![(id, o)],
            };
            history.execute(&mut doc, "Turn", place).unwrap();
        }
        let ids = doc.scene().ids();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
        for grouped in [false, true] {
            let mut sel = Selection::new();
            sel.set(doc.scene(), ids.clone());
            if grouped {
                let edit = group(&doc, &sel).unwrap();
                apply(&mut doc, &mut history, &mut sel, edit);
            }
            let start = selection_box(&doc, &sel);
            let mut target = start;
            target.rect.width *= 1.5;
            let edit = arrange(&doc, &sel, target).unwrap();
            assert_eq!(edit.action, Action::Resize);
            apply(&mut doc, &mut history, &mut sel, edit);
            let after = selection_box(&doc, &sel);
            assert!(close(after.rect.width, start.rect.width * 1.5), "{after:?}");
            assert!(
                close(after.rect.height, start.rect.height * 1.5),
                "{after:?}"
            );
            assert!(close(after.rect.x, start.rect.x) && close(after.rect.y, start.rect.y));
            history.undo(&mut doc).unwrap();
            sel.sync(doc.scene());
            if grouped {
                history.undo(&mut doc).unwrap();
            }
        }
    }

    /// The box the sidebar's Arrange entries show: one object's own, several as one box.
    fn selection_box(doc: &Document, sel: &Selection) -> Arrange {
        let objects: Vec<Node> = sel
            .ids()
            .iter()
            .map(|&id| Node::new(id, doc.scene().get(id).unwrap().object.clone()))
            .collect();
        match objects.as_slice() {
            [one] => one.object.arrange(),
            _ => Group::new(objects).arrange(),
        }
    }

    #[test]
    fn inserted_image_is_selected_and_undoable() {
        let (mut doc, mut history, _) = doc_with(1);
        let mut sel = Selection::new();
        let pip = Bitmap::filled(40, 30, Rgba::WHITE).unwrap();
        let edit = insert_image(&doc, pip, Point::new(200.0, 150.0)).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::Insert, Subject::One(Image::TYPE))
        );
        apply(&mut doc, &mut history, &mut sel, edit);
        assert_eq!(doc.scene().len(), 2);
        let id = sel.ids()[0];
        assert_eq!(doc.scene().ids()[1], id);
        assert_eq!(rect(&doc, id).centre(), Point::new(200.0, 150.0));
        assert_eq!(doc.resources().len(), 1);
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.scene().len(), 1);
    }
}
