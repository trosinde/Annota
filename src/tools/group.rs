//! Groups (spec editor-canvas: Z-order and grouping; mockups 05, 06): several objects that
//! are selected, moved, resized, rotated and stacked as one.
//!
//! A group is an ordinary object type whose state is its members, kept as scene nodes with
//! their own ids (unique in the whole document, see `Scene::owner`). It draws its members
//! bottom to top, is hit wherever a member is hit, and its selection box is the tight box
//! around the members in the group's own orientation: [`Group::rotation`] is the only geometry
//! the group stores itself, so the box can never disagree with the members. Moving the group
//! moves every member by exactly the same amount; resizing and rotating map every member's
//! placement from the old box to the new one.
//!
//! Grouping and ungrouping are one undo step each ([`group_command`], [`ungroup_command`]):
//! the members keep their ids, so ungrouping brings back exactly the objects that were grouped.
//!
//! A group that contains an object this version does not know (from a newer Annota) is not
//! read as a `Group` at all but kept verbatim as an unknown object (`registry`), so it is never
//! moved without the parts this version cannot move.

use serde::de::Error as _;
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::document::annotation::{
    Annotation, Property, PropertyError, PropertyValue, RenderContext,
};
use crate::document::command::Command;
use crate::document::geometry::{normalize_degrees, Arrange, Point, Rect};
use crate::document::registry::{self, UnknownObject};
use crate::document::scene::{Node, ObjectId};
use crate::document::Document;

#[derive(Clone, Debug)]
pub struct Group {
    /// Orientation of the group's box, degrees clockwise (0 when grouped; changes when the
    /// group is rotated as a whole).
    pub rotation: f64,
    /// The members, bottom to top.
    pub children: Vec<Node>,
}

impl Group {
    pub const TYPE: &'static str = "group";

    pub fn new(children: Vec<Node>) -> Self {
        Self {
            rotation: 0.0,
            children,
        }
    }

    /// The tight box around the members' boxes, in the group's orientation.
    fn frame(&self) -> Arrange {
        let rotation = self.rotation;
        let origin = Point::default();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for child in &self.children {
            let a = child.object.arrange();
            for corner in a.rect.corners() {
                // Corner in document space, then in the group's rotated axes.
                let p = a.to_document(corner).rotated_around(origin, -rotation);
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
        }
        if x0 > x1 || y0 > y1 {
            return Arrange::new(Rect::NONE, rotation);
        }
        let (w, h) = (x1 - x0, y1 - y0);
        // The box's centre back in document space; an `Arrange` rotates around its centre.
        let c = Point::new(x0 + w / 2.0, y0 + h / 2.0).rotated_around(origin, rotation);
        Arrange::new(Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h), rotation)
    }
}

impl Serialize for Group {
    /// `{"rotation": 0, "children": [<node>, …]}`; children are written like top-level nodes.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let children = self
            .children
            .iter()
            .map(registry::node_to_json)
            .collect::<Result<Vec<_>, _>>()
            .map_err(S::Error::custom)?;
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("rotation", &self.rotation)?;
        map.serialize_entry("children", &children)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for Group {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            rotation: f64,
            children: Vec<Value>,
        }
        let raw = Raw::deserialize(deserializer)?;
        if raw.children.is_empty() {
            return Err(D::Error::custom("a group needs at least one object"));
        }
        let children = raw
            .children
            .into_iter()
            .map(|child| registry::node_from_json(child, false))
            .collect::<Result<Vec<_>, _>>()
            .map_err(D::Error::custom)?;
        Ok(Self {
            rotation: raw.rotation,
            children,
        })
    }
}

