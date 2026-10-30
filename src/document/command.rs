//! Undo/redo: every model mutation is a [`Command`] (CLAUDE.md §7).
//!
//! A command applies itself to a document and returns its exact inverse, which is again a
//! command. The [`History`] keeps the inverses: undo applies one and keeps *its* inverse for
//! redo. Commands are small and generic (insert, remove, replace, reorder, set canvas/image/
//! metadata, batch); editor operations are compositions of them — e.g. moving, resizing,
//! rotating or recolouring objects is a `Replace` with edited clones, "Flatten All" a `Batch`
//! of `SetImage` and `Remove`. Replacing whole objects makes undo exact (no floating-point
//! drift from applying inverse deltas) and costs little, because objects hold bitmaps by
//! reference.

use std::collections::{HashSet, VecDeque};

use super::annotation::Annotation;
use super::bitmap::Bitmap;
use super::scene::{Node, ObjectId, SceneError};
use super::{BaseImage, Canvas, Document, Metadata};

/// Undo steps kept per document (spec editor-canvas: at least 200).
pub const HISTORY_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub enum Command {
    /// Inserts nodes at their final stacking positions (ascending), adding the embedded
    /// images they need first.
    Insert {
        nodes: Vec<(usize, Node)>,
        resources: Vec<(String, Bitmap)>,
    },
    /// Removes objects.
    Remove {
        ids: Vec<ObjectId>,
    },
    /// Swaps objects for edited versions (move, resize, rotate, property changes).
    Replace {
        objects: Vec<(ObjectId, Box<dyn Annotation>)>,
    },
    /// New stacking order, bottom to top; must contain every object once.
    Reorder {
        order: Vec<ObjectId>,
    },
    SetCanvas(Canvas),
    SetImage(BaseImage),
    SetMetadata(Metadata),
    /// Several commands as one undo step, applied in order, all or nothing.
    Batch(Vec<Command>),
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error(transparent)]
    Scene(#[from] SceneError),
    #[error("object {0:?} appears twice in one command")]
    Repeated(ObjectId),
    #[error("resource '{0}' already holds a different image")]
    ResourceConflict(String),
    #[error("invalid resource id '{0}'")]
    ResourceId(String),
    #[error("invalid value: {0}")]
    Value(&'static str),
}

fn no_repeats(ids: impl IntoIterator<Item = ObjectId>) -> Result<(), CommandError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(CommandError::Repeated(id));
        }
    }
    Ok(())
}

impl Command {
    /// Moves objects by `(dx, dy)` (drag, arrow-key nudge).
    pub fn translate(doc: &Document, ids: &[ObjectId], dx: f64, dy: f64) -> Command {
        Command::Replace {
            objects: ids
                .iter()
                .filter_map(|&id| {
                    let mut object = doc.scene().get(id)?.object.clone();
                    object.translate(dx, dy);
                    Some((id, object))
                })
                .collect(),
        }
    }

