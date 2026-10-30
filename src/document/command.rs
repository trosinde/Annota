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
//!
//! Every command is validated before it changes anything. Besides structural checks (ids,
//! positions, permutations) this keeps the document saveable and reopenable at all times: an
//! inserted or replacing object must have finite geometry, may only use embedded images the
//! document (or the same `Insert`) provides, and must read back from its own JSON.

use std::collections::{BTreeSet, HashSet, VecDeque};

use super::annotation::Annotation;
use super::bitmap::Bitmap;
use super::geometry::Rect;
use super::registry;
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
    /// E.g. a size computed by dividing by zero.
    #[error("object {0:?} has a position, size or rotation that is not a finite number")]
    NonFinite(ObjectId),
    #[error(
        "object {object:?} uses the embedded image '{resource}', which the document does not have"
    )]
    MissingResource { object: ObjectId, resource: String },
    #[error("object {object:?} could not be saved and reopened: {reason}")]
    Unsaveable { object: ObjectId, reason: String },
    /// A step of a `Batch` failed, and undoing the steps before it failed as well: the
    /// document may be left partly changed.
    #[error("{error}; undoing the partial change also failed: {rollback}")]
    Rollback {
        error: Box<CommandError>,
        rollback: Box<CommandError>,
    },
}

fn finite_rect(r: &Rect) -> bool {
    r.x.is_finite() && r.y.is_finite() && r.width.is_finite() && r.height.is_finite()
}

/// Finite `bounds()` and `arrange()`, also of nested objects (also checked when a file is
/// read). A group's bounds skip children whose bounds are not finite, so each child is checked
/// on its own.
pub(crate) fn check_geometry(id: ObjectId, object: &dyn Annotation) -> Result<(), CommandError> {
    let arrange = object.arrange();
    if !(finite_rect(&object.bounds())
        && finite_rect(&arrange.rect)
        && arrange.rotation.is_finite())
    {
        return Err(CommandError::NonFinite(id));
    }
    object
        .children()
        .iter()
        .try_for_each(|child| check_geometry(child.id, child.object.as_ref()))
}