impl Annotation for Group {
    fn type_name(&self) -> &'static str {
        Self::TYPE
    }

    fn bounds(&self) -> Rect {
        self.children
            .iter()
            .fold(Rect::NONE, |acc, c| acc.union(&c.object.bounds()))
    }

    fn hit_test(&self, point: Point, tolerance: f64) -> bool {
        self.children
            .iter()
            .any(|c| c.object.hit_test(point, tolerance))
    }

    fn render(&self, cr: &cairo::Context, ctx: &RenderContext<'_>) -> Result<(), cairo::Error> {
        for child in &self.children {
            cr.save()?;
            let drawn = child.object.render(cr, ctx);
            cr.restore()?;
            drawn?;
        }
        Ok(())
    }

    fn arrange(&self) -> Arrange {
        self.frame()
    }

    /// Maps every member from the current box to `next`: its centre keeps its relative place
    /// in the box, its size scales with the box (along the member's own axes, exact for
    /// uniform scaling and for members turned by multiples of 90° against the group), and it
    /// turns with the box.
    fn set_arrange(&mut self, next: Arrange) {
        let old = self.frame();
        if old.rect.is_none() || next.rect.is_none() {
            return;
        }
        let ratio = |new: f64, old: f64| if old > 0.0 { new / old } else { 1.0 };
        let (sx, sy) = (
            ratio(next.rect.width, old.rect.width),
            ratio(next.rect.height, old.rect.height),
        );
        let turn = next.rotation - old.rotation;
        for child in &mut self.children {
            let a = child.object.arrange();
            let local = old.to_local(a.rect.centre());
            let fraction = |v: f64, start: f64, size: f64| {
                if size > 0.0 {
                    (v - start) / size
                } else {
                    0.5
                }
            };
            let u = fraction(local.x, old.rect.x, old.rect.width);
            let v = fraction(local.y, old.rect.y, old.rect.height);
            let centre = next.to_document(Point::new(
                next.rect.x + u * next.rect.width,
                next.rect.y + v * next.rect.height,
            ));
            let (sin, cos) = (a.rotation - old.rotation).to_radians().sin_cos();
            let w = a.rect.width * (sx * cos).hypot(sy * sin);
            let h = a.rect.height * (sx * sin).hypot(sy * cos);
            child.object.set_arrange(Arrange::new(
                Rect::new(centre.x - w / 2.0, centre.y - h / 2.0, w, h),
                normalize_degrees(a.rotation + turn),
            ));
        }
        self.rotation = normalize_degrees(next.rotation);
    }

    /// Exact: every member moves by the same amount.
    fn translate(&mut self, dx: f64, dy: f64) {
        for child in &mut self.children {
            child.object.translate(dx, dy);
        }
    }

    /// The sidebar shows the member list and Ungroup instead (mockup 06).
    fn properties(&self) -> Vec<Property> {
        Vec::new()
    }

    fn set_property(&mut self, key: &str, _: &PropertyValue) -> Result<(), PropertyError> {
        Err(PropertyError::Unknown(key.to_string()))
    }

    fn resources(&self) -> Vec<&str> {
        self.children
            .iter()
            .flat_map(|c| c.object.resources())
            .collect()
    }

    fn children(&self) -> &[Node] {
        &self.children
    }

    fn renumber_nested(&mut self, next: &mut dyn FnMut() -> ObjectId) {
        for child in &mut self.children {
            child.id = next();
            child.object.renumber_nested(next);
        }
    }
}

/// Whether `object` can be a group member: an object this version knows.
fn groupable(object: &dyn Annotation) -> bool {
    !object.as_any().is::<UnknownObject>()
}

