//! Step sequences across the document (spec annotation-tools: Step — "Auto-increment",
//! "Renumber on delete"; mockups 11–13).
//!
//! **The state machine.** A marker belongs to a sequence (an id) and has a position in it
//! (`number`). Nothing else is stored; everything follows from the markers in the document,
//! top-level and inside groups:
//!
//! * *Placing* (a click with the step tool) joins the canvas's current sequence — the one
//!   Restart Sequence started, else the newest (highest id) in the document, else 1 — with the
//!   number after the highest of that sequence (or a number the user set as "Next value").
//!   Three clicks in an empty document give 1, 2, 3.
//! * *Restart Sequence* picks an id no marker has: the next click gives 1 (or A) again, and the
//!   earlier markers keep their own numbers.
//! * *Deleting* markers renumbers the markers that follow them **in the same sequence only**:
//!   each moves down by the number of deleted markers of its sequence before it. Markers 1–4,
//!   delete 2 → 1, 2, 3; a restarted sequence's A, B stay A, B. Deleting and renumbering are
//!   one `Batch`, so one undo brings back the marker and the old numbers exactly.
//! * A value typed for one marker only (`Step::value`) is just its label; it keeps its place in
//!   the sequence and is not renumbered visibly.
//!
//! New markers count like the markers already in their sequence (1, 2, 3 or A, B, C …).

use std::collections::HashMap;

use crate::document::annotation::Annotation;
use crate::document::scene::{ObjectId, Scene};
use crate::tools::group::Group;
use crate::tools::step::{Counting, Step};

/// Every marker in `object` (itself, or inside it at any depth).
fn collect<'a>(object: &'a dyn Annotation, out: &mut Vec<&'a Step>) {
    if let Some(step) = object.as_any().downcast_ref::<Step>() {
        out.push(step);
    }
    for child in object.children() {
        collect(child.object.as_ref(), out);
    }
}

/// Every marker in the document, top-level and inside groups, bottom to top.
pub fn markers(scene: &Scene) -> Vec<&Step> {
    let mut out = Vec::new();
    for node in scene.nodes() {
        collect(node.object.as_ref(), &mut out);
    }
    out
}

/// The newest sequence in the document (highest id), if it has markers.
pub fn newest_sequence(scene: &Scene) -> Option<u32> {
    markers(scene).iter().map(|s| s.sequence).max()
}

/// An id no marker has yet (Restart Sequence).
pub fn fresh_sequence(scene: &Scene) -> u32 {
    newest_sequence(scene).map_or(1, |s| s.saturating_add(1))
}

/// The number after the highest marker of `sequence` (1 if it has none).
pub fn next_number(scene: &Scene, sequence: u32) -> u32 {
    markers(scene)
        .iter()
        .filter(|s| s.sequence == sequence)
        .map(|s| s.number)
        .max()
        .map_or(1, |n| n.saturating_add(1))
}

/// How the markers of `sequence` count (the topmost one decides), if it has any.
pub fn counting(scene: &Scene, sequence: u32) -> Option<Counting> {
    markers(scene)
        .iter()
        .rev()
        .find(|s| s.sequence == sequence)
        .map(|s| s.counting)
}

/// The sequence the next marker joins: `current` (set by Restart Sequence) or the newest in the
/// document, else 1.
pub fn current_sequence(scene: &Scene, current: Option<u32>) -> u32 {
    current.or_else(|| newest_sequence(scene)).unwrap_or(1)
}

