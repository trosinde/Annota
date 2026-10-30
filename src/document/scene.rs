//! The scene graph: annotation objects in stacking order, with a spatial index.
//!
//! Objects are kept bottom-to-top in a `Vec`. Every object's bounds are cached and kept in an
//! R*-tree (`rstar`); above [`INDEX_THRESHOLD`] objects, hit-testing and render culling ask the
//! tree for candidates instead of scanning (design.md: spatial index beyond 100 objects).
//! Groups (`crate::tools::group`) are an object type whose children are nodes of their own:
//! their ids are unique in the whole document and tracked here ([`Scene::owner`]), but only
//! top-level nodes are in the stacking order, the spatial index and the selection.
//!
//! The scene is changed only by `Command`s (`super::command`); its mutators are crate-private.

use std::collections::HashMap;

use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};
use serde::{Deserialize, Serialize};

use super::annotation::Annotation;
use super::geometry::{Point, Rect};
use super::residual::Residual;

/// Above this many objects, queries use the R-tree.
pub const INDEX_THRESHOLD: usize = 100;

/// Stable identity of an object within a document; never reused within a session. Ids are
/// unique across the whole document, including objects nested inside other objects (groups),
/// so ungrouping never produces a duplicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectId(pub u64);

/// One object in the scene.
#[derive(Clone, Debug)]
pub struct Node {
    pub id: ObjectId,
    pub object: Box<dyn Annotation>,
    /// What a newer Annota wrote for this node that this version does not know (keys at any
    /// depth); written back unchanged. It stays with the node when the object is replaced.
    pub(crate) residual: Option<Residual>,
}

impl Node {
    pub fn new(id: ObjectId, object: Box<dyn Annotation>) -> Self {
        Self {
            id,
            object,
            residual: None,
        }
    }

    /// This node's id and the ids of every object nested in it.
    pub(crate) fn all_ids(&self) -> impl Iterator<Item = ObjectId> {
        std::iter::once(self.id).chain(self.object.nested_ids())
    }
}

/// The id after the largest one used by `node`, including nested objects.
fn next_after(node: &Node) -> u64 {
    node.all_ids()
        .map(|id| id.0.saturating_add(1))
        .max()
        .unwrap_or(1)
}

type Entry = GeomWithData<Rectangle<[f64; 2]>, ObjectId>;

fn entry(id: ObjectId, r: &Rect) -> Option<Entry> {
    if r.is_none() {
        return None;
    }
    Some(GeomWithData::new(
        Rectangle::from_corners([r.x, r.y], [r.right(), r.bottom()]),
        id,
    ))
}

fn envelope(r: &Rect) -> AABB<[f64; 2]> {
    AABB::from_corners([r.x, r.y], [r.right(), r.bottom()])
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SceneError {
    #[error("object {0:?} does not exist")]
    Unknown(ObjectId),
    #[error("object {0:?} exists already")]
    Duplicate(ObjectId),
    #[error("index {0} is out of range")]
    Index(usize),
    #[error("the new order is not a permutation of the objects")]
    NotAPermutation,
}

pub struct Scene {
    nodes: Vec<Node>,
    /// Cached `bounds()` per node, parallel to `nodes`.
    bounds: Vec<Rect>,
    positions: HashMap<ObjectId, usize>,
    /// Nested id → the top-level node that contains it.
    nested: HashMap<ObjectId, ObjectId>,
    tree: RTree<Entry>,
    next_id: u64,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("nodes", &self.nodes)
            .finish_non_exhaustive()
    }
}

