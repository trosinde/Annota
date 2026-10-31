//! One-shot editing operations on the selection: nudge (arrow keys), delete, duplicate, paste
//! (add-export-share task 2.4a), stacking order and grouping (spec editor-canvas: Object
//! selection and manipulation, Z-order and grouping). Each returns an [`Edit`]: one `Command` (one undo step) plus what the
//! editor needs to name it ("Undo Move Group") and the selection afterwards.

use std::collections::HashSet;

use crate::document::annotation::{Annotation, Property, PropertyValue};
use crate::document::bitmap::Bitmap;
use crate::document::clipboard::Clip;
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
/// Copies made with Duplicate (Ctrl+D) are offset by this much, so they are visible; so are
/// pasted objects ([`paste`]).
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
    /// Objects pasted from Annota's object clipboard (Ctrl+V).
    Paste,
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
    Group,
    Ungroup,
    /// A new object (an image dropped from the tray).
    Insert,
    /// A new object drawn with a tool (add-core-annotation-tools).
    Add,
    /// A property changed in the sidebar (colour, width, shadow …).
    Style,
    /// A quick style applied (add-style-system): several properties, one step.
    ApplyStyle,
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
    /// "Convert to Solid" before an export (add-export-share mockup 10); the subject counts
    /// the redactions converted (`super::redactions`).
    MakeSolid,
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

/// Delete (Del). Step markers that follow a deleted one in its sequence are renumbered in the
/// same undo step (spec annotation-tools: "Renumber on delete"; `super::steps`).
pub fn delete(doc: &Document, selection: &Selection) -> Option<Edit> {
    if selection.is_empty() {
        return None;
    }
    let remove = Command::Remove {
        ids: selection.ids().to_vec(),
    };
    let renumbered = super::steps::renumbered(doc.scene(), selection.ids());
    let command = if renumbered.is_empty() {
        remove
    } else {
        Command::Batch(vec![
            remove,
            Command::Replace {
                objects: renumbered,
            },
        ])
    };
    Some(Edit {
        action: Action::Delete,
        subject: Subject::of(doc.scene(), selection.ids()),
        command,
        select: Some(Vec::new()),
    })
}