/// Groups the objects `ids` (Ctrl+G) as one undo step: they leave the scene and come back as
/// the members of a new group, in their stacking order and with their ids, where the topmost
/// of them was. Returns the group's id and the command, or `None` for fewer than two objects
/// or objects that cannot be grouped.
pub fn group_command(doc: &Document, ids: &[ObjectId]) -> Option<(ObjectId, Command)> {
    let scene = doc.scene();
    let mut members: Vec<(usize, &Node)> = ids
        .iter()
        .filter_map(|&id| Some((scene.position(id)?, scene.get(id)?)))
        .collect();
    members.sort_by_key(|(position, _)| *position);
    members.dedup_by_key(|(position, _)| *position);
    if members.len() < 2 || !members.iter().all(|(_, n)| groupable(n.object.as_ref())) {
        return None;
    }
    let top = members.last().map(|(position, _)| *position)?;
    // Everything below the topmost member that is not a member stays below the group.
    let position = top + 1 - members.len();
    let group_id = *scene.fresh_ids(1).first()?;
    let children: Vec<Node> = members.iter().map(|(_, n)| (*n).clone()).collect();
    let removed = children.iter().map(|n| n.id).collect();
    let command = Command::Batch(vec![
        Command::Remove { ids: removed },
        Command::Insert {
            nodes: vec![(
                position,
                Node::new(group_id, Box::new(Group::new(children))),
            )],
            resources: Vec::new(),
        },
    ]);
    Some((group_id, command))
}

