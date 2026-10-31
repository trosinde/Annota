//! "Redaction export warning" (spec annotation-tools; decision D7; add-export-share task 2.7,
//! mockup 10): before a document leaves Annota, its blur and pixelate redactions — at any
//! depth, inside groups too — are counted ([`reversible`]), and "Convert to Solid" turns every
//! one of them into a solid box as **one** undo step ([`make_solid`]).
//!
//! Free of widgets; the alert itself is `ui::editor::redaction_warning`.

use crate::document::annotation::{Annotation, PropertyValue};
use crate::document::command::Command;
use crate::document::scene::ObjectId;
use crate::document::Document;
use crate::tools::group::Group;
use crate::tools::redaction::{redactions_in, Redaction, RedactionKind};

use super::ops::{Action, Edit, Subject};

/// How many redactions of a document might be reversed from an exported image.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reversible {
    pub blur: usize,
    pub pixelate: usize,
}

impl Reversible {
    pub fn total(&self) -> usize {
        self.blur + self.pixelate
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// The blur and pixelate redactions of `doc`, groups included (at any depth).
pub fn reversible(doc: &Document) -> Reversible {
    let mut found = Reversible::default();
    for node in doc.scene().nodes() {
        for redaction in redactions_in(node.object.as_ref()) {
            match redaction.kind {
                RedactionKind::Blur => found.blur += 1,
                RedactionKind::Pixelate => found.pixelate += 1,
                RedactionKind::Solid => {}
            }
        }
    }
    found
}

/// Sets every reversible redaction in `object` (itself or group members at any depth) to
/// solid, through `set_property("redaction", "solid")`; returns how many changed.
fn solidify(object: &mut dyn Annotation) -> usize {
    if let Some(redaction) = object.as_any_mut().downcast_mut::<Redaction>() {
        let solid = PropertyValue::Choice(RedactionKind::Solid.key().into());
        let changed = redaction.reversible() && redaction.set_property("redaction", &solid).is_ok();
        return usize::from(changed);
    }
    match object.as_any_mut().downcast_mut::<Group>() {
        Some(group) => group
            .children
            .iter_mut()
            .map(|child| solidify(child.object.as_mut()))
            .sum(),
        None => 0,
    }
}

/// "Convert to Solid" (mockup 10): every blur and pixelate redaction becomes a solid box, as
/// one undo step — a `Command::Replace` of each top-level object that holds one (a group is
/// replaced by a copy whose members are converted). `None` if there is nothing to convert.
pub fn make_solid(doc: &Document) -> Option<Edit> {
    let mut objects: Vec<(ObjectId, Box<dyn Annotation>)> = Vec::new();
    let mut converted = 0;
    for node in doc.scene().nodes() {
        if !redactions_in(node.object.as_ref())
            .iter()
            .any(|r| r.reversible())
        {
            continue;
        }
        let mut copy = node.object.clone_box();
        let n = solidify(copy.as_mut());
        if n > 0 {
            converted += n;
            objects.push((node.id, copy));
        }
    }
    if objects.is_empty() {
        return None;
    }
    Some(Edit {
        action: Action::MakeSolid,
        subject: Subject::Many(converted),
        command: Command::Replace { objects },
        select: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::palette;
    use crate::document::command::History;
    use crate::document::geometry::Rect;
    use crate::document::scene::tests::TestBox;
    use crate::document::scene::Node;

    fn redaction(x: f64, kind: RedactionKind) -> Box<dyn Annotation> {
        let mut r = Redaction::new(Rect::new(x, 10.0, 20.0, 20.0));
        r.kind = kind;
        Box::new(r)
    }

    /// Top level: pixelate, solid, a box, blur; a group holding a box, a pixelate redaction
    /// and a nested group with a blur redaction.
    fn document() -> (Document, History) {
        let mut doc = Document::new(Bitmap::filled(400, 100, palette::PAPER).unwrap());
        let ids = doc.scene().fresh_ids(10);
        let inner = Group::new(vec![
            Node::new(ids[8], redaction(320.0, RedactionKind::Blur)),
            Node::new(ids[9], TestBox::boxed(350.0, 10.0, 20.0, 20.0)),
        ]);
        let outer = Group::new(vec![
            Node::new(ids[5], TestBox::boxed(250.0, 10.0, 20.0, 20.0)),
            Node::new(ids[6], redaction(280.0, RedactionKind::Pixelate)),
            Node::new(ids[7], Box::new(inner)),
        ]);
        let objects: Vec<Box<dyn Annotation>> = vec![
            redaction(10.0, RedactionKind::Pixelate),
            redaction(40.0, RedactionKind::Solid),
            TestBox::boxed(70.0, 10.0, 20.0, 20.0),
            redaction(100.0, RedactionKind::Blur),
            Box::new(outer),
        ];
        let nodes = objects
            .into_iter()
            .zip(&ids[..5])
            .enumerate()
            .map(|(k, (o, id))| (k, Node::new(*id, o)))
            .collect();
        let mut history = History::new();
        history
            .execute(
                &mut doc,
                "Add",
                Command::Insert {
                    nodes,
                    resources: Vec::new(),
                },
            )
            .unwrap();
        (doc, history)
    }

    fn kinds(doc: &Document) -> Vec<RedactionKind> {
        doc.scene()
            .nodes()
            .iter()
            .flat_map(|n| redactions_in(n.object.as_ref()))
            .map(|r| r.kind)
            .collect()
    }

    #[test]
    fn counts_blur_and_pixelate_at_any_depth() {
        let (doc, _) = document();
        assert_eq!(
            reversible(&doc),
            Reversible {
                blur: 2,
                pixelate: 2
            }
        );
        assert_eq!(reversible(&doc).total(), 4);
        let plain = Document::new(Bitmap::filled(10, 10, palette::PAPER).unwrap());
        assert!(reversible(&plain).is_empty());
    }

    #[test]
    fn convert_to_solid_is_one_undo_step() {
        let (mut doc, mut history) = document();
        let before = kinds(&doc);
        let edit = make_solid(&doc).expect("there is something to convert");
        assert_eq!(edit.action, Action::MakeSolid);
        assert_eq!(edit.subject, Subject::Many(4));
        // Only the objects holding a reversible redaction are replaced: the pixelate, the
        // blur and the group (not the solid box or the plain box).
        match &edit.command {
            Command::Replace { objects } => assert_eq!(objects.len(), 3),
            other => panic!("{other:?}"),
        }
        let steps = history.undo_len();
        history.execute(&mut doc, "Convert", edit.command).unwrap();
        assert_eq!(history.undo_len(), steps + 1);
        assert!(reversible(&doc).is_empty());
        assert!(kinds(&doc).iter().all(|k| *k == RedactionKind::Solid));
        assert_eq!(kinds(&doc).len(), 5);
        // Nothing left to convert: no phantom undo step.
        assert!(make_solid(&doc).is_none());
        // One Undo brings every one of them back, the nested ones included.
        history.undo(&mut doc).unwrap();
        assert_eq!(kinds(&doc), before);
    }

    #[test]
    fn convert_keeps_geometry_and_ids() {
        let (mut doc, mut history) = document();
        let ids: Vec<ObjectId> = doc.scene().nodes().iter().map(|n| n.id).collect();
        let bounds: Vec<_> = doc
            .scene()
            .nodes()
            .iter()
            .map(|n| n.object.bounds())
            .collect();
        let edit = make_solid(&doc).unwrap();
        history.execute(&mut doc, "Convert", edit.command).unwrap();
        let after: Vec<ObjectId> = doc.scene().nodes().iter().map(|n| n.id).collect();
        assert_eq!(after, ids);
        let after_bounds: Vec<_> = doc
            .scene()
            .nodes()
            .iter()
            .map(|n| n.object.bounds())
            .collect();
        assert_eq!(after_bounds, bounds);
    }
}
