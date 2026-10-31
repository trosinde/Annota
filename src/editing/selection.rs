//! The selection (spec editor-canvas: Object selection and manipulation, Select all of a
//! type) and the box the canvas draws around it.
//!
//! The selection is editor state, not document content: it is not saved and changing it is
//! not an undo step (Ctrl+Z after a click undoes the last *change*, mockup 06: "Undo Move
//! Group"). It holds top-level object ids in stacking order; members of a group are selected
//! through their group. After undo/redo the editor calls [`Selection::sync`] so ids of objects
//! that no longer exist drop out.
//!
//! Modifiers: Shift+click *adds* (spec: "Shift+click (add)"); Ctrl+click toggles one object in
//! or out (GNOME's usual meaning of Ctrl in multi-selection). Rubber-band selection selects the
//! objects whose box lies entirely inside the band, with the same modifiers.

use crate::document::annotation::{
    box_handles, Annotation, Compass, Handle, HandleId, HandleKind, Modifiers,
};
use crate::document::geometry::{Arrange, Point, Rect};
use crate::document::registry::UnknownObject;
use crate::document::scene::{Node, ObjectId, Scene};
use crate::render::Viewport;
use crate::tools::group::Group;

/// How a click or band combines with the current selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    /// Plain click/band: the new objects become the selection.
    Replace,
    /// Shift: the new objects are added.
    Add,
    /// Ctrl: each new object is toggled in or out.
    Toggle,
}

impl SelectMode {
    pub fn from_modifiers(modifiers: Modifiers) -> Self {
        if modifiers.ctrl {
            SelectMode::Toggle
        } else if modifiers.shift {
            SelectMode::Add
        } else {
            SelectMode::Replace
        }
    }
}

/// Whether the user can select `object`: objects of unknown types (from a newer Annota) are
/// kept but neither drawn nor selectable.
pub fn is_selectable(object: &dyn Annotation) -> bool {
    !object.as_any().is::<UnknownObject>() && !object.bounds().is_none()
}

/// Selectable objects whose box (`arrange`, rotated) lies entirely inside `rect`, bottom to
/// top (rubber band).
pub fn enclosed(scene: &Scene, rect: Rect) -> Vec<ObjectId> {
    let inside = |b: Rect| {
        !b.is_none()
            && b.x >= rect.x
            && b.y >= rect.y
            && b.right() <= rect.right()
            && b.bottom() <= rect.bottom()
    };
    scene
        .nodes()
        .iter()
        .filter(|n| is_selectable(n.object.as_ref()) && inside(n.object.arrange().bounds()))
        .map(|n| n.id)
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Bottom to top.
    ids: Vec<ObjectId>,
}

