//! The `Annotation` trait every object type implements (CLAUDE.md §7).
//!
//! # Contract
//!
//! * **Coordinates.** Everything is in document space: one unit is one pixel of the original
//!   image, origin at its top-left corner, y pointing down, angles in degrees clockwise.
//! * **Immutability from the UI.** The UI never mutates objects that are in the document. To
//!   preview an edit it clones the object ([`AnnotationBase::clone_box`]), mutates the clone
//!   (`drag_handle`, `set_property`, `set_arrange`, `translate`) and finally submits a
//!   `Command::Replace` with the clone. Mutating methods are therefore plain `&mut self` and
//!   need no undo logic of their own.
//! * **Serialisation.** Types derive `Clone`, `serde::Serialize` and `serde::Deserialize` and
//!   are listed in [`super::registry`]. The serialised form must be a JSON object and must not
//!   use the keys `id` and `type`, which belong to the scene node. Do not use
//!   `skip_serializing_if`: keys the type does not write back are treated as unknown data from
//!   a newer Annota and preserved as they were (spec document-format: Forward compatibility).
//! * **Rendering.** `render` draws onto a cairo context whose user space is document space.
//!   It may read the rest of the document through [`RenderContext`] (embedded images, the
//!   original image for magnifiers and redactions, other objects for spotlights).
//! * **Bounds.** `bounds` covers everything `render` can touch, including stroke width and
//!   shadow; the renderer culls and the spatial index hit-tests by it.
//! * **Validity.** `Command`s refuse an object whose `bounds`/`arrange` are not finite, that
//!   uses an embedded image the document does not have, or whose serialised form does not
//!   read back (e.g. a NaN field, which JSON stores as `null`). The UI therefore does not have
//!   to guard every division by zero itself: the command fails and nothing changes.
//!
//! Default methods cover the generic parts (box handles, translate, rotation, whole-document
//! transforms), so a simple boxed type only implements geometry, rendering and its properties. New capabilities are
//! added later as default methods, which keeps existing implementations compiling.

use std::any::Any;
use std::fmt;

use serde::Serialize;

use super::color::Rgba;
use super::geometry::{normalize_degrees, Arrange, Point, Rect, Transform};
use super::scene::{Node, ObjectId};
use super::style::Shadow;
use super::Document;

/// Where a render goes. Objects may draw editing aids (e.g. the outline of an otherwise
/// invisible object) on screen only; export must never show them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Screen,
    Export,
}

/// What an object may read while it renders.
pub struct RenderContext<'a> {
    pub document: &'a Document,
    /// Device pixels per document pixel (canvas zoom, or the export scale).
    pub zoom: f64,
    pub target: Target,
}

/// Keyboard modifiers held during a drag. Their meaning is up to the type (Shift keeps the
/// ratio of a rectangle, constrains an arrow to 45°, frees the ratio of an image …).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// Compass position of a box handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compass {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Compass {
    pub const ALL: [Compass; 8] = [
        Compass::NW,
        Compass::N,
        Compass::NE,
        Compass::E,
        Compass::SE,
        Compass::S,
        Compass::SW,
        Compass::W,
    ];

    /// Unit direction from the centre: x, y ∈ {-1, 0, 1}.
    pub fn direction(self) -> (f64, f64) {
        match self {
            Compass::N => (0.0, -1.0),
            Compass::NE => (1.0, -1.0),
            Compass::E => (1.0, 0.0),
            Compass::SE => (1.0, 1.0),
            Compass::S => (0.0, 1.0),
            Compass::SW => (-1.0, 1.0),
            Compass::W => (-1.0, 0.0),
            Compass::NW => (-1.0, -1.0),
        }
    }
}

/// Identifies a handle when it is dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HandleId {
    /// One of the 8 resize handles of the selection box.
    Resize(Compass),
    /// The rotation handle above the top edge.
    Rotate,
    /// A type-specific handle (arrow end points, curve, callout tail, corner radius …);
    /// the number is the type's own.
    Custom(u32),
}

/// How a handle looks (redline: 10 × 10 squares, 14 px rotation circle, …).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleKind {
    /// Square resize handle.
    Resize,
    /// Round rotation handle on a stem.
    Rotate,
    /// Round handle on a point of the shape (arrow end, polygon vertex).
    Point,
    /// Diamond handle that bends a path.
    Curve,
    /// Filled handle that adds something when dragged (callout tail).
    Add,
    /// Filled handle that sets a corner radius.
    Radius,
}