/// The markers left after removing the top-level objects `removed` that need a new number,
/// as replacements of their top-level objects (groups with markers inside are replaced
/// whole). Empty if no marker was removed or none follows one.
pub fn renumbered(scene: &Scene, removed: &[ObjectId]) -> Vec<(ObjectId, Box<dyn Annotation>)> {
    // Numbers deleted per sequence.
    let mut deleted: HashMap<u32, Vec<u32>> = HashMap::new();
    for id in removed {
        if let Some(node) = scene.get(*id) {
            let mut gone = Vec::new();
            collect(node.object.as_ref(), &mut gone);
            for step in gone {
                deleted.entry(step.sequence).or_default().push(step.number);
            }
        }
    }
    if deleted.is_empty() {
        return Vec::new();
    }
    let shift = |step: &Step| -> u32 {
        deleted.get(&step.sequence).map_or(0, |numbers| {
            numbers.iter().filter(|&&n| n < step.number).count() as u32
        })
    };
    fn renumber(object: &mut dyn Annotation, shift: &dyn Fn(&Step) -> u32) -> bool {
        let any = object.as_any_mut();
        if let Some(step) = any.downcast_mut::<Step>() {
            let by = shift(step);
            if by > 0 {
                step.number = step.number.saturating_sub(by).max(1);
                return true;
            }
            return false;
        }
        let Some(group) = any.downcast_mut::<Group>() else {
            return false;
        };
        let mut changed = false;
        for child in &mut group.children {
            changed |= renumber(child.object.as_mut(), shift);
        }
        changed
    }
    scene
        .nodes()
        .iter()
        .filter(|n| !removed.contains(&n.id))
        .filter_map(|n| {
            let mut copy = n.object.clone();
            renumber(copy.as_mut(), &shift).then_some((n.id, copy))
        })
        .collect()
}