/// Everything an object must satisfy to enter the document; `has_resource` says whether an
/// embedded image will exist once the command is applied.
fn check_object(
    id: ObjectId,
    object: &dyn Annotation,
    has_resource: impl Fn(&str) -> bool,
) -> Result<(), CommandError> {
    check_geometry(id, object)?;
    if let Some(missing) = object.resources().into_iter().find(|r| !has_resource(r)) {
        return Err(CommandError::MissingResource {
            object: id,
            resource: missing.to_string(),
        });
    }
    registry::check_reads_back(object).map_err(|err| CommandError::Unsaveable {
        object: id,
        reason: err.to_string(),
    })
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

    /// Applies the command and returns its inverse. On error the document is unchanged
    /// (except for [`CommandError::Rollback`]).
    ///
    /// Crate-private: outside the model, every change goes through [`History::execute`], so
    /// nothing can bypass undo/redo and dirty tracking (CLAUDE.md §7).
    pub(crate) fn apply(self, doc: &mut Document) -> Result<Command, CommandError> {
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
                let resources_before: BTreeSet<String> =
                    doc.resources().ids().map(str::to_string).collect();
                let mut inverses: Vec<Command> = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => return Err(roll_back(doc, inverses, &resources_before, err)),
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
                // Ids are unique in the whole document, nested objects (group members)
                // included: a group must not bring ids that exist already.
                no_repeats(nodes.iter().flat_map(|(_, n)| n.all_ids()))?;
                for (k, (position, node)) in nodes.iter().enumerate() {
                    if let Some(id) = node.all_ids().find(|&id| scene.contains_id(id)) {
                        return Err(SceneError::Duplicate(id).into());
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
                let has_resource = |r: &str| {
                    doc.resources().contains(r) || resources.iter().any(|(id, _)| id == r)
                };
                for (_, node) in nodes {
                    check_object(node.id, node.object.as_ref(), has_resource)?;
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
                if let Some((id, _)) = objects.iter().find(|(id, _)| scene.position(*id).is_none())
                {
                    return Err(SceneError::Unknown(*id).into());
                }
                // A replacement may keep its own nested ids but not take anybody else's.
                no_repeats(objects.iter().flat_map(|(_, o)| o.nested_ids()))?;
                for (id, object) in objects {
                    if let Some(conflict) = scene.nested_conflict(*id, object.as_ref()) {
                        return Err(SceneError::Duplicate(conflict).into());
                    }
                }
                for (id, object) in objects {
                    check_object(*id, object.as_ref(), |r| doc.resources().contains(r))?;
                }
                Ok(())
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

/// Undoes the applied part of a failed `Batch`, newest first, and drops the embedded images
/// it added. Returns the error to report: `error`, or [`CommandError::Rollback`] if undoing
/// failed too (every inverse is still attempted).
fn roll_back(
    doc: &mut Document,
    inverses: Vec<Command>,
    resources_before: &BTreeSet<String>,
    error: CommandError,
) -> CommandError {
    let mut failed = None;
    for inverse in inverses.into_iter().rev() {
        if let Err(err) = inverse.apply(doc) {
            failed.get_or_insert(err);
        }
    }
    let added: Vec<String> = doc
        .resources()
        .ids()
        .filter(|id| !resources_before.contains(*id))
        .map(str::to_string)
        .collect();
    for id in added {
        doc.resources_mut().remove(&id);
    }
    match failed {
        None => error,
        Some(rollback) => CommandError::Rollback {
            error: Box::new(error),
            rollback: Box::new(rollback),
        },
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

    /// A document with one image object (registered type, so it is saved for real).
    fn doc_with_image() -> (Document, History, ObjectId) {
        let mut d = doc();
        let mut h = History::new();
        let pip = Bitmap::filled(20, 10, Rgba::opaque(9, 9, 9)).unwrap();
        let cmd = crate::tools::image::Image::insert_command(&d, pip, Point::new(50.0, 40.0));
        h.execute(&mut d, "Insert", cmd).unwrap();
        let id = d.scene().ids()[0];
        (d, h, id)
    }

    fn edited_image(
        d: &Document,
        id: ObjectId,
        edit: impl FnOnce(&mut crate::tools::image::Image),
    ) -> Command {
        let mut object = d.scene().get(id).unwrap().object.clone();
        edit(
            object
                .as_any_mut()
                .downcast_mut::<crate::tools::image::Image>()
                .unwrap(),
        );
        Command::Replace {
            objects: vec![(id, object)],
        }
    }

    /// Saves and reopens, also through an autosave slot (crash recovery path).
    fn assert_saves_and_reopens(d: &Document) {
        use crate::document::archive;
        use crate::document::autosave::{AutosaveId, Store};
        let back = archive::decode(&archive::encode(d).unwrap()).unwrap();
        assert_eq!(
            archive::document_json(&back).unwrap(),
            archive::document_json(d).unwrap()
        );
        let dir = std::env::temp_dir().join(format!(
            "annota-cmd-{}-{}",
            std::process::id(),
            AutosaveId::generate().as_str()
        ));
        let store = Store::new(&dir);
        let id = AutosaveId::generate();
        store.write(&id, d, None).unwrap();
        let entry = store.list().entries.pop().unwrap();
        store.restore(&entry).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn nan_coordinate_is_rejected() {
        // Reviewer repro (B3): Replace with a NaN coordinate used to be accepted, saved as
        // `null`, and then neither the file nor its autosave reopened.
        let (mut d, mut h, id) = doc_with_image();
        let before = archive_json(&d);
        let nan = edited_image(&d, id, |img| img.x = f64::NAN);
        assert_eq!(
            h.execute(&mut d, "Move", nan),
            Err(CommandError::NonFinite(id))
        );
        // Division by zero, e.g. setting H on something 0 px high: infinite scale.
        let inf = edited_image(&d, id, |img| {
            let scale = 30.0 / 0.0;
            img.width *= scale;
        });
        assert_eq!(
            h.execute(&mut d, "Resize", inf),
            Err(CommandError::NonFinite(id))
        );
        // Non-geometric NaN: serialises as `null`, which does not read back.
        let opacity = edited_image(&d, id, |img| img.opacity = f64::NAN);
        assert!(matches!(
            h.execute(&mut d, "Opacity", opacity),
            Err(CommandError::Unsaveable { object, .. }) if object == id
        ));
        // Also inside a batch, and on insert.
        let batch = Command::Batch(vec![edited_image(&d, id, |img| img.y = f64::INFINITY)]);
        assert!(h.execute(&mut d, "Batch", batch).is_err());
        let (_, insert) = insert(&d, Rect::new(f64::NAN, 0.0, 1.0, 1.0));
        assert!(matches!(
            h.execute(&mut d, "Add", insert),
            Err(CommandError::NonFinite(_))
        ));
        assert_eq!(archive_json(&d), before, "nothing changed");
        assert_eq!(h.undo_len(), 1);
        assert_saves_and_reopens(&d);
    }

    #[test]
    fn dangling_resource_reference_is_rejected() {
        // Reviewer repro (B3): pointing an object at a missing embedded image used to be
        // accepted, after which every save and autosave failed.
        let (mut d, mut h, id) = doc_with_image();
        let dangling = edited_image(&d, id, |img| img.source = "images/77.png".into());
        assert_eq!(
            h.execute(&mut d, "Replace", dangling),
            Err(CommandError::MissingResource {
                object: id,
                resource: "images/77.png".into()
            })
        );
        // An insert must bring the images it uses unless the document has them.
        let new_id = d.scene().fresh_ids(1)[0];
        let object = crate::tools::image::Image::new("images/5.png", Rect::new(0.0, 0.0, 4.0, 4.0));
        let node = || Node::new(new_id, Box::new(object.clone()));
        let without = Command::Insert {
            nodes: vec![(1, node())],
            resources: Vec::new(),
        };
        assert!(matches!(
            h.execute(&mut d, "Add", without),
            Err(CommandError::MissingResource { .. })
        ));
        let with = Command::Insert {
            nodes: vec![(1, node())],
            resources: vec![("images/5.png".into(), d.image().bitmap.clone())],
        };
        h.execute(&mut d, "Add", with).unwrap();
        // An existing embedded image may be shared.
        let shared = edited_image(&d, id, |img| img.source = "images/5.png".into());
        h.execute(&mut d, "Share", shared).unwrap();
        assert_saves_and_reopens(&d);
    }

    #[test]
    fn failed_batch_drops_the_images_it_added() {
        // Reviewer finding: a rolled-back Insert left its embedded image behind (1 → 2).
        let (mut d, mut h, _) = doc_with_image();
        assert_eq!(d.resources().len(), 1);
        let pip = Bitmap::filled(5, 5, Rgba::WHITE).unwrap();
        let batch = Command::Batch(vec![
            crate::tools::image::Image::insert_command(&d, pip, Point::new(10.0, 10.0)),
            Command::Remove {
                ids: vec![ObjectId(424242)],
            },
        ]);
        assert!(h.execute(&mut d, "Both", batch).is_err());
        assert_eq!(d.resources().len(), 1);
        assert_eq!(d.object_count(), 1);
        assert_eq!(h.undo_len(), 1);
    }

    fn archive_json(d: &Document) -> serde_json::Value {
        crate::document::archive::document_json(d).unwrap()
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