/// Distance of the rotation handle above the top-centre, in screen pixels (redline: Canvas).
pub const ROTATE_HANDLE_GAP: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Handle {
    pub id: HandleId,
    pub kind: HandleKind,
    /// Anchor in document space.
    pub at: Point,
    /// Extra offset in *screen* pixels, applied after zooming (keeps the rotation handle
    /// 24 px above the box at every zoom level).
    pub screen_offset: Point,
}

impl Handle {
    pub fn new(id: HandleId, kind: HandleKind, at: Point) -> Self {
        Self {
            id,
            kind,
            at,
            screen_offset: Point::default(),
        }
    }

    /// Screen position for a canvas mapping `device = doc * zoom + origin`.
    pub fn screen_position(&self, zoom: f64, origin: Point) -> Point {
        Point::new(
            self.at.x * zoom + origin.x + self.screen_offset.x,
            self.at.y * zoom + origin.y + self.screen_offset.y,
        )
    }
}

/// A property value as the sidebar edits it.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Bool(bool),
    Number(f64),
    Color(Rgba),
    Text(String),
    /// One of a fixed set of choices, by stable key ("triangle", "dashed" …).
    Choice(String),
    Shadow(Shadow),
}

/// A type property (sidebar: "Type properties"). Keys are stable, lowercase, `_`-separated and
/// shared between types where the meaning is shared (`color`, `opacity`, `shadow` …), so that
/// a mixed selection can edit them together.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    pub key: &'static str,
    pub value: PropertyValue,
}