/// A new object, drawn with a tool, on top of everything; it becomes the selection.
pub fn add(doc: &Document, object: Box<dyn Annotation>) -> Option<Edit> {
    let id = *doc.scene().fresh_ids(1).first()?;
    let subject = Subject::One(object.type_name());
    Some(Edit {
        action: Action::Add,
        subject,
        command: Command::Insert {
            nodes: vec![(doc.scene().len(), Node::new(id, object))],
            resources: Vec::new(),
        },
        select: Some(vec![id]),
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

/// Paste (Ctrl+V; spec export-share "Object copy and paste"): copied objects (`clip`, read from
/// the clipboard — nothing refers back to the document they came from) on top of everything,
/// offset by [`DUPLICATE_OFFSET`] like Duplicate, in their stacking order, as one undo step;
/// they become the selection. They get ids of this document (group members too), and their
/// embedded images get ids of this document as well — unless the document already has that
/// very picture (pasting into the document they were copied from), which is then shared, as
/// a duplicate shares it. Objects of a newer Annota raise the document's minor format version
/// in the same step (`Command::SetFormatMinor`), so it saves and reopens with them.
pub fn paste(doc: &Document, clip: &Clip) -> Option<Edit> {
    if clip.is_empty() {
        return None;
    }
    let scene = doc.scene();
    let mut next_image = doc
        .resources()
        .ids()
        .filter_map(crate::document::parse_resource_id)
        .max()
        .unwrap_or(0);
    let mut renamed: Vec<(String, String)> = Vec::with_capacity(clip.resources.len());
    let mut resources = Vec::new();
    for (id, bitmap) in &clip.resources {
        let target = match same_image(doc, id, bitmap) {
            Some(existing) => existing,
            None => {
                next_image = next_image.checked_add(1)?;
                let fresh = format!("images/{next_image}.png");
                resources.push((fresh.clone(), bitmap.clone()));
                fresh
            }
        };
        renamed.push((id.clone(), target));
    }
    let rename = |old: &str| {
        renamed
            .iter()
            .find(|(from, _)| from == old)
            .map(|(_, to)| to.clone())
    };

    let mut next = scene.fresh_ids(1).first()?.0;
    let mut fresh = || {
        let id = ObjectId(next);
        next += 1;
        id
    };
    let mut nodes = Vec::with_capacity(clip.len());
    let mut pasted = Vec::with_capacity(clip.len());
    for (k, original) in clip.nodes().iter().enumerate() {
        let mut node = original.clone();
        node.id = fresh();
        node.object.renumber_nested(&mut fresh);
        node.object.rename_resources(&rename);
        node.object.translate(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        pasted.push(node.id);
        nodes.push((scene.len() + k, node));
    }
    let subject = match clip.nodes() {
        [one] => Subject::One(one.object.type_name()),
        all => Subject::Many(all.len()),
    };
    let insert = Command::Insert { nodes, resources };
    let command = if clip.format_minor > doc.format_minor {
        Command::Batch(vec![insert, Command::SetFormatMinor(clip.format_minor)])
    } else {
        insert
    };
    Some(Edit {
        action: Action::Paste,
        subject,
        command,
        select: Some(pasted),
    })
}

/// The id under which `doc` already has the picture `bitmap` (same size, same PNG encoding),
/// trying `id` first.
fn same_image(doc: &Document, id: &str, bitmap: &Bitmap) -> Option<String> {
    let png = bitmap.png().ok()?;
    let same = |candidate: &Bitmap| {
        candidate.ptr_eq(bitmap)
            || (candidate.width() == bitmap.width()
                && candidate.height() == bitmap.height()
                && candidate.png().is_ok_and(|p| p == png))
    };
    if doc.resources().get(id).is_some_and(same) {
        return Some(id.to_string());
    }
    doc.resources()
        .ids()
        .find(|other| doc.resources().get(other).is_some_and(same))
        .map(str::to_string)
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

/// The current value of property `key` of `object`, if it has one.
pub fn property(object: &dyn Annotation, key: &str) -> Option<PropertyValue> {
    object
        .properties()
        .into_iter()
        .find(|p| p.key == key)
        .map(|p| p.value)
}

/// Copies of the objects `ids` with property `key` set to `value`: only the objects that have
/// the property, accept the value and actually change (compared as saved). Empty if nothing
/// would change — the sidebar's guard against phantom undo steps.
///
/// `counting` (how step markers count) changes every marker of the selected markers'
/// sequences ([`super::steps::recounted`]), not just the selected ones.
pub fn restyled(
    doc: &Document,
    ids: &[ObjectId],
    key: &str,
    value: &PropertyValue,
) -> Vec<(ObjectId, Box<dyn Annotation>)> {
    let scene = doc.scene();
    if key == "counting" {
        let Some(counting) = (match value {
            PropertyValue::Choice(k) => crate::tools::step::Counting::from_key(k),
            _ => None,
        }) else {
            return Vec::new();
        };
        let sequences = super::steps::sequences_of(scene, ids);
        return super::steps::recounted(scene, &sequences, counting);
    }
    ids.iter()
        .filter_map(|&id| {
            let original = scene.get(id)?.object.as_ref();
            let mut copy = original.clone_box();
            copy.set_property(key, value).ok()?;
            let changed = copy.to_json().ok()? != original.to_json().ok()?;
            changed.then_some((id, copy))
        })
        .collect()
}

/// The selected objects with all of `values` set (a quick style): copies of the objects that
/// really change. A value an object refuses is skipped for that object; the others still
/// apply.
pub fn restyled_all(
    doc: &Document,
    ids: &[ObjectId],
    values: &[Property],
) -> Vec<(ObjectId, Box<dyn Annotation>)> {
    let scene = doc.scene();
    ids.iter()
        .filter_map(|&id| {
            let original = scene.get(id)?.object.as_ref();
            let mut copy = original.clone_box();
            for p in values {
                let _ = copy.set_property(p.key, &p.value);
            }
            let changed = copy.to_json().ok()? != original.to_json().ok()?;
            changed.then_some((id, copy))
        })
        .collect()
}

/// Applies a quick style to the selection: every property at once, **one** undo step
/// ("Apply Style to Arrow"); `None` if no object changes (the phantom-edit guard of
/// [`set_property`], for a style that is already in use).
pub fn set_properties(doc: &Document, ids: &[ObjectId], values: &[Property]) -> Option<Edit> {
    let objects = restyled_all(doc, ids, values);
    if objects.is_empty() {
        return None;
    }
    let changed: Vec<ObjectId> = objects.iter().map(|(id, _)| *id).collect();
    Some(Edit {
        action: Action::ApplyStyle,
        subject: Subject::of(doc.scene(), &changed),
        command: Command::Replace { objects },
        select: None,
    })
}

/// A property edit in the sidebar as one undo step ("Change Arrow"); `None` if nothing
/// changes (no object has the property, the value is refused, or it is the current value).
pub fn set_property(
    doc: &Document,
    ids: &[ObjectId],
    key: &str,
    value: &PropertyValue,
) -> Option<Edit> {
    let objects = restyled(doc, ids, key, value);
    if objects.is_empty() {
        return None;
    }
    let changed: Vec<ObjectId> = objects.iter().map(|(id, _)| *id).collect();
    Some(Edit {
        action: Action::Style,
        subject: Subject::of(doc.scene(), &changed),
        command: Command::Replace { objects },
        select: None,
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

    #[test]
    fn property_edits_are_one_step_and_only_real_changes() {
        use crate::document::color::palette;
        use crate::tools::arrow::Arrow;
        use crate::tools::highlighter::Highlighter;
        let mut doc = Document::new(Bitmap::filled(400, 300, Rgba::WHITE).unwrap());
        let mut history = History::new();
        let a = Arrow::new(Point::new(10.0, 10.0), Point::new(100.0, 10.0));
        let b = Arrow::new(Point::new(10.0, 50.0), Point::new(100.0, 50.0));
        let h = Highlighter::new(Rect::new(10.0, 100.0, 50.0, 20.0));
        for object in [Box::new(a) as Box<dyn Annotation>, Box::new(b), Box::new(h)] {
            let edit = add(&doc, object).unwrap();
            history.execute(&mut doc, "Add", edit.command).unwrap();
        }
        let ids = doc.scene().ids();
        let azure = PropertyValue::Color(palette::AZURE);
        // Both arrows, one step, "Change 2 Objects".
        let edit = set_property(&doc, &ids[..2], "color", &azure).unwrap();
        assert_eq!(edit.action, Action::Style);
        assert_eq!(edit.subject, Subject::Many(2));
        history.execute(&mut doc, "Change", edit.command).unwrap();
        for id in &ids[..2] {
            assert_eq!(
                property(doc.scene().get(*id).unwrap().object.as_ref(), "color"),
                Some(azure.clone())
            );
        }
        // The same value again, a refused value, or a property the object does not have: no
        // step (the phantom-edit guard).
        assert!(set_property(&doc, &ids[..2], "color", &azure).is_none());
        assert!(set_property(&doc, &ids[..1], "width", &PropertyValue::Number(99.0)).is_none());
        assert!(set_property(&doc, &ids[2..], "shadow", &PropertyValue::Bool(true)).is_none());
        // A mixed selection changes the objects that have the property.
        let edit = set_property(&doc, &ids, "opacity", &PropertyValue::Number(0.5)).unwrap();
        assert_eq!(edit.subject, Subject::Many(3));
        history.undo(&mut doc).unwrap();
        assert_eq!(
            property(doc.scene().get(ids[0]).unwrap().object.as_ref(), "color"),
            Some(PropertyValue::Color(palette::CORAL)),
            "one undo restores the colour"
        );
    }

    #[test]
    fn a_quick_style_is_one_step_on_every_selected_object() {
        use crate::document::color::palette;
        use crate::tools::arrow::Arrow;
        let mut doc = Document::new(Bitmap::filled(400, 300, Rgba::WHITE).unwrap());
        let mut history = History::new();
        for y in [10.0, 50.0] {
            let a = Arrow::new(Point::new(10.0, y), Point::new(100.0, y));
            let edit = add(&doc, Box::new(a)).unwrap();
            history.execute(&mut doc, "Add", edit.command).unwrap();
        }
        let ids = doc.scene().ids();
        let style = [
            Property::new("color", PropertyValue::Color(palette::TEAL)),
            Property::new("width", PropertyValue::Number(12.0)),
            // Refused by an arrow: skipped, the rest applies.
            Property::new("width_of_nothing", PropertyValue::Number(1.0)),
        ];
        let edit = set_properties(&doc, &ids, &style).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::ApplyStyle, Subject::Many(2))
        );
        history.execute(&mut doc, "Apply", edit.command).unwrap();
        for id in &ids {
            let object = doc.scene().get(*id).unwrap().object.as_ref();
            assert_eq!(property(object, "color"), Some(style[0].value.clone()));
            assert_eq!(property(object, "width"), Some(style[1].value.clone()));
        }
        // Already in use: no step.
        assert!(set_properties(&doc, &ids, &style).is_none());
        history.undo(&mut doc).unwrap();
        let first = doc.scene().get(ids[0]).unwrap().object.as_ref();
        assert_eq!(
            property(first, "color"),
            Some(PropertyValue::Color(palette::CORAL))
        );
        assert_eq!(property(first, "width"), Some(PropertyValue::Number(8.0)));
    }

    // ---- paste (add-export-share task 2.4a; spec export-share "Object copy and paste") ----

    mod paste_tests {
        use super::*;
        use crate::document::archive;
        use crate::document::clipboard::{self, Clip};
        use crate::document::color::palette;
        use crate::document::registry::{node_from_json, node_to_json, UnknownObject};
        use crate::tools::arrow::Arrow;
        use serde_json::json;

        /// A document with an arrow (coral, 10,20 → 90,60).
        fn arrow_doc() -> (Document, History, ObjectId) {
            let mut doc = Document::new(Bitmap::filled(400, 300, Rgba::WHITE).unwrap());
            let mut history = History::new();
            let mut arrow = Arrow::new(Point::new(10.0, 20.0), Point::new(90.0, 60.0));
            arrow.color = palette::CORAL;
            let edit = add(&doc, Box::new(arrow)).unwrap();
            history.execute(&mut doc, "Add", edit.command).unwrap();
            let id = doc.scene().ids()[0];
            (doc, history, id)
        }

        /// What Ctrl+C puts on the clipboard and Ctrl+V reads back: bytes in between.
        fn copied(doc: &Document, ids: &[ObjectId]) -> Clip {
            clipboard::decode(&clipboard::encode(doc, ids).unwrap().unwrap()).unwrap()
        }

        fn arrow(doc: &Document, id: ObjectId) -> Arrow {
            doc.scene()
                .get(id)
                .unwrap()
                .object
                .as_any()
                .downcast_ref::<Arrow>()
                .unwrap()
                .clone()
        }

        #[test]
        fn copy_and_paste_into_the_same_document() {
            // Scenario "Copy and paste into the same document": a second arrow, offset, as one
            // undo step, selected.
            let (mut doc, mut history, original) = arrow_doc();
            let mut sel = Selection::new();
            sel.set(doc.scene(), [original]);
            let clip = copied(&doc, sel.ids());
            let edit = paste(&doc, &clip).unwrap();
            assert_eq!(
                (edit.action, edit.subject),
                (Action::Paste, Subject::One("arrow"))
            );
            apply(&mut doc, &mut history, &mut sel, edit);
            let all = doc.scene().ids();
            assert_eq!(all.len(), 2);
            assert_eq!(all[0], original);
            assert_eq!(sel.ids(), &all[1..], "the pasted arrow is selected");
            let (a, b) = (arrow(&doc, original), arrow(&doc, all[1]));
            assert_eq!(b.start, Point::new(a.start.x + 10.0, a.start.y + 10.0));
            assert_eq!(b.end, Point::new(a.end.x + 10.0, a.end.y + 10.0));
            assert_eq!(b.color, a.color);
            assert_eq!(b.width, a.width);
            // One undo step.
            history.undo(&mut doc).unwrap();
            assert_eq!(doc.scene().ids(), vec![original]);
            history.redo(&mut doc).unwrap();
            assert_eq!(doc.scene().len(), 2);
            // The same clip pastes again (fresh ids every time), at the same offset.
            let edit = paste(&doc, &clip).unwrap();
            apply(&mut doc, &mut history, &mut sel, edit);
            let all = doc.scene().ids();
            assert_eq!(all.len(), 3);
            assert_eq!(arrow(&doc, all[2]).start, b.start);
        }

        #[test]
        fn paste_matches_duplicate() {
            // Same command shape, offset and selection as Ctrl+D.
            let (doc, _, original) = arrow_doc();
            let mut sel = Selection::new();
            sel.set(doc.scene(), [original]);
            let dup = duplicate(&doc, &sel).unwrap();
            let pasted = paste(&doc, &copied(&doc, sel.ids())).unwrap();
            assert_eq!(dup.select, pasted.select);
            let nodes = |c: &Command| match c {
                Command::Insert { nodes, resources } => {
                    assert!(resources.is_empty());
                    nodes
                        .iter()
                        .map(|(at, n)| (*at, node_to_json(n).unwrap()))
                        .collect::<Vec<_>>()
                }
                other => panic!("expected Insert, got {other:?}"),
            };
            assert_eq!(nodes(&dup.command), nodes(&pasted.command));
        }

        #[test]
        fn paste_into_another_document_after_the_source_closed() {
            // Scenarios "Paste into a different document" and "Paste after the source document
            // closes": two image objects sharing images/1.png, pasted into a document whose own
            // images/1.png is another picture.
            let source = archive::tests::sample();
            let ids = source.scene().ids();
            let picture = source
                .resources()
                .get("images/1.png")
                .unwrap()
                .png()
                .unwrap();
            let expected: Vec<serde_json::Value> = source
                .scene()
                .nodes()
                .iter()
                .map(|n| {
                    let mut moved = n.clone();
                    moved.object.translate(10.0, 10.0);
                    node_to_json(&moved).unwrap()
                })
                .collect();
            let bytes = clipboard::encode(&source, &ids).unwrap().unwrap();
            drop(source); // closed

            let mut target = Document::new(Bitmap::filled(300, 200, palette::PAPER).unwrap());
            let mut history = History::new();
            let own = Bitmap::filled(20, 20, palette::CORAL).unwrap();
            let cmd = Image::insert_command(&target, own.clone(), Point::new(50.0, 50.0));
            history.execute(&mut target, "Insert", cmd).unwrap();
            let mut sel = Selection::new();
            let edit = paste(&target, &clipboard::decode(&bytes).unwrap()).unwrap();
            assert_eq!(edit.subject, Subject::Many(2));
            apply(&mut target, &mut history, &mut sel, edit);
            assert_eq!(target.object_count(), 3);
            assert_eq!(sel.ids(), &target.scene().ids()[1..]);
            // The target's own picture is untouched; the pasted objects share a new id.
            assert!(target.resources().get("images/1.png").unwrap().ptr_eq(&own));
            assert_eq!(
                target
                    .resources()
                    .get("images/2.png")
                    .unwrap()
                    .png()
                    .unwrap(),
                picture
            );
            for (k, node) in target.scene().nodes()[1..].iter().enumerate() {
                assert_eq!(node.object.resources(), vec!["images/2.png"]);
                // Everything else exactly as copied (and moved by the offset).
                let mut written = node_to_json(node).unwrap();
                let mut want = expected[k].clone();
                for v in [&mut written, &mut want] {
                    v["id"] = json!(0);
                    v["source"] = json!("");
                }
                assert_eq!(written, want);
            }
            // The document saves and reopens with all three.
            let back = archive::decode(&archive::encode(&target).unwrap()).unwrap();
            assert_eq!(back.object_count(), 3);
            assert_eq!(back.resources().len(), 2);
            // One undo step takes both pasted objects away.
            history.undo(&mut target).unwrap();
            assert_eq!(target.object_count(), 1);
        }

        #[test]
        fn pasting_where_the_picture_is_already_shares_it() {
            // Back into the source document: like Duplicate, no second copy of the image.
            let mut doc = archive::tests::sample();
            let mut history = History::new();
            let mut sel = Selection::new();
            let first = doc.scene().ids()[0];
            let edit = paste(&doc, &copied(&doc, &[first])).unwrap();
            match &edit.command {
                Command::Insert { resources, .. } => assert!(resources.is_empty()),
                other => panic!("expected Insert, got {other:?}"),
            }
            apply(&mut doc, &mut history, &mut sel, edit);
            assert_eq!(doc.resources().len(), 1);
            let pasted = sel.ids()[0];
            assert_eq!(
                doc.scene().get(pasted).unwrap().object.resources(),
                vec!["images/1.png"]
            );
        }

        #[test]
        fn pasted_groups_get_new_member_ids() {
            let (mut doc, mut history, ids) = doc_with(0);
            assert!(ids.is_empty());
            for x in [10.0, 100.0] {
                let edit = add(
                    &doc,
                    Box::new(Arrow::new(Point::new(x, 10.0), Point::new(x + 50.0, 40.0))),
                )
                .unwrap();
                history.execute(&mut doc, "Add", edit.command).unwrap();
            }
            let mut sel = Selection::new();
            sel.set(doc.scene(), doc.scene().ids());
            let edit = group(&doc, &sel).unwrap();
            apply(&mut doc, &mut history, &mut sel, edit);
            let gid = sel.ids()[0];
            let clip = copied(&doc, &[gid]);
            for _ in 0..2 {
                let edit = paste(&doc, &clip).unwrap();
                assert_eq!(edit.subject, Subject::One("group"));
                apply(&mut doc, &mut history, &mut sel, edit);
            }
            let groups = doc.scene().ids();
            assert_eq!(groups.len(), 3);
            let mut seen = HashSet::new();
            for id in &groups {
                assert!(seen.insert(*id));
                for m in doc.scene().get(*id).unwrap().object.nested_ids() {
                    assert!(seen.insert(m), "member id {m:?} repeated");
                    assert_eq!(doc.scene().owner(m), Some(*id));
                }
            }
        }

        #[test]
        fn objects_of_a_newer_annota_paste_and_the_document_still_reopens() {
            // A payload of format 1.3: an arrow with a key this version does not know, and an
            // arrow this version cannot read. Both are kept; the document's file says 1.3, so a
            // save reopens (a 1.0 file with an unreadable arrow would not).
            let (mut doc, mut history, _) = arrow_doc();
            let mut known = node_to_json(&doc.scene().nodes()[0]).unwrap();
            known["glow"] = json!({"radius": 4});
            let clip = Clip {
                format_minor: 3,
                nodes: vec![
                    node_from_json(known, true).unwrap(),
                    node_from_json(json!({"id": 9, "type": "arrow", "head": "spiral"}), true)
                        .unwrap(),
                ],
                resources: Vec::new(),
            };
            let mut sel = Selection::new();
            let edit = paste(&doc, &clip).unwrap();
            apply(&mut doc, &mut history, &mut sel, edit);
            assert_eq!(doc.object_count(), 3);
            // Only the arrow this version can show is selectable.
            assert_eq!(sel.ids().len(), 1);
            let saved = archive::document_json(&doc).unwrap();
            assert_eq!(saved["format"], json!("1.3"));
            assert_eq!(saved["objects"][1]["glow"], json!({"radius": 4}));
            assert_eq!(saved["objects"][2]["head"], json!("spiral"));
            let back = archive::decode(&archive::encode(&doc).unwrap()).unwrap();
            assert_eq!(back.object_count(), 3);
            assert_eq!(
                back.scene().nodes()[2].object.type_name(),
                UnknownObject::TYPE
            );
            // Undo puts the document's own version back too.
            history.undo(&mut doc).unwrap();
            assert_eq!(
                archive::document_json(&doc).unwrap()["format"],
                json!("1.0")
            );
            // A payload of this version leaves the document's version alone.
            let plain = paste(&doc, &copied(&doc, &doc.scene().ids())).unwrap();
            assert!(matches!(plain.command, Command::Insert { .. }));
        }

        #[test]
        fn unknown_objects_get_new_ids_and_image_names() {
            // An unknown container that holds children and an image reference.
            let mut doc = Document::new(Bitmap::filled(40, 30, Rgba::WHITE).unwrap());
            let mut history = History::new();
            let own = Bitmap::filled(4, 4, palette::CORAL).unwrap();
            let cmd = Image::insert_command(&doc, own, Point::new(10.0, 10.0));
            history.execute(&mut doc, "Insert", cmd).unwrap();
            let mut unknown = node_from_json(
                json!({"id": 5, "type": "stack", "art": "images/1.png",
                       "children": [{"id": 6, "type": "arrow"}]}),
                true,
            )
            .unwrap();
            let art = Bitmap::filled(8, 8, palette::AZURE).unwrap();
            unknown
                .object
                .as_any_mut()
                .downcast_mut::<UnknownObject>()
                .unwrap()
                .resources
                .push("images/1.png".into());
            let clip = Clip {
                format_minor: 2,
                nodes: vec![unknown],
                resources: vec![("images/1.png".into(), art.clone())],
            };
            let mut sel = Selection::new();
            for _ in 0..2 {
                let edit = paste(&doc, &clip).unwrap();
                apply(&mut doc, &mut history, &mut sel, edit);
            }
            let saved = archive::document_json(&doc).unwrap();
            let objects = saved["objects"].as_array().unwrap();
            assert_eq!(objects.len(), 3);
            assert_eq!(objects[1]["art"], json!("images/2.png"));
            assert_eq!(
                objects[2]["art"],
                json!("images/2.png"),
                "shared the 2nd time"
            );
            assert_ne!(
                objects[1]["children"][0]["id"],
                objects[2]["children"][0]["id"]
            );
            assert!(doc.resources().get("images/2.png").unwrap().ptr_eq(&art));
            let back = archive::decode(&archive::encode(&doc).unwrap()).unwrap();
            assert_eq!(back.object_count(), 3);
        }

        #[test]
        fn nothing_to_paste() {
            let (doc, _, _) = arrow_doc();
            let empty = Clip {
                format_minor: 0,
                nodes: Vec::new(),
                resources: Vec::new(),
            };
            assert!(paste(&doc, &empty).is_none());
        }
    }
}