/// Ungroups every group among `ids` (Ctrl+Shift+G) as one undo step: each group is replaced by
/// its members, at its place in the stacking order, with their ids. Returns the members' ids
/// (the new selection) and the command, or `None` if no group is among `ids`.
pub fn ungroup_command(doc: &Document, ids: &[ObjectId]) -> Option<(Vec<ObjectId>, Command)> {
    let scene = doc.scene();
    let group_of = |node: &Node| -> Option<Group> {
        ids.contains(&node.id)
            .then(|| node.object.as_any().downcast_ref::<Group>().cloned())
            .flatten()
    };
    let mut groups = Vec::new();
    let mut inserts = Vec::new();
    let mut members = Vec::new();
    // Final stacking position of every object after ungrouping, bottom to top.
    let mut position = 0;
    for node in scene.nodes() {
        match group_of(node) {
            Some(group) => {
                groups.push(node.id);
                for child in group.children {
                    members.push(child.id);
                    inserts.push((position, child));
                    position += 1;
                }
            }
            None => position += 1,
        }
    }
    if groups.is_empty() {
        return None;
    }
    let command = Command::Batch(vec![
        Command::Remove { ids: groups },
        Command::Insert {
            nodes: inserts,
            resources: Vec::new(),
        },
    ]);
    Some((members, command))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::annotation::{Compass, HandleId, Modifiers};
    use crate::document::archive;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::{palette, Rgba};
    use crate::document::command::{CommandError, History};
    use crate::document::scene::SceneError;
    use crate::tools::image::Image;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn rect_close(a: Rect, b: Rect) -> bool {
        close(a.x, b.x) && close(a.y, b.y) && close(a.width, b.width) && close(a.height, b.height)
    }

    /// A document with `n` image objects (registered type, so files are real) laid out in a
    /// row: object k at (100 k + 10, 20), 40 × 30, no border or shadow.
    fn doc_with_images(n: usize) -> (Document, History, Vec<ObjectId>) {
        let mut doc = Document::new(Bitmap::filled(800, 400, Rgba::WHITE).unwrap());
        let mut history = History::new();
        for k in 0..n {
            let pip = Bitmap::filled(40, 30, palette::CORAL).unwrap();
            let cmd = Image::insert_command(&doc, pip, Point::new(0.0, 0.0));
            history.execute(&mut doc, "Insert", cmd).unwrap();
            let id = *doc.scene().ids().last().unwrap();
            let mut object = doc.scene().get(id).unwrap().object.clone();
            let img = object.as_any_mut().downcast_mut::<Image>().unwrap();
            img.border_width = 0.0;
            img.shadow.enabled = false;
            img.x = 100.0 * k as f64 + 10.0;
            img.y = 20.0;
            history
                .execute(
                    &mut doc,
                    "Place",
                    Command::Replace {
                        objects: vec![(id, object)],
                    },
                )
                .unwrap();
        }
        let ids = doc.scene().ids();
        (doc, history, ids)
    }

    fn arrange_of(doc: &Document, id: ObjectId) -> Arrange {
        doc.scene().get(id).unwrap().object.arrange()
    }

    fn group_of(doc: &Document, id: ObjectId) -> &Group {
        doc.scene()
            .get(id)
            .unwrap()
            .object
            .as_any()
            .downcast_ref::<Group>()
            .unwrap()
    }

    fn image_node(id: u64, x: f64) -> Node {
        Node::new(
            ObjectId(id),
            Box::new(Image::new("images/1.png", Rect::new(x, 0.0, 10.0, 10.0))),
        )
    }

    #[test]
    fn box_is_the_union_of_the_members() {
        let group = Group::new(vec![image_node(1, 0.0), image_node(2, 50.0)]);
        assert_eq!(
            group.arrange(),
            Arrange::new(Rect::new(0.0, 0.0, 60.0, 10.0), 0.0)
        );
        assert!(group.hit_test(Point::new(55.0, 5.0), 0.0));
        assert!(
            !group.hit_test(Point::new(30.0, 5.0), 0.0),
            "the gap is not the group"
        );
        assert_eq!(group.nested_ids(), vec![ObjectId(1), ObjectId(2)]);
    }

    #[test]
    fn group_moves_together() {
        // Spec "Group moves together" (model part): moving the group moves every member by
        // the same amount, exactly.
        let mut group = Group::new(vec![image_node(1, 0.0), image_node(2, 50.0)]);
        group.translate(560.0, 330.0);
        let xs: Vec<f64> = group
            .children
            .iter()
            .map(|c| c.object.arrange().rect.x)
            .collect();
        assert_eq!(xs, vec![560.0, 610.0]);
        assert_eq!(group.arrange().rect, Rect::new(560.0, 330.0, 60.0, 10.0));
    }

    #[test]
    fn resizing_scales_the_members() {
        let mut group = Group::new(vec![image_node(1, 0.0), image_node(2, 50.0)]);
        group.drag_handle(
            HandleId::Resize(Compass::E),
            Point::new(120.0, 5.0),
            Modifiers::default(),
        );
        assert!(rect_close(
            group.arrange().rect,
            Rect::new(0.0, 0.0, 120.0, 10.0)
        ));
        let rects: Vec<Rect> = group
            .children
            .iter()
            .map(|c| c.object.arrange().rect)
            .collect();
        assert!(rect_close(rects[0], Rect::new(0.0, 0.0, 20.0, 10.0)));
        assert!(rect_close(rects[1], Rect::new(100.0, 0.0, 20.0, 10.0)));
    }

    #[test]
    fn rotating_turns_members_around_the_centre_and_keeps_the_box() {
        let mut group = Group::new(vec![image_node(1, 0.0), image_node(2, 50.0)]);
        let before = group.arrange();
        // Rotation handle dragged straight right of the centre (30, 5): a quarter turn.
        group.drag_handle(
            HandleId::Rotate,
            Point::new(200.0, 5.0),
            Modifiers::default(),
        );
        let after = group.arrange();
        assert!(close(after.rotation, 90.0));
        // The box turned in place: same size and centre.
        assert!(rect_close(after.rect, before.rect), "{after:?}");
        let first = group.children[0].object.arrange();
        assert!(close(first.rotation, 90.0));
        // Member 1's centre (5, 5) turned a quarter around (30, 5): (30, -20).
        let c = first.rect.centre();
        assert!(close(c.x, 30.0) && close(c.y, -20.0), "{c:?}");
        // Turning back restores the members.
        group.drag_handle(
            HandleId::Rotate,
            Point::new(30.0, -200.0),
            Modifiers::default(),
        );
        assert!(close(group.arrange().rotation, 0.0));
        let c = group.children[0].object.arrange().rect;
        assert!(rect_close(c, Rect::new(0.0, 0.0, 10.0, 10.0)), "{c:?}");
    }

    #[test]
    fn resizing_a_rotated_group_works_in_its_own_axes() {
        let mut group = Group::new(vec![image_node(1, 0.0), image_node(2, 50.0)]);
        group.set_arrange(Arrange::new(group.arrange().rect, 90.0));
        let start = group.arrange();
        // Double the width along the group's own x axis (which now points down).
        let mut next = start;
        next.rect.width *= 2.0;
        next.rect.x -= start.rect.width / 2.0;
        group.set_arrange(next);
        let after = group.arrange();
        assert!(close(after.rotation, 90.0));
        assert!(
            close(after.rect.width, 120.0) && close(after.rect.height, 10.0),
            "{after:?}"
        );
        // Members keep their rotation and grow along the group's axis.
        for c in &group.children {
            let a = c.object.arrange();
            assert!(close(a.rotation, 90.0));
            assert!(
                close(a.rect.width, 20.0) && close(a.rect.height, 10.0),
                "{a:?}"
            );
        }
    }

    #[test]
    fn group_and_ungroup_keep_ids_order_and_undo() {
        let (mut doc, mut history, ids) = doc_with_images(4);
        // Group objects 1 and 3 (positions 0 and 2); object 2 stays between, below the group.
        let (gid, cmd) = group_command(&doc, &[ids[2], ids[0]]).unwrap();
        assert!(!ids.contains(&gid));
        history.execute(&mut doc, "Group", cmd).unwrap();
        assert_eq!(doc.scene().ids(), vec![ids[1], gid, ids[3]]);
        let members: Vec<ObjectId> = group_of(&doc, gid).children.iter().map(|c| c.id).collect();
        assert_eq!(members, vec![ids[0], ids[2]]);
        // Members are known to the scene, but not as top-level objects.
        assert_eq!(doc.scene().owner(ids[0]), Some(gid));
        assert!(doc.scene().get(ids[0]).is_none());

        let (back, cmd) = ungroup_command(&doc, &[gid]).unwrap();
        assert_eq!(back, vec![ids[0], ids[2]]);
        history.execute(&mut doc, "Ungroup", cmd).unwrap();
        assert_eq!(doc.scene().ids(), vec![ids[1], ids[0], ids[2], ids[3]]);
        assert_eq!(arrange_of(&doc, ids[2]).rect.x, 210.0);

        history.undo(&mut doc).unwrap();
        assert_eq!(doc.scene().ids(), vec![ids[1], gid, ids[3]]);
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.scene().ids(), ids);
        assert_eq!(doc.scene().owner(ids[0]), Some(ids[0]));
        history.redo(&mut doc).unwrap();
        assert_eq!(doc.scene().ids(), vec![ids[1], gid, ids[3]]);
    }

    #[test]
    fn grouping_needs_two_known_objects() {
        let (doc, _, ids) = doc_with_images(2);
        assert!(group_command(&doc, &[ids[0]]).is_none());
        assert!(group_command(&doc, &[ids[0], ids[0]]).is_none());
        assert!(group_command(&doc, &[ids[0], ObjectId(999)]).is_none());
        assert!(ungroup_command(&doc, &ids).is_none(), "no group selected");
    }

    #[test]
    fn group_and_ungroup_never_reuse_or_duplicate_ids() {
        // Regression for the phase-1 nested-id fix, against the real Group type.
        let (mut doc, mut history, ids) = doc_with_images(3);
        let (gid, cmd) = group_command(&doc, &ids[..2]).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        // New objects never get a member's id, nor the group's.
        let fresh = doc.scene().fresh_ids(3);
        assert!(fresh.iter().all(|f| !ids.contains(f) && *f != gid));
        let pip = Bitmap::filled(5, 5, palette::LIME).unwrap();
        let insert = Image::insert_command(&doc, pip, Point::new(300.0, 300.0));
        history.execute(&mut doc, "Insert", insert).unwrap();
        let new_id = *doc.scene().ids().last().unwrap();
        assert!(!ids.contains(&new_id) && new_id != gid);

        // Ungroup, then the members are top-level again and still unique.
        let (_, cmd) = ungroup_command(&doc, &[gid]).unwrap();
        history.execute(&mut doc, "Ungroup", cmd).unwrap();
        let mut all = doc.scene().ids();
        let n = all.len();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), n);
        // The group's id is not handed out again either.
        assert!(!doc.scene().fresh_ids(5).contains(&gid));

        // A saved and reopened document knows the members' ids.
        let (gid, cmd) = group_command(&doc, &[ids[0], ids[2]]).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        let reopened = archive::decode(&archive::encode(&doc).unwrap()).unwrap();
        assert_eq!(
            archive::document_json(&reopened).unwrap(),
            archive::document_json(&doc).unwrap()
        );
        assert_eq!(reopened.scene().owner(ids[2]), Some(gid));
        let fresh = reopened.scene().fresh_ids(1)[0];
        assert!(
            fresh.0
                > ids
                    .iter()
                    .chain([&gid, &new_id])
                    .map(|i| i.0)
                    .max()
                    .unwrap()
        );
        // Spec "Group ids survive saving": ungrouped after reopening, the members are back
        // with their ids and unchanged, and new ids stay new.
        let mut reopened = reopened;
        let mut h = History::new();
        let (members, cmd) = ungroup_command(&reopened, &[gid]).unwrap();
        assert_eq!(members, vec![ids[0], ids[2]]);
        h.execute(&mut reopened, "Ungroup", cmd).unwrap();
        for id in [ids[0], ids[2]] {
            let before = group_of(&doc, gid)
                .children
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .object
                .arrange();
            assert_eq!(arrange_of(&reopened, id), before);
        }
        assert_eq!(reopened.scene().fresh_ids(1)[0], fresh);
    }

    #[test]
    fn groups_cannot_take_ids_that_exist() {
        let (mut doc, mut history, ids) = doc_with_images(2);
        let (gid, cmd) = group_command(&doc, &ids).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        let other = doc.scene().fresh_ids(1)[0];
        let child = |id: ObjectId| {
            Node::new(
                id,
                Box::new(Image::new("images/1.png", Rect::new(0.0, 0.0, 5.0, 5.0))),
            )
        };
        // Insert: a group whose member has the id of another group's member.
        let stolen = Command::Insert {
            nodes: vec![(
                1,
                Node::new(other, Box::new(Group::new(vec![child(ids[0])]))),
            )],
            resources: Vec::new(),
        };
        assert_eq!(
            history.execute(&mut doc, "Insert", stolen),
            Err(CommandError::Scene(SceneError::Duplicate(ids[0])))
        );
        // Insert: a member repeating the group's own id, or another member's.
        let selfish = Command::Insert {
            nodes: vec![(
                1,
                Node::new(other, Box::new(Group::new(vec![child(other)]))),
            )],
            resources: Vec::new(),
        };
        assert!(history.execute(&mut doc, "Insert", selfish).is_err());
        let twice = ObjectId(other.0 + 1);
        let twins = Command::Insert {
            nodes: vec![(
                1,
                Node::new(
                    other,
                    Box::new(Group::new(vec![child(twice), child(twice)])),
                ),
            )],
            resources: Vec::new(),
        };
        assert!(history.execute(&mut doc, "Insert", twins).is_err());
        // Replace: a group may keep its members' ids but not take the group's own id.
        let mut changed = group_of(&doc, gid).clone();
        changed.children[1].id = gid;
        assert!(history
            .execute(
                &mut doc,
                "Edit",
                Command::Replace {
                    objects: vec![(gid, Box::new(changed))]
                }
            )
            .is_err());
        let mut moved = group_of(&doc, gid).clone();
        moved.translate(5.0, 0.0);
        history
            .execute(
                &mut doc,
                "Move",
                Command::Replace {
                    objects: vec![(gid, Box::new(moved))],
                },
            )
            .unwrap();
        assert_eq!(doc.scene().owner(ids[1]), Some(gid));
    }

    #[test]
    fn files_with_duplicate_member_ids_are_refused() {
        let (mut doc, mut history, ids) = doc_with_images(3);
        let (_, cmd) = group_command(&doc, &ids[..2]).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        let mut json = archive::document_json(&doc).unwrap();
        // The group is at the bottom; a member takes the id of the top-level object above.
        assert_eq!(json["objects"][0]["type"], "group");
        json["objects"][0]["children"][0]["id"] = serde_json::json!(ids[2].0);
        let objects: Vec<Node> = json["objects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| registry::node_from_json(v.clone(), false).unwrap())
            .collect();
        assert!(crate::document::scene::Scene::try_from_nodes(objects).is_err());
    }

    #[test]
    fn json_shape_and_round_trip() {
        let group = Group::new(vec![image_node(3, 0.0), image_node(4, 20.0)]);
        let node = Node::new(ObjectId(9), Box::new(group));
        let json = registry::node_to_json(&node).unwrap();
        assert_eq!(json["type"], "group");
        assert_eq!(json["rotation"], 0.0);
        assert_eq!(json["children"][1]["id"], 4);
        assert_eq!(json["children"][1]["type"], "image");
        let back = registry::node_from_json(json.clone(), false).unwrap();
        assert!(back.residual.is_none());
        assert_eq!(registry::node_to_json(&back).unwrap(), json);
        // Unknown keys inside members survive.
        let mut extended = json.clone();
        extended["children"][0]["glow"] = serde_json::json!(3);
        extended["sticky"] = serde_json::json!(true);
        let back = registry::node_from_json(extended.clone(), false).unwrap();
        assert_eq!(registry::node_to_json(&back).unwrap(), extended);
        // An empty group is not a group.
        let empty = serde_json::json!({"id": 1, "type": "group", "rotation": 0, "children": []});
        assert!(registry::node_from_json(empty, false).is_err());
    }

    #[test]
    fn group_with_an_unknown_member_is_kept_verbatim() {
        let value = serde_json::json!({"id": 5, "type": "group", "rotation": 0, "children": [
            {"id": 6, "type": "image", "source": "images/1.png", "x": 0, "y": 0, "width": 4,
             "height": 4, "rotation": 0, "opacity": 1, "border_color": "#FFFFFFFF",
             "border_width": 0, "corner_radius": 0, "shadow": {"enabled": false,
             "color": "#000006FF", "opacity": 0.4, "angle": 52, "distance": 10, "blur": 8}},
            {"id": 7, "type": "hologram", "source": "images/2.png"}
        ]});
        let node = registry::node_from_json(value.clone(), false).unwrap();
        let unknown = node
            .object
            .as_any()
            .downcast_ref::<UnknownObject>()
            .unwrap();
        assert_eq!(unknown.type_tag, "group");
        assert_eq!(node.object.nested_ids(), vec![ObjectId(6), ObjectId(7)]);
        assert_eq!(registry::node_to_json(&node).unwrap(), value);
    }

    #[test]
    fn group_renders_like_its_members() {
        // A group draws exactly what its members draw on their own.
        let (mut doc, mut history, ids) = doc_with_images(3);
        let ungrouped = crate::render::flatten(&doc, 1.0).unwrap();
        let (_, cmd) = group_command(&doc, &ids[..2]).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        let grouped = crate::render::flatten(&doc, 1.0).unwrap();
        for y in 0..grouped.height() {
            for x in 0..grouped.width() {
                assert_eq!(grouped.pixel(x, y), ungrouped.pixel(x, y), "{x},{y}");
            }
        }
    }
}