    /// Applies the command and returns its inverse. On error the document is unchanged.
    pub fn apply(self, doc: &mut Document) -> Result<Command, CommandError> {
        self.validate(doc)?;
        match self {
            Command::Insert { nodes, resources } => {
                for (id, bitmap) in resources {
                    doc.resources_mut().insert(id, bitmap);
                }
                let ids = nodes.iter().map(|(_, n)| n.id).collect();
                for (position, node) in nodes {
                    doc.scene_mut().insert(position, node)?;
                }
                Ok(Command::Remove { ids })
            }
            Command::Remove { ids } => {
                let mut positions: Vec<(usize, ObjectId)> = ids
                    .iter()
                    .filter_map(|&id| doc.scene().position(id).map(|p| (p, id)))
                    .collect();
                positions.sort_unstable();
                let mut removed = Vec::with_capacity(positions.len());
                for &(_, id) in positions.iter().rev() {
                    removed.push(doc.scene_mut().remove(id)?);
                }
                removed.reverse();
                // Embedded images stay in the document so the inverse can restore objects;
                // saving only writes images that objects still use.
                Ok(Command::Insert {
                    nodes: removed,
                    resources: Vec::new(),
                })
            }
            Command::Replace { objects } => {
                let mut previous = Vec::with_capacity(objects.len());
                for (id, object) in objects {
                    previous.push((id, doc.scene_mut().replace(id, object)?));
                }
                Ok(Command::Replace { objects: previous })
            }
            Command::Reorder { order } => Ok(Command::Reorder {
                order: doc.scene_mut().reorder(&order)?,
            }),
            Command::SetCanvas(canvas) => Ok(Command::SetCanvas(doc.replace_canvas(canvas))),
            Command::SetImage(image) => Ok(Command::SetImage(doc.replace_image(image))),
            Command::SetMetadata(metadata) => {
                Ok(Command::SetMetadata(doc.replace_metadata(metadata)))
            }
            Command::Batch(commands) => {
                let mut inverses: Vec<Command> = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => {
                            // Roll back what was applied, newest first.
                            for inverse in inverses.into_iter().rev() {
                                let _ = inverse.apply(doc);
                            }
                            return Err(err);
                        }
                    }
                }
                inverses.reverse();
                Ok(Command::Batch(inverses))
            }
        }
    }

    /// Checks everything that can fail before anything is changed.
    fn validate(&self, doc: &Document) -> Result<(), CommandError> {
        let scene = doc.scene();
        match self {
            Command::Insert { nodes, resources } => {
                no_repeats(nodes.iter().map(|(_, n)| n.id))?;
                for (k, (position, node)) in nodes.iter().enumerate() {
                    if scene.position(node.id).is_some() {
                        return Err(SceneError::Duplicate(node.id).into());
                    }
                    let previous = k.checked_sub(1).and_then(|p| nodes.get(p)).map(|(p, _)| *p);
                    if *position > scene.len() + k || previous.is_some_and(|p| p >= *position) {
                        return Err(SceneError::Index(*position).into());
                    }
                }
                for (id, bitmap) in resources {
                    if super::parse_resource_id(id).is_none() {
                        return Err(CommandError::ResourceId(id.clone()));
                    }
                    if let Some(existing) = doc.resources().get(id) {
                        if !existing.ptr_eq(bitmap) {
                            return Err(CommandError::ResourceConflict(id.clone()));
                        }
                    }
                }
                Ok(())
            }
            Command::Remove { ids } => {
                no_repeats(ids.iter().copied())?;
                match ids.iter().find(|id| scene.position(**id).is_none()) {
                    Some(id) => Err(SceneError::Unknown(*id).into()),
                    None => Ok(()),
                }
            }
            Command::Replace { objects } => {
                no_repeats(objects.iter().map(|(id, _)| *id))?;
                match objects.iter().find(|(id, _)| scene.position(*id).is_none()) {
                    Some((id, _)) => Err(SceneError::Unknown(*id).into()),
                    None => Ok(()),
                }
            }
            Command::Reorder { order } => {
                if scene.is_permutation(order) {
                    Ok(())
                } else {
                    Err(SceneError::NotAPermutation.into())
                }
            }
            Command::SetCanvas(canvas) => {
                if canvas.rect.width > 0 && canvas.rect.height > 0 {
                    Ok(())
                } else {
                    Err(CommandError::Value("canvas size must be positive"))
                }
            }
            Command::SetImage(image) => {
                if image.corner_radius.is_finite() && image.corner_radius >= 0.0 {
                    Ok(())
                } else {
                    Err(CommandError::Value("corner radius must be ≥ 0"))
                }
            }
            Command::SetMetadata(metadata) => {
                if metadata.scale_factor.is_finite() && metadata.scale_factor > 0.0 {
                    Ok(())
                } else {
                    Err(CommandError::Value("scale factor must be positive"))
                }
            }
            // Sub-commands are validated as they are applied; failures roll back.
            Command::Batch(_) => Ok(()),
        }
    }
}

struct Step {
    label: String,
    inverse: Command,
    before: u64,
    after: u64,
}