/// What deleting `removed` does to steps, for the toast (mockup 13: "Step 2 deleted · 2 steps
/// renumbered"): the label of the one deleted marker (if exactly one object, a marker, is
/// deleted) and how many markers get a new number.
pub fn deletion_summary(scene: &Scene, removed: &[ObjectId]) -> (Option<String>, usize) {
    let label = match removed {
        [one] => scene
            .get(*one)
            .and_then(|n| n.object.as_any().downcast_ref::<Step>())
            .map(Step::label),
        _ => None,
    };
    let count = renumbered(scene, removed)
        .iter()
        .map(|(id, after)| {
            let mut before = Vec::new();
            if let Some(node) = scene.get(*id) {
                collect(node.object.as_ref(), &mut before);
            }
            let mut now = Vec::new();
            collect(after.as_ref(), &mut now);
            before
                .iter()
                .zip(&now)
                .filter(|(a, b)| a.number != b.number)
                .count()
        })
        .sum();
    (label, count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::{Command, History};
    use crate::document::geometry::Point;
    use crate::document::scene::Node;
    use crate::document::Document;
    use crate::editing::ops;
    use crate::editing::selection::Selection;

    fn place(doc: &mut Document, history: &mut History, sequence: u32, at: f64) -> ObjectId {
        let number = next_number(doc.scene(), sequence);
        let id = doc.scene().fresh_ids(1)[0];
        let step = Step::new(Point::new(at, 50.0), sequence, number);
        let at = doc.scene().len();
        history
            .execute(
                doc,
                "Add",
                Command::Insert {
                    nodes: vec![(at, Node::new(id, Box::new(step)))],
                    resources: Vec::new(),
                },
            )
            .unwrap();
        id
    }

    fn labels(doc: &Document) -> Vec<(u32, String)> {
        markers(doc.scene())
            .iter()
            .map(|s| (s.sequence, s.label()))
            .collect()
    }

    fn doc() -> (Document, History) {
        (
            Document::new(Bitmap::filled(800, 200, Rgba::WHITE).unwrap()),
            History::new(),
        )
    }

    #[test]
    fn three_clicks_make_1_2_3() {
        // Spec "Auto-increment".
        let (mut doc, mut history) = doc();
        let seq = current_sequence(doc.scene(), None);
        for x in [50.0, 150.0, 250.0] {
            place(&mut doc, &mut history, seq, x);
        }
        assert_eq!(
            labels(&doc),
            vec![(1, "1".into()), (1, "2".into()), (1, "3".into())]
        );
    }

    #[test]
    fn deleting_marker_2_of_4_renumbers_the_rest() {
        // Spec "Renumber on delete": markers 1–4, delete 2 → 1, 2, 3; one undo step.
        let (mut doc, mut history) = doc();
        let ids: Vec<ObjectId> = (0..4)
            .map(|k| place(&mut doc, &mut history, 1, 50.0 + 100.0 * f64::from(k)))
            .collect();
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[1]]);
        assert_eq!(
            deletion_summary(doc.scene(), &[ids[1]]),
            (Some("2".into()), 2)
        );
        let edit = ops::delete(&doc, &sel).unwrap();
        history.execute(&mut doc, "Delete", edit.command).unwrap();
        let read: Vec<String> = labels(&doc).into_iter().map(|(_, l)| l).collect();
        assert_eq!(read, ["1", "2", "3"]);
        // The renumbered markers are the old 3 and 4, in place.
        assert_eq!(doc.scene().ids(), vec![ids[0], ids[2], ids[3]]);
        history.undo(&mut doc).unwrap();
        let read: Vec<String> = labels(&doc).into_iter().map(|(_, l)| l).collect();
        assert_eq!(read, ["1", "2", "3", "4"]);
        // The next click continues after the highest.
        assert_eq!(next_number(doc.scene(), 1), 5);
    }

    #[test]
    fn a_restarted_sequence_keeps_its_own_numbering() {
        // Redline: "markers keep their sequence id, deleting renumbers only the same sequence".
        let (mut doc, mut history) = doc();
        let first: Vec<ObjectId> = (0..3)
            .map(|k| place(&mut doc, &mut history, 1, 50.0 + 50.0 * f64::from(k)))
            .collect();
        let restarted = fresh_sequence(doc.scene());
        assert_eq!(restarted, 2);
        let a = place(&mut doc, &mut history, restarted, 400.0);
        let b = place(&mut doc, &mut history, restarted, 450.0);
        assert_eq!(next_number(doc.scene(), restarted), 3);
        // Delete marker 1 of the first sequence: 2, 3 → 1, 2; the restarted 1, 2 stay.
        let mut sel = Selection::new();
        sel.set(doc.scene(), [first[0]]);
        let edit = ops::delete(&doc, &sel).unwrap();
        history.execute(&mut doc, "Delete", edit.command).unwrap();
        assert_eq!(
            labels(&doc),
            vec![
                (1, "1".into()),
                (1, "2".into()),
                (2, "1".into()),
                (2, "2".into())
            ]
        );
        // Deleting from the restarted sequence leaves the first alone.
        sel.set(doc.scene(), [a]);
        let edit = ops::delete(&doc, &sel).unwrap();
        history.execute(&mut doc, "Delete", edit.command).unwrap();
        assert_eq!(
            labels(&doc),
            vec![(1, "1".into()), (1, "2".into()), (2, "1".into())]
        );
        assert!(doc.scene().get(b).is_some());
        // With no restart pending, the newest sequence continues.
        assert_eq!(current_sequence(doc.scene(), None), 2);
    }

    #[test]
    fn manual_values_and_grouped_markers() {
        let (mut doc, mut history) = doc();
        let ids: Vec<ObjectId> = (0..4)
            .map(|k| place(&mut doc, &mut history, 1, 50.0 + 100.0 * f64::from(k)))
            .collect();
        // Marker 4 reads "7" (this marker only); markers 3 and 4 are grouped.
        let mut four = doc.scene().get(ids[3]).unwrap().object.clone();
        four.as_any_mut().downcast_mut::<Step>().unwrap().value = Some("7".into());
        history
            .execute(
                &mut doc,
                "Value",
                Command::Replace {
                    objects: vec![(ids[3], four)],
                },
            )
            .unwrap();
        let mut sel = Selection::new();
        sel.set(doc.scene(), [ids[2], ids[3]]);
        let edit = ops::group(&doc, &sel).unwrap();
        history.execute(&mut doc, "Group", edit.command).unwrap();
        // Deleting marker 1 renumbers inside the group; the typed value stays.
        sel.set(doc.scene(), [ids[0]]);
        let edit = ops::delete(&doc, &sel).unwrap();
        history.execute(&mut doc, "Delete", edit.command).unwrap();
        let numbers: Vec<u32> = markers(doc.scene()).iter().map(|s| s.number).collect();
        assert_eq!(numbers, [1, 2, 3]);
        let read: Vec<String> = labels(&doc).into_iter().map(|(_, l)| l).collect();
        assert_eq!(read, ["1", "2", "7"]);
        // Deleting a non-step object renumbers nothing.
        assert!(renumbered(doc.scene(), &[]).is_empty());
    }
}