impl Selection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Selected ids, bottom to top.
    pub fn ids(&self) -> &[ObjectId] {
        &self.ids
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.ids.contains(&id)
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }

    /// Selects exactly `ids`, keeping only selectable top-level objects, in stacking order.
    pub fn set(&mut self, scene: &Scene, ids: impl IntoIterator<Item = ObjectId>) {
        let mut ids: Vec<(usize, ObjectId)> = ids
            .into_iter()
            .filter(|&id| {
                scene
                    .get(id)
                    .is_some_and(|n| is_selectable(n.object.as_ref()))
            })
            .filter_map(|id| scene.position(id).map(|p| (p, id)))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        self.ids = ids.into_iter().map(|(_, id)| id).collect();
    }

    /// Drops objects that no longer exist and restores stacking order (after undo, redo,
    /// reordering or ungrouping).
    pub fn sync(&mut self, scene: &Scene) {
        let ids = std::mem::take(&mut self.ids);
        self.set(scene, ids);
    }

    fn combine(&mut self, scene: &Scene, base: &[ObjectId], new: &[ObjectId], mode: SelectMode) {
        let ids: Vec<ObjectId> = match mode {
            SelectMode::Replace => new.to_vec(),
            SelectMode::Add => base.iter().chain(new).copied().collect(),
            SelectMode::Toggle => base
                .iter()
                .filter(|id| !new.contains(id))
                .chain(new.iter().filter(|id| !base.contains(id)))
                .copied()
                .collect(),
        };
        self.set(scene, ids);
    }

    /// A click that hit `hit` (the topmost object under the pointer) or nothing. A plain click
    /// on nothing clears the selection; with Shift or Ctrl it keeps it.
    pub fn click(&mut self, scene: &Scene, hit: Option<ObjectId>, mode: SelectMode) {
        match hit {
            Some(id) => {
                let base = self.ids.clone();
                self.combine(scene, &base, &[id], mode);
            }
            None if mode == SelectMode::Replace => self.clear(),
            None => {}
        }
    }

    /// Rubber band `rect` (document space) combined with `base`, the selection when the band
    /// started.
    pub fn band(&mut self, scene: &Scene, base: &Selection, rect: Rect, mode: SelectMode) {
        let inside = enclosed(scene, rect);
        self.combine(scene, &base.ids, &inside, mode);
    }

    /// Selects every selectable object for which `predicate` holds; returns how many.
    pub fn select_where(&mut self, scene: &Scene, predicate: impl Fn(&Node) -> bool) -> usize {
        let ids: Vec<ObjectId> = scene
            .nodes()
            .iter()
            .filter(|n| predicate(n))
            .map(|n| n.id)
            .collect();
        self.set(scene, ids);
        self.len()
    }

    /// "Select All <Type>" (sidebar; spec "Recolour all arrows"): every object of the type
    /// tagged `type_name`. Returns how many.
    pub fn select_type(&mut self, scene: &Scene, type_name: &str) -> usize {
        self.select_where(scene, |n| n.object.type_name() == type_name)
    }

    /// Ctrl+A.
    pub fn select_all(&mut self, scene: &Scene) -> usize {
        self.select_where(scene, |_| true)
    }

    /// The type tag all selected objects share ("5 Arrows"), if they share one.
    pub fn common_type<'a>(&self, scene: &'a Scene) -> Option<&'a str> {
        let mut types = self
            .ids
            .iter()
            .filter_map(|&id| scene.get(id))
            .map(|n| n.object.type_name());
        let first = types.next()?;
        types.all(|t| t == first).then_some(first)
    }
}

/// Size of a square resize handle and of point handles, in screen pixels (redline: Canvas).
pub const HANDLE_SIZE: f64 = 10.0;
/// Diameter of the rotation handle, in screen pixels (redline: Canvas).
pub const ROTATE_HANDLE_SIZE: f64 = 14.0;
/// Extra grab distance around a handle, in screen pixels.
pub const HANDLE_SLACK: f64 = 3.0;
/// Diameter of arrow/line end handles and callout tail tips, in screen pixels
/// (add-core-annotation-tools redline "On-canvas handles").
pub const END_HANDLE_SIZE: f64 = 12.0;

/// What the canvas draws for the selection (redline: Canvas — selection).
#[derive(Clone, Debug)]
pub struct SelectionFrame {
    /// The box with the handles: the object's own placement for one object, the union of all
    /// boxes (unrotated) for several.
    pub frame: Arrange,
    /// Whether to draw [`SelectionFrame::frame`] (a single object whose handles are only its
    /// own points, such as an arrow's ends, draws no box).
    pub boxed: bool,
    /// Draw the box dashed: the selection is one group (mockup 06).
    pub dashed: bool,
    /// Thin boxes of the selected objects in a multi-selection, each with whether it is a
    /// group (drawn dashed).
    pub members: Vec<(Arrange, bool)>,
    pub handles: Vec<Handle>,
}

fn is_group(object: &dyn Annotation) -> bool {
    object.as_any().is::<Group>()
}