/// Undo/redo stacks of one document, with a revision number for "unsaved changes" and
/// autosave decisions.
pub struct History {
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    limit: usize,
    next_revision: u64,
    current: u64,
    saved: Option<u64>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self::with_limit(HISTORY_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit: limit.max(1),
            next_revision: 1,
            current: 0,
            saved: None,
        }
    }

    /// Applies `command` as a new undo step named `label` (translated by the caller, shown
    /// as "Undo <label>"). Clears the redo stack.
    pub fn execute(
        &mut self,
        doc: &mut Document,
        label: impl Into<String>,
        command: Command,
    ) -> Result<(), CommandError> {
        let inverse = command.apply(doc)?;
        let after = self.next_revision;
        self.next_revision += 1;
        self.undo.push_back(Step {
            label: label.into(),
            inverse,
            before: self.current,
            after,
        });
        self.current = after;
        while self.undo.len() > self.limit {
            self.undo.pop_front();
        }
        self.redo.clear();
        Ok(())
    }

    /// Undoes the last step; returns its label, or `None` if there is nothing to undo.
    pub fn undo(&mut self, doc: &mut Document) -> Result<Option<String>, CommandError> {
        let Some(step) = self.undo.pop_back() else {
            return Ok(None);
        };
        match step.inverse.clone().apply(doc) {
            Ok(redo) => {
                self.current = step.before;
                let label = step.label.clone();
                self.redo.push(Step {
                    inverse: redo,
                    ..step
                });
                Ok(Some(label))
            }
            Err(err) => {
                self.undo.push_back(step);
                Err(err)
            }
        }
    }

    /// Redoes the last undone step; returns its label, or `None` if there is nothing to redo.
    pub fn redo(&mut self, doc: &mut Document) -> Result<Option<String>, CommandError> {
        let Some(step) = self.redo.pop() else {
            return Ok(None);
        };
        match step.inverse.clone().apply(doc) {
            Ok(undo) => {
                self.current = step.after;
                let label = step.label.clone();
                self.undo.push_back(Step {
                    inverse: undo,
                    ..step
                });
                Ok(Some(label))
            }
            Err(err) => {
                self.redo.push(step);
                Err(err)
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Label for the Undo tooltip ("Undo Move Group").
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Identifies the document state: equal revisions mean equal content.
    pub fn revision(&self) -> u64 {
        self.current
    }

    /// Records that the current state is on disk (after Save, or after opening a file).
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    /// Whether the document differs from its saved state. A never-saved document is dirty.
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::color::Rgba;
    use crate::document::geometry::{IRect, Point, Rect};
    use crate::document::scene::tests::TestBox;
    use crate::document::Background;

    fn doc() -> Document {
        Document::new(Bitmap::filled(100, 80, Rgba::WHITE).unwrap())
    }

    fn insert(doc: &Document, rect: Rect) -> (ObjectId, Command) {
        let id = doc.scene().fresh_ids(1)[0];
        let node = Node::new(id, TestBox::boxed(rect.x, rect.y, rect.width, rect.height));
        let at = doc.scene().len();
        (
            id,
            Command::Insert {
                nodes: vec![(at, node)],
                resources: Vec::new(),
            },
        )
    }

    fn rect_of(doc: &Document, id: ObjectId) -> Rect {
        doc.scene().get(id).unwrap().object.arrange().rect
    }

    #[test]
    fn undo_move_restores_position() {
        // Spec editor-canvas "Undo move".
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(10.0, 10.0, 20.0, 20.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        let mv = Command::translate(&d, &[id], 33.3, -7.1);
        h.execute(&mut d, "Move", mv).unwrap();
        assert_eq!(rect_of(&d, id).x, 10.0 + 33.3);
        assert_eq!(h.undo(&mut d).unwrap().as_deref(), Some("Move"));
        assert_eq!(rect_of(&d, id), Rect::new(10.0, 10.0, 20.0, 20.0));
        assert_eq!(h.redo(&mut d).unwrap().as_deref(), Some("Move"));
        assert_eq!(rect_of(&d, id).x, 10.0 + 33.3);
    }

    #[test]
    fn keyboard_nudge_moves_ten_pixels() {
        // Spec editor-canvas "Keyboard nudge": Shift+→ moves 10 px right (the key binding
        // arrives with the editor UI; this is the model operation it issues).
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(10.0, 10.0, 20.0, 20.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        let mv = Command::translate(&d, &[id], 10.0, 0.0);
        h.execute(&mut d, "Move", mv).unwrap();
        assert_eq!(rect_of(&d, id), Rect::new(20.0, 10.0, 20.0, 20.0));
    }

    #[test]
    fn new_command_clears_redo() {
        let mut d = doc();
        let mut h = History::new();
        let (a, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add A", cmd).unwrap();
        h.undo(&mut d).unwrap();
        assert!(h.can_redo());
        assert_eq!(h.redo_label(), Some("Add A"));
        let (b, cmd) = insert(&d, Rect::new(1.0, 1.0, 5.0, 5.0));
        assert_ne!(a, b, "ids are not reused after undo");
        h.execute(&mut d, "Add B", cmd).unwrap();
        assert!(!h.can_redo());
        assert_eq!(h.redo(&mut d).unwrap(), None);
        assert_eq!(d.scene().ids(), vec![b]);
    }

    #[test]
    fn empty_stacks_do_nothing() {
        let mut d = doc();
        let mut h = History::new();
        assert!(!h.can_undo() && !h.can_redo());
        assert_eq!(h.undo(&mut d).unwrap(), None);
        assert_eq!(h.redo(&mut d).unwrap(), None);
        assert_eq!(h.undo_label(), None);
    }

    #[test]
    fn keeps_at_least_200_steps() {
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        for _ in 0..250 {
            let mv = Command::translate(&d, &[id], 1.0, 0.0);
            h.execute(&mut d, "Move", mv).unwrap();
        }
        assert_eq!(h.undo_len(), HISTORY_LIMIT);
        let mut undone = 0;
        while h.undo(&mut d).unwrap().is_some() {
            undone += 1;
        }
        assert_eq!(undone, 200);
        // The 50 oldest moves (and the insert) fell off the end.
        assert_eq!(rect_of(&d, id).x, 50.0);
        let mut redone = 0;
        while h.redo(&mut d).unwrap().is_some() {
            redone += 1;
        }
        assert_eq!(redone, 200);
        assert_eq!(rect_of(&d, id).x, 250.0);
    }

    #[test]
    fn remove_and_undo_restore_stacking_order() {
        let mut d = doc();
        let mut h = History::new();
        let mut ids = Vec::new();
        for i in 0..5 {
            let (id, cmd) = insert(&d, Rect::new(f64::from(i), 0.0, 5.0, 5.0));
            h.execute(&mut d, "Add", cmd).unwrap();
            ids.push(id);
        }
        let gone = vec![ids[3], ids[0], ids[2]];
        h.execute(&mut d, "Delete", Command::Remove { ids: gone })
            .unwrap();
        assert_eq!(d.scene().ids(), vec![ids[1], ids[4]]);
        h.undo(&mut d).unwrap();
        assert_eq!(d.scene().ids(), ids);
        h.redo(&mut d).unwrap();
        assert_eq!(d.scene().ids(), vec![ids[1], ids[4]]);
    }

    #[test]
    fn reorder_undoes_to_previous_order() {
        let mut d = doc();
        let mut h = History::new();
        let mut ids = Vec::new();
        for _ in 0..3 {
            let (id, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
            h.execute(&mut d, "Add", cmd).unwrap();
            ids.push(id);
        }
        let front = vec![ids[1], ids[2], ids[0]]; // Bring to Front for ids[0]
        h.execute(
            &mut d,
            "Bring to Front",
            Command::Reorder {
                order: front.clone(),
            },
        )
        .unwrap();
        assert_eq!(d.scene().ids(), front);
        assert_eq!(d.scene().hit_test(Point::new(1.0, 1.0), 0.0), Some(ids[0]));
        h.undo(&mut d).unwrap();
        assert_eq!(d.scene().ids(), ids);
    }

    #[test]
    fn select_all_recolour_is_one_step() {
        // Shape of spec "Recolour all arrows": every object of a type edited in one Replace.
        let mut d = doc();
        let mut h = History::new();
        for i in 0..5 {
            let (_, cmd) = insert(&d, Rect::new(f64::from(i) * 10.0, 0.0, 5.0, 5.0));
            h.execute(&mut d, "Add", cmd).unwrap();
        }
        let ids = d.scene().of_type("test-box");
        let objects = ids
            .iter()
            .map(|&id| {
                let mut o = d.scene().get(id).unwrap().object.clone();
                o.set_property(
                    "value",
                    &crate::document::annotation::PropertyValue::Number(7.0),
                )
                .unwrap();
                (id, o)
            })
            .collect();
        h.execute(&mut d, "Change Colour", Command::Replace { objects })
            .unwrap();
        let value = |d: &Document, id| {
            d.scene().get(id).unwrap().object.properties()[0]
                .value
                .clone()
        };
        assert!(ids
            .iter()
            .all(|&id| value(&d, id) == crate::document::annotation::PropertyValue::Number(7.0)));
        h.undo(&mut d).unwrap();
        assert!(ids
            .iter()
            .all(|&id| value(&d, id) == crate::document::annotation::PropertyValue::Number(0.0)));
    }

    #[test]
    fn invalid_commands_leave_the_document_alone() {
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add", cmd.clone()).unwrap();
        // Same id again.
        assert!(h.execute(&mut d, "Add", cmd).is_err());
        assert!(matches!(
            h.execute(
                &mut d,
                "Delete",
                Command::Remove {
                    ids: vec![ObjectId(999)]
                }
            ),
            Err(CommandError::Scene(SceneError::Unknown(ObjectId(999))))
        ));
        assert!(matches!(
            h.execute(&mut d, "Delete", Command::Remove { ids: vec![id, id] }),
            Err(CommandError::Repeated(_))
        ));
        assert!(h
            .execute(
                &mut d,
                "Canvas",
                Command::SetCanvas(Canvas {
                    rect: IRect::new(0, 0, 0, 10),
                    background: Background::Transparent
                })
            )
            .is_err());
        assert_eq!(d.scene().ids(), vec![id]);
        assert_eq!(h.undo_len(), 1);
    }

    #[test]
    fn failing_batch_rolls_back() {
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        let batch = Command::Batch(vec![
            Command::translate(&d, &[id], 10.0, 0.0),
            Command::Remove {
                ids: vec![ObjectId(42)],
            },
        ]);
        assert!(h.execute(&mut d, "Both", batch).is_err());
        assert_eq!(rect_of(&d, id).x, 0.0);
        assert_eq!(h.undo_len(), 1);
    }

    #[test]
    fn batch_undoes_in_reverse() {
        // Shape of "Flatten All": new image + remove every object, one undo step.
        let mut d = doc();
        let mut h = History::new();
        let (id, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        let old = d.image().bitmap.clone();
        let flat = Bitmap::filled(100, 80, Rgba::opaque(0, 0, 0)).unwrap();
        let batch = Command::Batch(vec![
            Command::SetImage(BaseImage {
                bitmap: flat.clone(),
                corner_radius: 0.0,
            }),
            Command::Remove { ids: vec![id] },
        ]);
        h.execute(&mut d, "Flatten All", batch).unwrap();
        assert!(d.image().bitmap.ptr_eq(&flat));
        assert!(d.scene().is_empty());
        h.undo(&mut d).unwrap();
        assert!(d.image().bitmap.ptr_eq(&old));
        assert_eq!(d.scene().ids(), vec![id]);
        h.redo(&mut d).unwrap();
        assert!(d.scene().is_empty());
    }

    #[test]
    fn canvas_extension_undoes() {
        let mut d = doc();
        let mut h = History::new();
        let wider = Canvas {
            rect: IRect::new(0, 0, 300, 80),
            background: Background::Color {
                color: crate::document::color::palette::CORAL,
            },
        };
        h.execute(&mut d, "Extend Canvas", Command::SetCanvas(wider))
            .unwrap();
        assert_eq!(d.canvas().rect.width, 300);
        h.undo(&mut d).unwrap();
        assert_eq!(d.canvas().rect, IRect::new(0, 0, 100, 80));
        assert_eq!(d.canvas().background, Background::Transparent);
    }

    #[test]
    fn dirty_tracking_follows_undo_and_redo() {
        let mut d = doc();
        let mut h = History::new();
        assert!(h.is_dirty(), "a new capture is unsaved");
        h.mark_saved();
        assert!(!h.is_dirty());
        let (_, cmd) = insert(&d, Rect::new(0.0, 0.0, 5.0, 5.0));
        h.execute(&mut d, "Add", cmd).unwrap();
        assert!(h.is_dirty());
        let r = h.revision();
        h.undo(&mut d).unwrap();
        assert!(!h.is_dirty(), "back at the saved state");
        h.redo(&mut d).unwrap();
        assert_eq!(h.revision(), r);
        assert!(h.is_dirty());
    }

    #[test]
    fn insert_positions_are_validated() {
        let mut d = doc();
        let id = d.scene().fresh_ids(1)[0];
        let cmd = Command::Insert {
            nodes: vec![(5, Node::new(id, TestBox::boxed(0.0, 0.0, 1.0, 1.0)))],
            resources: Vec::new(),
        };
        assert!(cmd.apply(&mut d).is_err());
        let ids = d.scene().fresh_ids(2);
        let cmd = Command::Insert {
            nodes: vec![
                (0, Node::new(ids[0], TestBox::boxed(0.0, 0.0, 1.0, 1.0))),
                (1, Node::new(ids[1], TestBox::boxed(0.0, 0.0, 1.0, 1.0))),
            ],
            resources: vec![("../evil.png".into(), d.image().bitmap.clone())],
        };
        assert!(matches!(
            cmd.apply(&mut d),
            Err(CommandError::ResourceId(_))
        ));
        assert!(d.scene().is_empty());
    }
}