impl Property {
    pub fn new(key: &'static str, value: PropertyValue) -> Self {
        Self { key, value }
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum PropertyError {
    #[error("unknown property '{0}'")]
    Unknown(String),
    #[error("wrong value type for property '{0}'")]
    Type(String),
    #[error("value out of range for property '{0}'")]
    Range(String),
}

/// Object-safe plumbing every annotation gets for free from `Clone + Serialize`.
pub trait AnnotationBase {
    fn clone_box(&self) -> Box<dyn Annotation>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// The type's own fields as a JSON object (without `id`/`type`).
    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error>;
}

impl<T> AnnotationBase for T
where
    T: Annotation + Clone + Serialize + 'static,
{
    fn clone_box(&self) -> Box<dyn Annotation> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn to_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

pub trait Annotation: AnnotationBase + fmt::Debug {
    /// Stable serialisation tag (`"image"`, later `"arrow"`, `"text"` …); also what
    /// "Select All <Type>" groups by.
    fn type_name(&self) -> &'static str;

    /// Axis-aligned box around everything `render` draws, in document space.
    fn bounds(&self) -> Rect;

    /// Whether `point` hits the object. `tolerance` is in document units (the UI passes a few
    /// screen pixels divided by the zoom), so thin lines stay easy to grab.
    fn hit_test(&self, point: Point, tolerance: f64) -> bool;

    fn render(&self, cr: &cairo::Context, ctx: &RenderContext<'_>) -> Result<(), cairo::Error>;

    /// Generic placement for the Arrange group (X, Y, W, H, Rotation) and the selection box.
    fn arrange(&self) -> Arrange;

    /// Fits the object into a new placement (Arrange entries, box handles, group scaling).
    fn set_arrange(&mut self, arrange: Arrange);

    /// Type properties for the sidebar.
    fn properties(&self) -> Vec<Property>;

    fn set_property(&mut self, key: &str, value: &PropertyValue) -> Result<(), PropertyError>;

    /// Editing handles. The default is the selection box: 8 resize handles and a rotation
    /// handle. Types with their own geometry (arrow end points, callout tails) override this;
    /// the canvas draws each handle by its [`HandleKind`], hit-tests it in screen pixels and
    /// passes drags to [`Annotation::drag_handle`]. It draws the selection box only if the
    /// handles include [`HandleKind::Resize`] ones (an arrow can offer just its two ends).
    fn handles(&self) -> Vec<Handle> {
        box_handles(&self.arrange())
    }

    /// Moves handle `handle` so that it follows the pointer at `to`.
    ///
    /// Called on a copy of the object *as it was when the drag started* for every pointer
    /// motion, so implementations compute the new state from the start state and `to` only.
    fn drag_handle(&mut self, handle: HandleId, to: Point, modifiers: Modifiers) {
        drag_box_handle(self, handle, to, modifiers, false);
    }

    /// Moves the object; must be exact so a move by (dx, dy) and (-dx, -dy) round-trips.
    fn translate(&mut self, dx: f64, dy: f64) {
        let mut a = self.arrange();
        a.rect = a.rect.translate(dx, dy);
        self.set_arrange(a);
    }

    /// Follows a transform of the whole document — crop, rotate by 90°, flip, resize image —
    /// so the object stays on the image pixels it belongs to. `Arrange` cannot express these:
    /// a quarter turn or a mirror changes where the object is *and* how it is turned, and a
    /// resize scales positions together with sizes.
    ///
    /// The default moves a translation exactly (`translate`) and otherwise maps the object's
    /// box with [`Transform::map_arrange`]; mirrored content keeps reading the right way round
    /// (right for text and stamps; `Image` overrides this and mirrors its picture). Types whose
    /// geometry is not a box (arrow end points, pen strokes) override this and map their points
    /// with [`Transform::apply`]; types with line widths, text sizes or shadows scale those by
    /// [`Transform::style_scale`] ("Scale annotations").
    fn transform(&mut self, t: &Transform) {
        if t.is_translation() {
            self.translate(t.dx, t.dy);
        } else {
            let next = t.map_arrange(&self.arrange());
            self.set_arrange(next);
        }
    }

    /// Ids of document resources (embedded images) this object draws; they are saved with it.
    fn resources(&self) -> Vec<&str> {
        Vec::new()
    }

    /// Objects nested inside this one, bottom to top (a group's members). They are nodes with
    /// ids of their own, unique in the whole document; they are not in the scene's top level, so
    /// they cannot be selected on their own.
    fn children(&self) -> &[Node] {
        &[]
    }

    /// Ids of objects nested inside this one (a group's children, at any depth). The scene
    /// never hands out these ids again, so ungrouping cannot create duplicates. The default
    /// collects [`Annotation::children`] recursively.
    fn nested_ids(&self) -> Vec<ObjectId> {
        self.children()
            .iter()
            .flat_map(|child| std::iter::once(child.id).chain(child.object.nested_ids()))
            .collect()
    }

    /// Gives every nested object a new id from `next` (Duplicate: the copy of a group must not
    /// share its members' ids with the original). Types without children do nothing.
    fn renumber_nested(&mut self, _next: &mut dyn FnMut() -> ObjectId) {}
}

impl Clone for Box<dyn Annotation> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Smallest width/height a box can be resized to, in document pixels.
pub const MIN_SIZE: f64 = 1.0;

/// The default handle set for a box: 8 resize handles and the rotation handle.
pub fn box_handles(arrange: &Arrange) -> Vec<Handle> {
    let r = arrange.rect;
    let c = r.centre();
    let mut handles: Vec<Handle> = Compass::ALL
        .iter()
        .map(|&compass| {
            let (dx, dy) = compass.direction();
            let local = Point::new(c.x + dx * r.width / 2.0, c.y + dy * r.height / 2.0);
            Handle::new(
                HandleId::Resize(compass),
                HandleKind::Resize,
                arrange.to_document(local),
            )
        })
        .collect();
    let top = arrange.to_document(Point::new(c.x, r.y));
    let mut rotate = Handle::new(HandleId::Rotate, HandleKind::Rotate, top);
    rotate.screen_offset =
        Point::new(0.0, -ROTATE_HANDLE_GAP).rotated_around(Point::default(), arrange.rotation);
    handles.push(rotate);
    handles
}

/// Default behaviour of box handles. `keep_ratio_by_default` flips the meaning of Shift:
/// shapes keep their ratio with Shift, images keep it without (mockup 12 sidebar note).
pub fn drag_box_handle<A: Annotation + ?Sized>(
    object: &mut A,
    handle: HandleId,
    to: Point,
    modifiers: Modifiers,
    keep_ratio_by_default: bool,
) {
    match handle {
        HandleId::Resize(compass) => {
            let keep = keep_ratio_by_default != modifiers.shift;
            let next = resize_arrange(&object.arrange(), compass, to, keep);
            object.set_arrange(next);
        }
        HandleId::Rotate => {
            let next = rotate_arrange(&object.arrange(), to, modifiers.shift);
            object.set_arrange(next);
        }
        HandleId::Custom(_) => {}
    }
}

/// New placement when the `compass` handle of `start` is dragged to `to`; the opposite edge
/// or corner stays where it is, also for rotated boxes.
pub fn resize_arrange(start: &Arrange, compass: Compass, to: Point, keep_ratio: bool) -> Arrange {
    let (sx, sy) = compass.direction();
    let r = start.rect;
    let c = r.centre();
    let anchor = start.to_document(Point::new(
        c.x - sx * r.width / 2.0,
        c.y - sy * r.height / 2.0,
    ));
    // Pointer relative to the anchor, in the box's own axes.
    let v = Point::new(to.x - anchor.x, to.y - anchor.y)
        .rotated_around(Point::default(), -start.rotation);
    let mut w = if sx != 0.0 {
        (v.x * sx).max(MIN_SIZE)
    } else {
        r.width
    };
    let mut h = if sy != 0.0 {
        (v.y * sy).max(MIN_SIZE)
    } else {
        r.height
    };
    if keep_ratio && r.width > 0.0 && r.height > 0.0 {
        let scale = match (sx != 0.0, sy != 0.0) {
            (true, true) => (w / r.width).max(h / r.height),
            (true, false) => w / r.width,
            _ => h / r.height,
        };
        w = r.width * scale;
        h = r.height * scale;
    }
    let offset =
        Point::new(sx * w / 2.0, sy * h / 2.0).rotated_around(Point::default(), start.rotation);
    let centre = Point::new(anchor.x + offset.x, anchor.y + offset.y);
    Arrange::new(
        Rect::new(centre.x - w / 2.0, centre.y - h / 2.0, w, h),
        start.rotation,
    )
}

/// Rotation step while Shift is held (spec editor-canvas: Rotate snapping).
pub const ROTATION_SNAP: f64 = 15.0;

/// New placement when the rotation handle is dragged to `to`.
pub fn rotate_arrange(start: &Arrange, to: Point, snap: bool) -> Arrange {
    let c = start.rect.centre();
    // The handle sits straight above the centre at rotation 0, i.e. at atan2 = -90°.
    let mut angle = (to.y - c.y).atan2(to.x - c.x).to_degrees() + 90.0;
    if snap {
        angle = (angle / ROTATION_SNAP).round() * ROTATION_SNAP;
    }
    Arrange::new(start.rect, normalize_degrees(angle))
}

/// Helpers for `set_property` implementations.
pub mod value {
    use super::{PropertyError, PropertyValue, Rgba, Shadow};

    pub fn number(
        key: &str,
        value: &PropertyValue,
        range: std::ops::RangeInclusive<f64>,
    ) -> Result<f64, PropertyError> {
        match value {
            PropertyValue::Number(n) if n.is_finite() && range.contains(n) => Ok(*n),
            PropertyValue::Number(_) => Err(PropertyError::Range(key.to_string())),
            _ => Err(PropertyError::Type(key.to_string())),
        }
    }

    pub fn color(key: &str, value: &PropertyValue) -> Result<Rgba, PropertyError> {
        match value {
            PropertyValue::Color(c) => Ok(*c),
            _ => Err(PropertyError::Type(key.to_string())),
        }
    }

    pub fn boolean(key: &str, value: &PropertyValue) -> Result<bool, PropertyError> {
        match value {
            PropertyValue::Bool(b) => Ok(*b),
            _ => Err(PropertyError::Type(key.to_string())),
        }
    }

    pub fn shadow(key: &str, value: &PropertyValue) -> Result<Shadow, PropertyError> {
        match value {
            PropertyValue::Shadow(s)
                if (0.0..=1.0).contains(&s.opacity)
                    && s.distance.is_finite()
                    && s.angle.is_finite()
                    && s.blur.is_finite()
                    && s.blur >= 0.0 =>
            {
                Ok(*s)
            }
            PropertyValue::Shadow(_) => Err(PropertyError::Range(key.to_string())),
            _ => Err(PropertyError::Type(key.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn rect_close(a: Rect, b: Rect) -> bool {
        close(a.x, b.x) && close(a.y, b.y) && close(a.width, b.width) && close(a.height, b.height)
    }

    #[test]
    fn corner_resize_keeps_opposite_corner() {
        let start = Arrange::new(Rect::new(10.0, 10.0, 100.0, 50.0), 0.0);
        let next = resize_arrange(&start, Compass::SE, Point::new(150.0, 100.0), false);
        assert!(rect_close(next.rect, Rect::new(10.0, 10.0, 140.0, 90.0)));
        let next = resize_arrange(&start, Compass::NW, Point::new(0.0, 0.0), false);
        assert!(rect_close(next.rect, Rect::new(0.0, 0.0, 110.0, 60.0)));
    }

    #[test]
    fn edge_resize_changes_one_dimension() {
        let start = Arrange::new(Rect::new(10.0, 10.0, 100.0, 50.0), 0.0);
        let next = resize_arrange(&start, Compass::E, Point::new(200.0, 999.0), false);
        assert!(rect_close(next.rect, Rect::new(10.0, 10.0, 190.0, 50.0)));
    }

    #[test]
    fn keep_ratio_scales_both_sides() {
        let start = Arrange::new(Rect::new(0.0, 0.0, 100.0, 50.0), 0.0);
        let next = resize_arrange(&start, Compass::SE, Point::new(200.0, 60.0), true);
        assert!(rect_close(next.rect, Rect::new(0.0, 0.0, 200.0, 100.0)));
        // Edge handle with ratio: the other side grows around the centre line.
        let next = resize_arrange(&start, Compass::E, Point::new(200.0, 0.0), true);
        assert!(rect_close(next.rect, Rect::new(0.0, -25.0, 200.0, 100.0)));
    }

    #[test]
    fn resize_never_inverts() {
        let start = Arrange::new(Rect::new(0.0, 0.0, 100.0, 50.0), 0.0);
        let next = resize_arrange(&start, Compass::SE, Point::new(-50.0, -50.0), false);
        assert!(rect_close(
            next.rect,
            Rect::new(0.0, 0.0, MIN_SIZE, MIN_SIZE)
        ));
    }

    #[test]
    fn rotated_resize_keeps_anchor_in_document_space() {
        let start = Arrange::new(Rect::new(0.0, 0.0, 100.0, 50.0), 30.0);
        let anchor_before = start.to_document(Point::new(0.0, 0.0)); // NW corner
        let target = start.to_document(Point::new(160.0, 80.0));
        let next = resize_arrange(&start, Compass::SE, target, false);
        assert!(close(next.rect.width, 160.0) && close(next.rect.height, 80.0));
        let anchor_after = next.to_document(Point::new(next.rect.x, next.rect.y));
        assert!(close(anchor_before.x, anchor_after.x) && close(anchor_before.y, anchor_after.y));
        assert_eq!(next.rotation, 30.0);
    }

    #[test]
    fn rotation_follows_pointer_and_snaps() {
        let start = Arrange::new(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0);
        // Pointer to the right of the centre: a quarter turn clockwise.
        let free = rotate_arrange(&start, Point::new(200.0, 50.0), false);
        assert!(close(free.rotation, 90.0));
        // Spec scenario "Rotate snapping": with Shift the angle is a multiple of 15°.
        let p = Point::new(
            50.0 + 100.0 * 22f64.to_radians().sin(),
            50.0 - 100.0 * 22f64.to_radians().cos(),
        );
        let unsnapped = rotate_arrange(&start, p, false);
        assert!(close(unsnapped.rotation, 22.0));
        let snapped = rotate_arrange(&start, p, true);
        assert!(close(snapped.rotation, 15.0));
        for deg in [-170.0, -97.0, -8.0, 7.6, 52.0, 179.0] {
            let rad: f64 = f64::to_radians(deg);
            let p = Point::new(50.0 + 80.0 * rad.sin(), 50.0 - 80.0 * rad.cos());
            let a = rotate_arrange(&start, p, true).rotation;
            assert!(close((a / 15.0).round() * 15.0, a), "{deg} -> {a}");
        }
    }

    #[test]
    fn box_handles_sit_on_the_rotated_box() {
        let a = Arrange::new(Rect::new(0.0, 0.0, 100.0, 50.0), 90.0);
        let handles = box_handles(&a);
        assert_eq!(handles.len(), 9);
        let n = handles
            .iter()
            .find(|h| h.id == HandleId::Resize(Compass::N))
            .unwrap();
        // Rotated a quarter turn clockwise around (50, 25): the top edge now faces right.
        assert!(close(n.at.x, 75.0) && close(n.at.y, 25.0));
        let rotate = handles.iter().find(|h| h.id == HandleId::Rotate).unwrap();
        assert!(close(rotate.screen_offset.x, ROTATE_HANDLE_GAP));
        assert!(close(rotate.screen_offset.y, 0.0));
        let screen = rotate.screen_position(2.0, Point::new(10.0, 10.0));
        assert!(close(screen.x, 75.0 * 2.0 + 10.0 + ROTATE_HANDLE_GAP));
    }

    #[test]
    fn value_helpers_check_type_and_range() {
        assert_eq!(
            value::number("w", &PropertyValue::Number(3.0), 0.0..=10.0),
            Ok(3.0)
        );
        assert_eq!(
            value::number("w", &PropertyValue::Number(11.0), 0.0..=10.0),
            Err(PropertyError::Range("w".into()))
        );
        assert_eq!(
            value::number("w", &PropertyValue::Bool(true), 0.0..=10.0),
            Err(PropertyError::Type("w".into()))
        );
        assert!(value::number("w", &PropertyValue::Number(f64::NAN), 0.0..=10.0).is_err());
    }
}