/// The frame of the selected objects, taking each from `preview` where it has an edited copy
/// (during a drag) and from the scene otherwise.
pub fn selection_frame(
    scene: &Scene,
    selection: &Selection,
    preview: &[(ObjectId, Box<dyn Annotation>)],
) -> Option<SelectionFrame> {
    let objects: Vec<&dyn Annotation> = selection
        .ids()
        .iter()
        .filter_map(|&id| {
            preview
                .iter()
                .find(|(p, _)| *p == id)
                .map(|(_, o)| o.as_ref())
                .or_else(|| scene.get(id).map(|n| n.object.as_ref()))
        })
        .collect();
    match objects.as_slice() {
        [] => None,
        [one] => {
            let handles = one.handles();
            Some(SelectionFrame {
                frame: one.arrange(),
                boxed: handles.iter().any(|h| h.kind == HandleKind::Resize),
                dashed: is_group(*one),
                members: Vec::new(),
                handles,
            })
        }
        many => {
            let union = many
                .iter()
                .fold(Rect::NONE, |acc, o| acc.union(&o.arrange().bounds()));
            let frame = Arrange::new(union, 0.0);
            Some(SelectionFrame {
                frame,
                boxed: true,
                dashed: false,
                members: many.iter().map(|o| (o.arrange(), is_group(*o))).collect(),
                handles: box_handles(&frame),
            })
        }
    }
}

/// The handles the canvas shows: on a box smaller than three handles on screen, the edge
/// handles between the corners are left out so the corners stay reachable.
pub fn visible_handles(frame: &SelectionFrame, zoom: f64) -> Vec<Handle> {
    let too_narrow = frame.frame.rect.width * zoom < 3.0 * HANDLE_SIZE;
    let too_low = frame.frame.rect.height * zoom < 3.0 * HANDLE_SIZE;
    frame
        .handles
        .iter()
        .filter(|h| match h.id {
            HandleId::Resize(Compass::N | Compass::S) => !too_narrow,
            HandleId::Resize(Compass::E | Compass::W) => !too_low,
            _ => true,
        })
        .copied()
        .collect()
}

/// Grab radius of a handle in screen pixels.
pub fn handle_radius(kind: HandleKind) -> f64 {
    let size = match kind {
        HandleKind::Rotate => ROTATE_HANDLE_SIZE,
        HandleKind::Add => 18.0,
        HandleKind::Curve | HandleKind::End | HandleKind::Tip => END_HANDLE_SIZE,
        _ => HANDLE_SIZE,
    };
    size / 2.0 + HANDLE_SLACK
}