impl Clone for Scene {
    fn clone(&self) -> Self {
        let mut scene = Scene::from_nodes(self.nodes.clone());
        scene.next_id = self.next_id;
        scene
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::from_nodes(Vec::new())
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a scene from nodes in stacking order (load). Duplicate ids, also among nested
    /// objects, are rejected.
    pub(crate) fn try_from_nodes(nodes: Vec<Node>) -> Result<Self, SceneError> {
        let mut seen = std::collections::HashSet::new();
        for id in nodes.iter().flat_map(Node::all_ids) {
            if !seen.insert(id) {
                return Err(SceneError::Duplicate(id));
            }
        }
        Ok(Self::from_nodes(nodes))
    }

    fn from_nodes(nodes: Vec<Node>) -> Self {
        let bounds: Vec<Rect> = nodes.iter().map(|n| n.object.bounds()).collect();
        let entries: Vec<Entry> = nodes
            .iter()
            .zip(&bounds)
            .filter_map(|(n, b)| entry(n.id, b))
            .collect();
        let next_id = nodes.iter().map(next_after).max().unwrap_or(1).max(1);
        let nested = nodes
            .iter()
            .flat_map(|n| n.object.nested_ids().into_iter().map(move |id| (id, n.id)))
            .collect();
        let mut scene = Self {
            nodes,
            bounds,
            positions: HashMap::new(),
            nested,
            tree: RTree::bulk_load(entries),
            next_id,
        };
        scene.reindex_positions(0);
        scene
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nodes bottom to top.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn get(&self, id: ObjectId) -> Option<&Node> {
        self.position(id).and_then(|i| self.nodes.get(i))
    }

    /// Stacking position (0 = bottom).
    pub fn position(&self, id: ObjectId) -> Option<usize> {
        self.positions.get(&id).copied()
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.nodes.iter().map(|n| n.id).collect()
    }

    /// The top-level object that is or contains `id` (a group for one of its members).
    pub fn owner(&self, id: ObjectId) -> Option<ObjectId> {
        if self.positions.contains_key(&id) {
            Some(id)
        } else {
            self.nested.get(&id).copied()
        }
    }

    /// Whether any object in the document, top-level or nested, has `id`.
    pub fn contains_id(&self, id: ObjectId) -> bool {
        self.owner(id).is_some()
    }

    /// The first id of `object`'s nested objects that would collide with another object if
    /// `object` became (or replaced) the top-level node `id`: one that repeats inside `object`,
    /// is `id` itself, or belongs to any other object.
    pub(crate) fn nested_conflict(
        &self,
        id: ObjectId,
        object: &dyn Annotation,
    ) -> Option<ObjectId> {
        let mut seen = std::collections::HashSet::new();
        object
            .nested_ids()
            .into_iter()
            .find(|&n| n == id || !seen.insert(n) || self.owner(n).is_some_and(|o| o != id))
    }

    /// `n` ids no object has had in this session — neither a top-level nor a nested one — for
    /// `Command::Insert`.
    pub fn fresh_ids(&self, n: usize) -> Vec<ObjectId> {
        (0..n as u64).map(|i| ObjectId(self.next_id + i)).collect()
    }

    /// Cached bounds of the node at `position`.
    pub fn bounds_at(&self, position: usize) -> Option<Rect> {
        self.bounds.get(position).copied()
    }

    /// Every object of one type, bottom to top ("Select All <Type>").
    pub fn of_type(&self, type_name: &str) -> Vec<ObjectId> {
        self.nodes
            .iter()
            .filter(|n| n.object.type_name() == type_name)
            .map(|n| n.id)
            .collect()
    }

    /// Positions (bottom to top) of the objects whose bounds touch `rect`.
    pub fn query(&self, rect: &Rect) -> Vec<usize> {
        if rect.is_none() {
            return Vec::new();
        }
        if self.nodes.len() <= INDEX_THRESHOLD {
            return self
                .bounds
                .iter()
                .enumerate()
                .filter(|(_, b)| b.intersects(rect))
                .map(|(i, _)| i)
                .collect();
        }
        let mut hits: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(envelope(rect))
            .filter_map(|e| self.position(e.data))
            .collect();
        hits.sort_unstable();
        hits
    }

    /// The topmost object under `point`. `tolerance` is in document units.
    pub fn hit_test(&self, point: Point, tolerance: f64) -> Option<ObjectId> {
        let probe = Rect::new(point.x, point.y, 0.0, 0.0).inflate(tolerance.max(0.0));
        self.query(&probe)
            .into_iter()
            .rev()
            .filter_map(|i| self.nodes.get(i))
            .find(|n| n.object.hit_test(point, tolerance))
            .map(|n| n.id)
    }

    // ---- mutation (commands only) ----

    fn reindex_positions(&mut self, from: usize) {
        for (i, n) in self.nodes.iter().enumerate().skip(from) {
            self.positions.insert(n.id, i);
        }
    }

    pub(crate) fn insert(&mut self, position: usize, node: Node) -> Result<(), SceneError> {
        if self.contains_id(node.id) {
            return Err(SceneError::Duplicate(node.id));
        }
        if let Some(id) = self.nested_conflict(node.id, node.object.as_ref()) {
            return Err(SceneError::Duplicate(id));
        }
        if position > self.nodes.len() {
            return Err(SceneError::Index(position));
        }
        let b = node.object.bounds();
        if let Some(e) = entry(node.id, &b) {
            self.tree.insert(e);
        }
        self.next_id = self.next_id.max(next_after(&node));
        for nested in node.object.nested_ids() {
            self.nested.insert(nested, node.id);
        }
        self.nodes.insert(position, node);
        self.bounds.insert(position, b);
        self.reindex_positions(position);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: ObjectId) -> Result<(usize, Node), SceneError> {
        let position = self.position(id).ok_or(SceneError::Unknown(id))?;
        let node = self.nodes.remove(position);
        let b = self.bounds.remove(position);
        if let Some(e) = entry(id, &b) {
            self.tree.remove(&e);
        }
        self.positions.remove(&id);
        for nested in node.object.nested_ids() {
            self.nested.remove(&nested);
        }
        self.reindex_positions(position);
        Ok((position, node))
    }

    pub(crate) fn replace(
        &mut self,
        id: ObjectId,
        object: Box<dyn Annotation>,
    ) -> Result<Box<dyn Annotation>, SceneError> {
        let position = self.position(id).ok_or(SceneError::Unknown(id))?;
        if let Some(conflict) = self.nested_conflict(id, object.as_ref()) {
            return Err(SceneError::Duplicate(conflict));
        }
        let new_bounds = object.bounds();
        let (Some(node), Some(cached)) =
            (self.nodes.get_mut(position), self.bounds.get_mut(position))
        else {
            return Err(SceneError::Unknown(id));
        };
        if let Some(e) = entry(id, cached) {
            self.tree.remove(&e);
        }
        if let Some(e) = entry(id, &new_bounds) {
            self.tree.insert(e);
        }
        *cached = new_bounds;
        let previous = std::mem::replace(&mut node.object, object);
        self.next_id = self.next_id.max(next_after(node));
        for nested in previous.nested_ids() {
            self.nested.remove(&nested);
        }
        for nested in node.object.nested_ids() {
            self.nested.insert(nested, id);
        }
        Ok(previous)
    }

    /// Puts the objects into `order` (bottom to top); returns the previous order.
    pub(crate) fn reorder(&mut self, order: &[ObjectId]) -> Result<Vec<ObjectId>, SceneError> {
        if !self.is_permutation(order) {
            return Err(SceneError::NotAPermutation);
        }
        let previous = self.ids();
        let mut slots: Vec<Option<(Node, Rect)>> = std::mem::take(&mut self.nodes)
            .into_iter()
            .zip(std::mem::take(&mut self.bounds))
            .map(Some)
            .collect();
        for id in order {
            let taken = self
                .positions
                .get(id)
                .and_then(|&i| slots.get_mut(i))
                .and_then(Option::take);
            if let Some((node, b)) = taken {
                self.nodes.push(node);
                self.bounds.push(b);
            }
        }
        self.reindex_positions(0);
        Ok(previous)
    }

    pub(crate) fn is_permutation(&self, order: &[ObjectId]) -> bool {
        if order.len() != self.nodes.len() {
            return false;
        }
        let mut seen = std::collections::HashSet::with_capacity(order.len());
        order
            .iter()
            .all(|id| self.positions.contains_key(id) && seen.insert(*id))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::document::annotation::{Property, PropertyError, PropertyValue, RenderContext};
    use crate::document::geometry::Arrange;

    /// Minimal annotation for scene, command and renderer tests: a filled box.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct TestBox {
        pub rect: Rect,
        pub value: f64,
    }

    impl TestBox {
        pub fn boxed(x: f64, y: f64, w: f64, h: f64) -> Box<dyn Annotation> {
            Box::new(TestBox {
                rect: Rect::new(x, y, w, h),
                value: 0.0,
            })
        }
    }

    impl Annotation for TestBox {
        fn type_name(&self) -> &'static str {
            "test-box"
        }
        fn bounds(&self) -> Rect {
            self.rect
        }
        fn hit_test(&self, p: Point, tolerance: f64) -> bool {
            self.rect.inflate(tolerance).contains(p)
        }
        fn render(&self, cr: &cairo::Context, _: &RenderContext<'_>) -> Result<(), cairo::Error> {
            cr.rectangle(self.rect.x, self.rect.y, self.rect.width, self.rect.height);
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.fill()
        }
        fn arrange(&self) -> Arrange {
            Arrange::new(self.rect, 0.0)
        }
        fn set_arrange(&mut self, a: Arrange) {
            self.rect = a.rect;
        }
        fn properties(&self) -> Vec<Property> {
            vec![Property::new("value", PropertyValue::Number(self.value))]
        }
        fn set_property(&mut self, key: &str, v: &PropertyValue) -> Result<(), PropertyError> {
            match key {
                "value" => {
                    self.value =
                        crate::document::annotation::value::number(key, v, f64::MIN..=f64::MAX)?;
                    Ok(())
                }
                _ => Err(PropertyError::Unknown(key.into())),
            }
        }
    }

    fn scene_with(n: u64) -> Scene {
        let nodes = (1..=n)
            .map(|i| {
                let f = i as f64;
                Node::new(ObjectId(i), TestBox::boxed(f * 10.0, f * 10.0, 15.0, 15.0))
            })
            .collect();
        Scene::try_from_nodes(nodes).unwrap()
    }

    #[test]
    fn topmost_object_wins_the_hit() {
        let scene = scene_with(3);
        // (30..45) overlaps boxes 2 (20..35) and 3 (30..45).
        assert_eq!(
            scene.hit_test(Point::new(32.0, 32.0), 0.0),
            Some(ObjectId(3))
        );
        assert_eq!(
            scene.hit_test(Point::new(22.0, 22.0), 0.0),
            Some(ObjectId(2))
        );
        assert_eq!(scene.hit_test(Point::new(0.0, 0.0), 0.0), None);
        // Tolerance reaches a box that is just out of reach otherwise.
        assert_eq!(scene.hit_test(Point::new(8.0, 8.0), 3.0), Some(ObjectId(1)));
    }

    #[test]
    fn indexed_and_linear_queries_agree() {
        // 400 objects: above the threshold the R-tree answers; compare against a scan.
        let mut nodes = Vec::new();
        for i in 0..400u64 {
            let x = ((i * 37) % 1000) as f64;
            let y = ((i * 91) % 700) as f64;
            nodes.push(Node::new(ObjectId(i + 1), TestBox::boxed(x, y, 40.0, 25.0)));
        }
        let scene = Scene::try_from_nodes(nodes).unwrap();
        assert!(scene.len() > INDEX_THRESHOLD);
        for probe in [
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(500.0, 300.0, 1.0, 1.0),
            Rect::new(-50.0, -50.0, 2000.0, 2000.0),
        ] {
            let linear: Vec<usize> = (0..scene.len())
                .filter(|&i| scene.bounds_at(i).unwrap().intersects(&probe))
                .collect();
            assert_eq!(scene.query(&probe), linear);
        }
        for p in [
            Point::new(510.0, 310.0),
            Point::new(37.0, 91.0),
            Point::new(-5.0, -5.0),
        ] {
            let linear = scene
                .nodes()
                .iter()
                .rev()
                .find(|n| n.object.hit_test(p, 0.0))
                .map(|n| n.id);
            assert_eq!(scene.hit_test(p, 0.0), linear);
        }
    }

    #[test]
    fn index_follows_mutations() {
        let mut nodes = Vec::new();
        for i in 0..150u64 {
            nodes.push(Node::new(
                ObjectId(i + 1),
                TestBox::boxed(i as f64, 0.0, 1.0, 1.0),
            ));
        }
        let mut scene = Scene::try_from_nodes(nodes).unwrap();
        let target = ObjectId(10);
        assert_eq!(scene.hit_test(Point::new(9.5, 0.5), 0.0), Some(target));
        scene
            .replace(target, TestBox::boxed(5000.0, 5000.0, 10.0, 10.0))
            .unwrap();
        assert_eq!(scene.hit_test(Point::new(9.5, 0.5), 0.0), None);
        assert_eq!(
            scene.hit_test(Point::new(5005.0, 5005.0), 0.0),
            Some(target)
        );
        let (pos, node) = scene.remove(target).unwrap();
        assert_eq!(pos, 9);
        assert_eq!(scene.hit_test(Point::new(5005.0, 5005.0), 0.0), None);
        scene.insert(0, node).unwrap();
        assert_eq!(scene.position(target), Some(0));
        assert_eq!(
            scene.hit_test(Point::new(5005.0, 5005.0), 0.0),
            Some(target)
        );
        assert_eq!(scene.position(ObjectId(1)), Some(1));
    }

    #[test]
    fn reorder_requires_a_permutation() {
        let mut scene = scene_with(3);
        let ids = [ObjectId(3), ObjectId(1), ObjectId(2)];
        assert_eq!(
            scene.reorder(&ids).unwrap(),
            vec![ObjectId(1), ObjectId(2), ObjectId(3)]
        );
        assert_eq!(scene.ids(), ids.to_vec());
        assert_eq!(scene.position(ObjectId(2)), Some(2));
        assert_eq!(
            scene.reorder(&[ObjectId(1), ObjectId(1), ObjectId(2)]),
            Err(SceneError::NotAPermutation)
        );
        assert_eq!(
            scene.reorder(&[ObjectId(1)]),
            Err(SceneError::NotAPermutation)
        );
    }

    #[test]
    fn ids_are_not_reused() {
        let mut scene = scene_with(2);
        assert_eq!(scene.fresh_ids(2), vec![ObjectId(3), ObjectId(4)]);
        scene.remove(ObjectId(2)).unwrap();
        assert_eq!(scene.fresh_ids(1), vec![ObjectId(3)]);
        assert_eq!(
            scene.insert(
                0,
                Node::new(ObjectId(1), TestBox::boxed(0.0, 0.0, 1.0, 1.0))
            ),
            Err(SceneError::Duplicate(ObjectId(1)))
        );
        assert!(Scene::try_from_nodes(vec![
            Node::new(ObjectId(1), TestBox::boxed(0.0, 0.0, 1.0, 1.0)),
            Node::new(ObjectId(1), TestBox::boxed(0.0, 0.0, 1.0, 1.0)),
        ])
        .is_err());
    }

    #[test]
    fn nested_ids_are_never_handed_out() {
        // Reviewer finding: only top-level ids were considered, so a new object could take
        // the id of a group's child and collide after ungrouping.
        let container = |id: u64, child: u64| {
            let fields = serde_json::json!({"children": [{"id": child, "type": "arrow"}]});
            let serde_json::Value::Object(fields) = fields else {
                unreachable!()
            };
            Node::new(
                ObjectId(id),
                Box::new(crate::document::registry::UnknownObject::new(
                    "stack", fields,
                )),
            )
        };
        let mut scene = Scene::try_from_nodes(vec![
            Node::new(ObjectId(1), TestBox::boxed(0.0, 0.0, 1.0, 1.0)),
            container(2, 40),
        ])
        .unwrap();
        assert_eq!(scene.fresh_ids(1), vec![ObjectId(41)]);
        scene.insert(0, container(50, 90)).unwrap();
        assert_eq!(scene.fresh_ids(1), vec![ObjectId(91)]);
        // A nested id that repeats a top-level one makes the file invalid.
        assert_eq!(
            Scene::try_from_nodes(vec![
                Node::new(ObjectId(40), TestBox::boxed(0.0, 0.0, 1.0, 1.0)),
                container(2, 40),
            ])
            .err(),
            Some(SceneError::Duplicate(ObjectId(40)))
        );
    }

    #[test]
    fn select_all_of_a_type() {
        let scene = scene_with(4);
        assert_eq!(scene.of_type("test-box").len(), 4);
        assert!(scene.of_type("arrow").is_empty());
    }
}