/// The handle under `screen` (view coordinates), the nearest if several are in reach.
pub fn handle_at(frame: &SelectionFrame, viewport: &Viewport, screen: Point) -> Option<Handle> {
    visible_handles(frame, viewport.zoom)
        .into_iter()
        .map(|h| {
            let at = h.screen_position(viewport.zoom, viewport.origin);
            (h, at.distance(screen))
        })
        .filter(|(h, d)| *d <= handle_radius(h.kind))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(h, _)| h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::scene::tests::TestBox;

    fn scene() -> Scene {
        // Boxes 1..=4 at x = 0, 100, 200, 300, 50 × 50.
        let nodes = (1..=4u64)
            .map(|i| {
                Node::new(
                    ObjectId(i),
                    TestBox::boxed((i - 1) as f64 * 100.0, 0.0, 50.0, 50.0),
                )
            })
            .collect();
        Scene::try_from_nodes(nodes).unwrap()
    }

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

    #[test]
    fn click_shift_click_and_ctrl_click() {
        let scene = scene();
        let mut sel = Selection::new();
        sel.click(&scene, Some(ObjectId(3)), SelectMode::Replace);
        assert_eq!(sel.ids(), &[ObjectId(3)]);
        // Shift adds and never removes (spec: "Shift+click (add)").
        sel.click(&scene, Some(ObjectId(1)), SelectMode::from_modifiers(SHIFT));
        assert_eq!(sel.ids(), &[ObjectId(1), ObjectId(3)], "stacking order");
        sel.click(&scene, Some(ObjectId(1)), SelectMode::from_modifiers(SHIFT));
        assert_eq!(sel.len(), 2);
        // Ctrl toggles.
        sel.click(&scene, Some(ObjectId(1)), SelectMode::from_modifiers(CTRL));
        assert_eq!(sel.ids(), &[ObjectId(3)]);
        sel.click(&scene, Some(ObjectId(2)), SelectMode::from_modifiers(CTRL));
        assert_eq!(sel.ids(), &[ObjectId(2), ObjectId(3)]);
        // Clicking empty canvas: Shift keeps, a plain click clears.
        sel.click(&scene, None, SelectMode::Add);
        assert_eq!(sel.len(), 2);
        sel.click(&scene, None, SelectMode::Replace);
        assert!(sel.is_empty());
    }

    #[test]
    fn rubber_band_selects_enclosed_objects() {
        let scene = scene();
        let mut sel = Selection::new();
        let base = sel.clone();
        // Encloses boxes 2 and 3 entirely, touches box 4 only partly.
        sel.band(
            &scene,
            &base,
            Rect::new(90.0, -10.0, 230.0, 70.0),
            SelectMode::Replace,
        );
        assert_eq!(sel.ids(), &[ObjectId(2), ObjectId(3)]);
        // Shift+band adds to what was selected when the band started.
        let mut sel = Selection::new();
        sel.click(&scene, Some(ObjectId(1)), SelectMode::Replace);
        let base = sel.clone();
        sel.band(
            &scene,
            &base,
            Rect::new(290.0, -10.0, 70.0, 70.0),
            SelectMode::Add,
        );
        assert_eq!(sel.ids(), &[ObjectId(1), ObjectId(4)]);
        // Ctrl+band toggles.
        let base = sel.clone();
        sel.band(
            &scene,
            &base,
            Rect::new(-10.0, -10.0, 170.0, 70.0),
            SelectMode::Toggle,
        );
        assert_eq!(sel.ids(), &[ObjectId(2), ObjectId(4)]);
    }

    #[test]
    fn select_all_of_a_type_and_by_predicate() {
        // Spec "Select all of a type" (selection part): every object of the type, at once.
        let mut nodes: Vec<Node> = (1..=3u64)
            .map(|i| Node::new(ObjectId(i), TestBox::boxed(i as f64 * 10.0, 0.0, 5.0, 5.0)))
            .collect();
        nodes.push(Node::new(
            ObjectId(4),
            Box::new(crate::tools::image::Image::new(
                "images/1.png",
                Rect::new(0.0, 0.0, 5.0, 5.0),
            )),
        ));
        let scene = Scene::try_from_nodes(nodes).unwrap();
        let mut sel = Selection::new();
        assert_eq!(sel.select_type(&scene, "test-box"), 3);
        assert_eq!(sel.common_type(&scene), Some("test-box"));
        assert_eq!(sel.select_type(&scene, "arrow"), 0);
        assert_eq!(sel.select_all(&scene), 4);
        assert_eq!(sel.common_type(&scene), None);
        let n = sel.select_where(&scene, |n| n.object.bounds().x >= 20.0);
        assert_eq!(n, 2);
        assert_eq!(sel.ids(), &[ObjectId(2), ObjectId(3)]);
    }

    #[test]
    fn unknown_objects_are_not_selectable() {
        let unknown = crate::document::registry::UnknownObject::new("hologram", Default::default());
        let scene = Scene::try_from_nodes(vec![
            Node::new(ObjectId(1), TestBox::boxed(0.0, 0.0, 5.0, 5.0)),
            Node::new(ObjectId(2), Box::new(unknown)),
        ])
        .unwrap();
        let mut sel = Selection::new();
        assert_eq!(sel.select_all(&scene), 1);
        sel.set(&scene, [ObjectId(2), ObjectId(9)]);
        assert!(sel.is_empty());
    }

    #[test]
    fn sync_drops_vanished_objects_and_follows_reordering() {
        let mut scene = scene();
        let mut sel = Selection::new();
        sel.set(&scene, [ObjectId(4), ObjectId(2)]);
        assert_eq!(sel.ids(), &[ObjectId(2), ObjectId(4)]);
        scene
            .reorder(&[ObjectId(4), ObjectId(1), ObjectId(2), ObjectId(3)])
            .unwrap();
        scene.remove(ObjectId(2)).unwrap();
        sel.sync(&scene);
        assert_eq!(sel.ids(), &[ObjectId(4)]);
    }

    #[test]
    fn frames_for_one_several_and_a_group() {
        let scene = scene();
        let mut sel = Selection::new();
        sel.set(&scene, [ObjectId(2)]);
        let f = selection_frame(&scene, &sel, &[]).unwrap();
        assert_eq!(f.frame.rect, Rect::new(100.0, 0.0, 50.0, 50.0));
        assert!(f.boxed && !f.dashed && f.members.is_empty());
        assert_eq!(
            f.handles.len(),
            9,
            "8 resize handles and the rotation handle"
        );
        // Several: thin boxes per object and a union box with handles.
        sel.set(&scene, [ObjectId(1), ObjectId(3)]);
        let f = selection_frame(&scene, &sel, &[]).unwrap();
        assert_eq!(f.frame.rect, Rect::new(0.0, 0.0, 250.0, 50.0));
        assert_eq!(f.members.len(), 2);
        assert_eq!(f.handles.len(), 9);
        // A preview copy replaces the object.
        let moved = TestBox::boxed(500.0, 0.0, 50.0, 50.0);
        let f = selection_frame(&scene, &sel, &[(ObjectId(3), moved)]).unwrap();
        assert_eq!(f.frame.rect.right(), 550.0);
        // A group is one object with a dashed box.
        let group = Group::new(vec![Node::new(
            ObjectId(7),
            TestBox::boxed(0.0, 0.0, 5.0, 5.0),
        )]);
        let scene = Scene::try_from_nodes(vec![Node::new(ObjectId(8), Box::new(group))]).unwrap();
        sel.set(&scene, [ObjectId(8)]);
        assert!(selection_frame(&scene, &sel, &[]).unwrap().dashed);
        assert!(selection_frame(&scene, &Selection::new(), &[]).is_none());
    }

    #[test]
    fn handles_are_hit_in_screen_pixels() {
        let scene = scene();
        let mut sel = Selection::new();
        sel.set(&scene, [ObjectId(2)]);
        let f = selection_frame(&scene, &sel, &[]).unwrap();
        // 200 %: the box spans 200..300 on screen (origin 0).
        let vp = Viewport::new(2.0, Point::new(0.0, 0.0));
        let se = handle_at(&f, &vp, Point::new(303.0, 102.0)).unwrap();
        assert_eq!(se.id, HandleId::Resize(Compass::SE));
        assert!(
            handle_at(&f, &vp, Point::new(250.0, 50.0)).is_none(),
            "inside"
        );
        // The rotation handle sits 24 screen px above the top centre at every zoom.
        let rot = handle_at(&f, &vp, Point::new(250.0, -24.0)).unwrap();
        assert_eq!(rot.id, HandleId::Rotate);
        let vp = Viewport::new(0.5, Point::new(0.0, 0.0));
        let rot = handle_at(&f, &vp, Point::new(62.5, -24.0)).unwrap();
        assert_eq!(rot.id, HandleId::Rotate);
    }

    #[test]
    fn small_boxes_hide_their_edge_handles() {
        let scene = scene();
        let mut sel = Selection::new();
        sel.set(&scene, [ObjectId(1)]);
        let f = selection_frame(&scene, &sel, &[]).unwrap();
        assert_eq!(visible_handles(&f, 1.0).len(), 9);
        // 50 px at 50 % = 25 screen px: only corners and the rotation handle.
        let small = visible_handles(&f, 0.5);
        assert_eq!(small.len(), 5);
        assert!(small
            .iter()
            .all(|h| !matches!(h.id, HandleId::Resize(Compass::N | Compass::E))));
    }
}
