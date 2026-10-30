//! Operations on the whole document (spec editor-canvas: Image operations, Canvas extension,
//! Flatten; mockups 08–10, 17, 18): crop, resize, rotate by 90°, flip, extend the canvas, its
//! background, the image's rounded corners, and burning objects into the image.
//!
//! Each returns an [`Edit`] — one `Command::Batch`, one undo step — or `None` if it would not
//! change anything. Crop, resize, rotate and flip map the image, the canvas and every object
//! through one [`Transform`] (objects through `Annotation::transform`), so annotations keep
//! their place on the image. Objects of types this version does not know (from a newer Annota)
//! cannot follow such a transform, and leaving them where they are would put them on the wrong
//! pixels. So while a document has any ([`has_unknown_objects`]), crop, rotate, flip, resize
//! image and flatten refuse (return `None`); the editor disables them and says why. Extending the
//! canvas, its background and the corner radius move nothing and stay available.

use crate::document::annotation::Annotation;
use crate::document::bitmap::{Bitmap, MAX_SIDE};
use crate::document::command::Command;
use crate::document::geometry::{IRect, Mirror, Transform};
use crate::document::registry::UnknownObject;
use crate::document::scene::ObjectId;
use crate::document::{Background, BaseImage, Canvas, Document};
use crate::render;

use super::ops::{Action, Edit, Subject};
use super::selection::{is_selectable, Selection};

/// Which way Rotate turns the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// Rotate Left (Ctrl+Shift+R): a quarter turn counter-clockwise.
    Left,
    /// Rotate Right (Ctrl+R): a quarter turn clockwise.
    Right,
}

/// The original image's rectangle in document space (always at the origin).
pub fn image_rect(doc: &Document) -> IRect {
    let bitmap = &doc.image().bitmap;
    IRect::new(0, 0, bitmap.width(), bitmap.height())
}

/// Whether the document has objects of a type this version does not know (kept verbatim from a
/// newer Annota; a group with such a member is one of them). The operations that re-base or move
/// every object — crop, rotate, flip, resize image, flatten — are unavailable then.
pub fn has_unknown_objects(doc: &Document) -> bool {
    doc.scene()
        .nodes()
        .iter()
        .any(|n| n.object.as_any().is::<UnknownObject>())
}

/// Every object this version can move, transformed by `t`.
fn transformed_objects(doc: &Document, t: &Transform) -> Vec<(ObjectId, Box<dyn Annotation>)> {
    doc.scene()
        .nodes()
        .iter()
        .filter(|n| !n.object.as_any().is::<UnknownObject>())
        .map(|n| {
            let mut object = n.object.clone();
            object.transform(t);
            (n.id, object)
        })
        .collect()
}

/// Image, canvas and objects through `t`; `image` is the already transformed bitmap.
fn transform_document(doc: &Document, t: &Transform, image: BaseImage) -> Command {
    let new_image = IRect::new(0, 0, image.bitmap.width(), image.bitmap.height());
    let canvas = Canvas {
        // The canvas never ends up smaller than the image (rounding of a resize).
        rect: t.map_irect(doc.canvas().rect).union(&new_image),
        background: doc.canvas().background,
    };
    let mut steps = vec![Command::SetImage(image), Command::SetCanvas(canvas)];
    let objects = transformed_objects(doc, t);
    if !objects.is_empty() {
        steps.push(Command::Replace { objects });
    }
    Command::Batch(steps)
}

fn edit(doc: &Document, action: Action, command: Command) -> Edit {
    Edit {
        action,
        subject: Subject::Many(doc.object_count()),
        command,
        select: None,
    }
}

/// Rotate Left / Rotate Right: the image, the canvas and every object turn a quarter around the
/// image.
pub fn rotate(doc: &Document, turn: Turn) -> Option<Edit> {
    if has_unknown_objects(doc) {
        return None;
    }
    let image = doc.image();
    let (w, h) = (image.bitmap.width(), image.bitmap.height());
    let t = match turn {
        Turn::Right => Transform {
            quarter_turns: 1,
            dx: f64::from(h),
            ..Transform::IDENTITY
        },
        Turn::Left => Transform {
            quarter_turns: 3,
            dy: f64::from(w),
            ..Transform::IDENTITY
        },
    };
    let bitmap = image.bitmap.transformed(&t, h, w).ok()?;
    let command = transform_document(
        doc,
        &t,
        BaseImage {
            bitmap,
            corner_radius: image.corner_radius,
        },
    );
    let action = match turn {
        Turn::Left => Action::RotateLeft,
        Turn::Right => Action::RotateRight,
    };
    Some(edit(doc, action, command))
}

/// Flip Horizontally / Flip Vertically: mirrored at the image's centre line. Objects move to
/// their mirrored place with their angle mirrored (see `Transform::map_rotation`); image objects
/// mirror their picture too, other types keep their content readable.
pub fn flip(doc: &Document, mirror: Mirror) -> Option<Edit> {
    if has_unknown_objects(doc) {
        return None;
    }
    let image = doc.image();
    let (w, h) = (image.bitmap.width(), image.bitmap.height());
    let (t, action) = match mirror {
        Mirror::Horizontal => (
            Transform {
                mirror: Some(Mirror::Horizontal),
                dx: f64::from(w),
                ..Transform::IDENTITY
            },
            Action::FlipHorizontally,
        ),
        Mirror::Vertical => (
            Transform {
                mirror: Some(Mirror::Vertical),
                dy: f64::from(h),
                ..Transform::IDENTITY
            },
            Action::FlipVertically,
        ),
    };
    let bitmap = image.bitmap.transformed(&t, w, h).ok()?;
    let command = transform_document(
        doc,
        &t,
        BaseImage {
            bitmap,
            corner_radius: image.corner_radius,
        },
    );
    Some(edit(doc, action, command))
}

/// Resize Image (mockup 09): the image is resampled to `width` × `height`; the canvas and the
/// objects scale with it, so annotations stay on what they point at. With `scale_annotations`
/// line widths, text sizes, borders and shadows scale too (by the mean of both factors);
/// without, they keep their size in pixels.
pub fn resize(doc: &Document, width: i32, height: i32, scale_annotations: bool) -> Option<Edit> {
    if has_unknown_objects(doc) {
        return None;
    }
    let image = doc.image();
    let (w, h) = (image.bitmap.width(), image.bitmap.height());
    if !(1..=MAX_SIDE).contains(&width) || !(1..=MAX_SIDE).contains(&height) {
        return None;
    }
    if (width, height) == (w, h) {
        return None;
    }
    let (sx, sy) = (
        f64::from(width) / f64::from(w),
        f64::from(height) / f64::from(h),
    );
    let t = Transform {
        scale_x: sx,
        scale_y: sy,
        style_scale: if scale_annotations {
            (sx * sy).sqrt()
        } else {
            1.0
        },
        ..Transform::IDENTITY
    };
    let bitmap = image.bitmap.transformed(&t, width, height).ok()?;
    let corner_radius = image.corner_radius * sx.min(sy);
    let command = transform_document(
        doc,
        &t,
        BaseImage {
            bitmap,
            corner_radius,
        },
    );
    Some(edit(doc, Action::ResizeImage, command))
}

/// Whether `rect` is a crop that does something: inside the canvas, smaller than it, and
/// keeping at least one pixel of the image (and no unknown objects in the document).
pub fn can_crop(doc: &Document, rect: IRect) -> bool {
    let canvas = doc.canvas().rect;
    !has_unknown_objects(doc)
        && rect.width > 0
        && rect.height > 0
        && canvas.intersection(&rect) == Some(rect)
        && rect != canvas
        && rect.intersection(&image_rect(doc)).is_some()
}

/// Crop (mockup 08): the canvas becomes `rect`, the image keeps only its part inside it, and
/// the document is re-based so the image starts at the origin again. Objects keep their place
/// on the image; those outside the new canvas stay in the document (out of view).
pub fn crop(doc: &Document, rect: IRect) -> Option<Edit> {
    if !can_crop(doc, rect) {
        return None;
    }
    let image = doc.image();
    let whole = image_rect(doc);
    let kept = rect.intersection(&whole)?;
    let t = Transform::translation(-f64::from(kept.x), -f64::from(kept.y));
    let mut steps = Vec::new();
    if kept != whole {
        let bitmap = image
            .bitmap
            .cropped(kept.x, kept.y, kept.width, kept.height)
            .ok()?;
        steps.push(Command::SetImage(BaseImage {
            bitmap,
            corner_radius: image.corner_radius,
        }));
    }
    steps.push(Command::SetCanvas(Canvas {
        rect: rect.translate(-kept.x, -kept.y),
        background: doc.canvas().background,
    }));
    if kept.x != 0 || kept.y != 0 {
        let objects = transformed_objects(doc, &t);
        if !objects.is_empty() {
            steps.push(Command::Replace { objects });
        }
    }
    Some(edit(doc, Action::Crop, Command::Batch(steps)))
}

/// Canvas extension (mockup 10): the canvas becomes `rect`; the image and objects stay where
/// they are (the canvas may start left of or above the image).
pub fn set_canvas_rect(doc: &Document, rect: IRect) -> Option<Edit> {
    let canvas = doc.canvas();
    if rect == canvas.rect || rect.width < 1 || rect.height < 1 {
        return None;
    }
    let command = Command::SetCanvas(Canvas {
        rect,
        background: canvas.background,
    });
    Some(edit(doc, Action::ExtendCanvas, command))
}

/// The canvas background (sidebar: Canvas › Background).
pub fn set_background(doc: &Document, background: Background) -> Option<Edit> {
    let canvas = doc.canvas();
    if canvas.background == background {
        return None;
    }
    let command = Command::SetCanvas(Canvas {
        rect: canvas.rect,
        background,
    });
    Some(edit(doc, Action::CanvasBackground, command))
}

/// The largest useful corner radius: half the image's shorter side.
pub fn max_corner_radius(doc: &Document) -> f64 {
    let r = image_rect(doc);
    f64::from(r.width.min(r.height)) / 2.0
}

/// Rounded image corners (sidebar spin, the handle at the top-left inner corner; mockup 09).
pub fn set_corner_radius(doc: &Document, radius: f64) -> Option<Edit> {
    let image = doc.image();
    let radius = radius.round().clamp(0.0, max_corner_radius(doc));
    if !radius.is_finite() || radius == image.corner_radius {
        return None;
    }
    let command = Command::SetImage(BaseImage {
        bitmap: image.bitmap.clone(),
        corner_radius: radius,
    });
    Some(edit(doc, Action::CornerRadius, command))
}

/// Burns `ids` into the image as one undo step: the image is replaced by
/// [`render::burn_in`]'s bitmap (rounded corners included, so its corner radius becomes 0) and
/// the objects are removed. If that bitmap starts left of or above the old image (an object on
/// an extended canvas), everything is re-based so the image starts at the origin again.
fn burn(doc: &Document, ids: Vec<ObjectId>, action: Action) -> Option<Edit> {
    if ids.is_empty() || has_unknown_objects(doc) {
        return None;
    }
    let subject = Subject::of(doc.scene(), &ids);
    let (bitmap, rect): (Bitmap, IRect) = render::burn_in(doc, &ids).ok()?;
    let mut steps = vec![
        Command::SetImage(BaseImage {
            bitmap,
            corner_radius: 0.0,
        }),
        Command::Remove { ids: ids.clone() },
    ];
    if rect.x != 0 || rect.y != 0 {
        let canvas = doc.canvas();
        steps.push(Command::SetCanvas(Canvas {
            rect: canvas.rect.translate(-rect.x, -rect.y),
            background: canvas.background,
        }));
        let t = Transform::translation(-f64::from(rect.x), -f64::from(rect.y));
        let objects: Vec<_> = transformed_objects(doc, &t)
            .into_iter()
            .filter(|(id, _)| !ids.contains(id))
            .collect();
        if !objects.is_empty() {
            steps.push(Command::Replace { objects });
        }
    }
    Some(Edit {
        action,
        subject,
        command: Command::Batch(steps),
        select: Some(Vec::new()),
    })
}

/// Flatten Selection (Ctrl+T).
pub fn flatten(doc: &Document, selection: &Selection) -> Option<Edit> {
    burn(doc, selection.ids().to_vec(), Action::Flatten)
}

/// The objects Flatten All burns in: every object this version can draw. (Flatten refuses while
/// the document has objects of unknown types, see [`has_unknown_objects`].)
pub fn flattenable(doc: &Document) -> Vec<ObjectId> {
    doc.scene()
        .nodes()
        .iter()
        .filter(|n| is_selectable(n.object.as_ref()))
        .map(|n| n.id)
        .collect()
}

/// Flatten All (Ctrl+Shift+T; mockup 18).
pub fn flatten_all(doc: &Document) -> Option<Edit> {
    let ids = flattenable(doc);
    let mut edit = burn(doc, ids.clone(), Action::FlattenAll)?;
    edit.subject = Subject::Many(ids.len());
    Some(edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::color::palette;
    use crate::document::command::History;
    use crate::document::geometry::{Point, Rect};
    use crate::render::flatten as export;
    use crate::tools::group::Group;
    use crate::tools::image::Image;

    /// A 4-colour test card, 64 × 40: coral | lime over azure | ink.
    fn card() -> Bitmap {
        Bitmap::draw(64, 40, |cr| {
            for (i, c) in [palette::CORAL, palette::LIME, palette::AZURE, palette::INK]
                .iter()
                .enumerate()
            {
                c.set_source(cr);
                cr.rectangle((i % 2) as f64 * 32.0, (i / 2) as f64 * 20.0, 32.0, 20.0);
                cr.fill()?;
            }
            Ok(())
        })
        .unwrap()
    }

    /// The card with one image object (no border/shadow) at `rect`.
    fn doc_with_object(rect: Rect) -> (Document, History, ObjectId) {
        let mut doc = Document::new(card());
        let mut history = History::new();
        // As large as its box: drawn without resampling, so pixels compare exactly.
        let pip = Bitmap::filled(rect.width as i32, rect.height as i32, palette::AMBER).unwrap();
        let cmd = Image::insert_command(&doc, pip, rect.centre());
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let id = doc.scene().ids()[0];
        let mut object = doc.scene().get(id).unwrap().object.clone();
        let img = object.as_any_mut().downcast_mut::<Image>().unwrap();
        img.border_width = 0.0;
        img.shadow.enabled = false;
        img.x = rect.x;
        img.y = rect.y;
        img.width = rect.width;
        img.height = rect.height;
        history
            .execute(
                &mut doc,
                "Place",
                Command::Replace {
                    objects: vec![(id, object)],
                },
            )
            .unwrap();
        (doc, history, id)
    }

    fn run(doc: &mut Document, history: &mut History, edit: Edit) {
        history.execute(doc, "op", edit.command).unwrap();
    }

    /// Runs `op` on the document as one undo step (it must change something).
    fn apply(
        doc: &mut Document,
        history: &mut History,
        op: impl FnOnce(&Document) -> Option<Edit>,
    ) {
        let edit = op(doc).unwrap();
        run(doc, history, edit);
    }

    fn arrange(doc: &Document, id: ObjectId) -> crate::document::geometry::Arrange {
        doc.scene().get(id).unwrap().object.arrange()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn crop_keeps_annotations_on_their_image_position() {
        // Spec "Crop": image and canvas are cropped and annotations keep their image positions.
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 25.0, 10.0, 6.0));
        let before = export(&doc, 1.0).unwrap();
        let rect = IRect::new(30, 10, 30, 25);
        assert!(can_crop(&doc, rect));
        let edit = crop(&doc, rect).unwrap();
        assert_eq!(edit.action, Action::Crop);
        run(&mut doc, &mut history, edit);
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 30, 25));
        assert_eq!(image_rect(&doc), IRect::new(0, 0, 30, 25));
        assert_eq!(arrange(&doc, id).rect, Rect::new(10.0, 15.0, 10.0, 6.0));
        // Every exported pixel equals the same pixel before the crop.
        let after = export(&doc, 1.0).unwrap();
        for y in 0..25 {
            for x in 0..30 {
                assert_eq!(after.pixel(x, y), before.pixel(x + 30, y + 10), "{x},{y}");
            }
        }
        // One undo step brings everything back.
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 64, 40));
        assert_eq!(arrange(&doc, id).rect, Rect::new(40.0, 25.0, 10.0, 6.0));
        // Nothing to crop: the whole canvas, outside the canvas, or no image left.
        assert!(!can_crop(&doc, IRect::new(0, 0, 64, 40)));
        assert!(!can_crop(&doc, IRect::new(10, 10, 100, 10)));
        assert!(crop(&doc, IRect::new(0, 0, 0, 5)).is_none());
    }

    #[test]
    fn crop_into_an_extended_canvas_keeps_the_extension() {
        let (mut doc, mut history, _) = doc_with_object(Rect::new(40.0, 25.0, 10.0, 6.0));
        let wider = set_canvas_rect(&doc, IRect::new(-20, 0, 84, 40)).unwrap();
        run(&mut doc, &mut history, wider);
        // Keep 10 px of the extension and the left half of the image.
        let edit = crop(&doc, IRect::new(-10, 0, 42, 40)).unwrap();
        run(&mut doc, &mut history, edit);
        assert_eq!(image_rect(&doc), IRect::new(0, 0, 32, 40));
        assert_eq!(doc.canvas().rect, IRect::new(-10, 0, 42, 40));
    }

    #[test]
    fn rotating_right_turns_image_canvas_and_objects() {
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 25.0, 10.0, 6.0));
        let before = export(&doc, 1.0).unwrap();
        apply(&mut doc, &mut history, |d| rotate(d, Turn::Right));
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 40, 64));
        let a = arrange(&doc, id);
        // Centre (45, 28) → (40 − 28, 45) = (12, 45); turned a quarter.
        assert!(close(a.rect.centre().x, 12.0) && close(a.rect.centre().y, 45.0));
        assert!(close(a.rotation, 90.0));
        // The export is the old export turned clockwise, pixel for pixel.
        let after = export(&doc, 1.0).unwrap();
        for y in 0..64 {
            for x in 0..40 {
                assert_eq!(after.pixel(x, y), before.pixel(y, 39 - x), "{x},{y}");
            }
        }
        // Left undoes Right exactly (pixels), and Ctrl+Z restores the state itself.
        apply(&mut doc, &mut history, |d| rotate(d, Turn::Left));
        let back = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(back.pixel(x, y), before.pixel(x, y), "{x},{y}");
            }
        }
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
        assert_eq!(arrange(&doc, id).rect, Rect::new(40.0, 25.0, 10.0, 6.0));
    }

    #[test]
    fn flips_mirror_the_image_and_move_objects() {
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 25.0, 10.0, 6.0));
        let before = export(&doc, 1.0).unwrap();
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Horizontal));
        assert_eq!(arrange(&doc, id).rect, Rect::new(14.0, 25.0, 10.0, 6.0));
        let after = export(&doc, 1.0).unwrap();
        // The base image is mirrored (the object is symmetric and lands mirrored too).
        assert_eq!(after.pixel(0, 0), before.pixel(63, 0));
        assert_eq!(after.pixel(63, 39), before.pixel(0, 39));
        assert_eq!(after.pixel(20, 27), before.pixel(43, 27));
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Vertical));
        assert_eq!(arrange(&doc, id).rect, Rect::new(14.0, 9.0, 10.0, 6.0));
        assert_eq!(export(&doc, 1.0).unwrap().pixel(0, 0), before.pixel(63, 39));
    }

    /// The card with one image object (no border/shadow) at (8, 6), 20 × 10, showing an
    /// asymmetric picture: amber left half, violet right half, an ink stripe along the top.
    fn doc_with_picture() -> (Document, History, ObjectId) {
        let mut doc = Document::new(card());
        let mut history = History::new();
        let picture = Bitmap::draw(20, 10, |cr| {
            for (c, x, y, w, h) in [
                (palette::AMBER, 0.0, 0.0, 10.0, 10.0),
                (palette::VIOLET, 10.0, 0.0, 10.0, 10.0),
                (palette::INK, 0.0, 0.0, 20.0, 2.0),
            ] {
                c.set_source(cr);
                cr.rectangle(x, y, w, h);
                cr.fill()?;
            }
            Ok(())
        })
        .unwrap();
        let cmd = Image::insert_command(&doc, picture, Point::new(18.0, 11.0));
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let id = doc.scene().ids()[0];
        let mut object = doc.scene().get(id).unwrap().object.clone();
        let img = object.as_any_mut().downcast_mut::<Image>().unwrap();
        img.border_width = 0.0;
        img.shadow.enabled = false;
        // As large as the picture (the insert scaled it to the canvas): exact pixels.
        (img.x, img.y, img.width, img.height) = (8.0, 6.0, 20.0, 10.0);
        let place = Command::Replace {
            objects: vec![(id, object)],
        };
        history.execute(&mut doc, "Place", place).unwrap();
        (doc, history, id)
    }

    fn image_of(doc: &Document, id: ObjectId) -> &Image {
        doc.scene()
            .get(id)
            .unwrap()
            .object
            .as_any()
            .downcast_ref::<Image>()
            .unwrap()
    }

    #[test]
    fn flips_mirror_image_objects_with_their_content() {
        // Maintainer decision (final review): flipping mirrors an image object's picture too,
        // like the base image — so the whole export is the old export mirrored, pixel for
        // pixel. (Before, the object moved to its mirrored place but its picture did not turn
        // over, so the amber/violet halves stayed in their order.)
        let (mut doc, mut history, id) = doc_with_picture();
        let before = export(&doc, 1.0).unwrap();
        let original = doc.scene().get(id).unwrap().object.to_json().unwrap();
        assert_eq!(before.pixel(9, 10), Some(palette::AMBER));
        assert_eq!(before.pixel(26, 10), Some(palette::VIOLET));
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Horizontal));
        assert!(image_of(&doc, id).flip_horizontal && !image_of(&doc, id).flip_vertical);
        let after = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(after.pixel(63 - x, y), before.pixel(x, y), "{x},{y}");
            }
        }
        // The object now sits at 36..56 with violet on the left, amber on the right.
        assert_eq!(after.pixel(37, 10), Some(palette::VIOLET));
        assert_eq!(after.pixel(54, 10), Some(palette::AMBER));
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Vertical));
        assert!(image_of(&doc, id).flip_vertical);
        let upside_down = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(upside_down.pixel(x, 39 - y), after.pixel(x, y), "{x},{y}");
            }
        }
        // The ink stripe is at the bottom of the object now.
        assert_eq!(upside_down.pixel(40, 32), Some(palette::INK));
        assert_eq!(upside_down.pixel(40, 25), Some(palette::VIOLET));
        // Flipping both back gives the same object and the same pixels.
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Vertical));
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Horizontal));
        assert_eq!(
            doc.scene().get(id).unwrap().object.to_json().unwrap(),
            original
        );
        let back = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(back.pixel(x, y), before.pixel(x, y), "{x},{y}");
            }
        }
        // Undo restores the unflipped object.
        for _ in 0..4 {
            history.undo(&mut doc).unwrap();
        }
        assert!(!image_of(&doc, id).flip_horizontal && !image_of(&doc, id).flip_vertical);
    }

    #[test]
    fn rotating_and_resizing_keep_an_image_object_unmirrored() {
        let (mut doc, mut history, id) = doc_with_picture();
        apply(&mut doc, &mut history, |d| rotate(d, Turn::Right));
        apply(&mut doc, &mut history, |d| resize(d, 80, 128, true));
        let img = image_of(&doc, id);
        assert!(!img.flip_horizontal && !img.flip_vertical);
    }

    #[test]
    fn grouped_image_objects_mirror_with_the_document() {
        let (mut doc, mut history, id) = doc_with_picture();
        let (_, other) = {
            let pip = Bitmap::filled(4, 4, palette::LIME).unwrap();
            let cmd = Image::insert_command(&doc, pip, Point::new(50.0, 30.0));
            history.execute(&mut doc, "Insert", cmd).unwrap();
            (0, *doc.scene().ids().last().unwrap())
        };
        let (gid, cmd) = crate::tools::group::group_command(&doc, &[id, other]).unwrap();
        history.execute(&mut doc, "Group", cmd).unwrap();
        apply(&mut doc, &mut history, |d| flip(d, Mirror::Horizontal));
        let group = doc.scene().get(gid).unwrap();
        for member in group.object.children() {
            let img = member.object.as_any().downcast_ref::<Image>().unwrap();
            assert!(img.flip_horizontal, "member {:?}", member.id);
        }
    }

    #[test]
    fn files_without_flip_flags_read_unmirrored() {
        // Written before the flags existed (development autosaves): not mirrored, and written
        // back with the flags.
        let value = serde_json::json!({"id": 6, "type": "image", "source": "images/1.png",
            "x": 0, "y": 0, "width": 4, "height": 4, "rotation": 0, "opacity": 1,
            "border_color": "#FFFFFFFF", "border_width": 0, "corner_radius": 0,
            "shadow": {"enabled": false, "color": "#000006FF", "opacity": 0.4, "angle": 52,
                       "distance": 10, "blur": 8}});
        let node = crate::document::registry::node_from_json(value, false).unwrap();
        let img = node.object.as_any().downcast_ref::<Image>().unwrap();
        assert!(!img.flip_horizontal && !img.flip_vertical);
        let written = crate::document::registry::node_to_json(&node).unwrap();
        assert_eq!(written["flip_horizontal"], false);
        assert_eq!(written["flip_vertical"], false);
    }

    /// Adds an object of a type this version does not know (as read from a newer Annota).
    fn add_unknown(doc: &mut Document, history: &mut History) -> ObjectId {
        let fields = serde_json::json!({"x": 30, "y": 20, "width": 10, "height": 5});
        let serde_json::Value::Object(fields) = fields else {
            panic!("object expected");
        };
        let id = doc.scene().fresh_ids(1)[0];
        let node =
            crate::document::scene::Node::new(id, Box::new(UnknownObject::new("hologram", fields)));
        let at = doc.scene().len();
        history
            .execute(
                doc,
                "Read",
                Command::Insert {
                    nodes: vec![(at, node)],
                    resources: Vec::new(),
                },
            )
            .unwrap();
        id
    }

    #[test]
    fn unknown_objects_block_the_operations_that_move_every_object() {
        // N1 (maintainer decision): crop, rotate, flip, resize image and flatten would leave an
        // object of an unknown type behind on the wrong pixels (its geometry is opaque), which
        // breaks "left where they are". They refuse while such an object is in the document;
        // everything that moves nothing, and editing the known objects, keeps working.
        let (mut doc, mut history, known) = doc_with_object(Rect::new(40.0, 25.0, 10.0, 6.0));
        // Without unknown objects every operation is available.
        assert!(!has_unknown_objects(&doc));
        assert!(rotate(&doc, Turn::Right).is_some());
        assert!(flip(&doc, Mirror::Horizontal).is_some());
        assert!(resize(&doc, 32, 20, true).is_some());
        assert!(can_crop(&doc, IRect::new(0, 0, 30, 20)));
        assert!(flatten_all(&doc).is_some());

        let unknown = add_unknown(&mut doc, &mut history);
        assert!(has_unknown_objects(&doc));
        let before = crate::document::archive::document_json(&doc).unwrap();
        assert!(rotate(&doc, Turn::Right).is_none());
        assert!(rotate(&doc, Turn::Left).is_none());
        assert!(flip(&doc, Mirror::Horizontal).is_none());
        assert!(flip(&doc, Mirror::Vertical).is_none());
        assert!(resize(&doc, 32, 20, true).is_none());
        assert!(!can_crop(&doc, IRect::new(0, 0, 30, 20)));
        assert!(crop(&doc, IRect::new(0, 0, 30, 20)).is_none());
        assert!(flatten_all(&doc).is_none());
        let mut sel = Selection::new();
        sel.set(doc.scene(), [known]);
        assert!(flatten(&doc, &sel).is_none());
        assert_eq!(
            crate::document::archive::document_json(&doc).unwrap(),
            before
        );

        // Moving nothing stays available: canvas size, background, corner radius.
        apply(&mut doc, &mut history, |d| {
            set_canvas_rect(d, IRect::new(0, 0, 84, 40))
        });
        apply(&mut doc, &mut history, |d| {
            set_background(
                d,
                Background::Color {
                    color: palette::PAPER,
                },
            )
        });
        apply(&mut doc, &mut history, |d| set_corner_radius(d, 4.0));
        // The known object is still edited normally.
        let edit = crate::editing::ops::nudge(&doc, &sel, 5.0, 0.0).unwrap();
        run(&mut doc, &mut history, edit);
        assert_eq!(arrange(&doc, known).rect.x, 45.0);
        let edit = crate::editing::ops::duplicate(&doc, &sel).unwrap();
        run(&mut doc, &mut history, edit);
        assert!(doc.scene().get(unknown).is_some());

        // Once the unknown object is gone (here: undoing back to before it was read), the
        // operations are available again.
        while doc.scene().get(unknown).is_some() {
            history.undo(&mut doc).unwrap();
        }
        assert!(!has_unknown_objects(&doc));
        assert!(rotate(&doc, Turn::Right).is_some());
    }

    #[test]
    fn resize_scales_image_canvas_and_objects() {
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 20.0, 10.0, 6.0));
        let edit = resize(&doc, 32, 20, true).unwrap();
        assert_eq!(edit.action, Action::ResizeImage);
        run(&mut doc, &mut history, edit);
        assert_eq!(image_rect(&doc), IRect::new(0, 0, 32, 20));
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 32, 20));
        assert_eq!(arrange(&doc, id).rect, Rect::new(20.0, 10.0, 5.0, 3.0));
        // Quadrant colours survive the halving (no fading at the edges).
        let out = export(&doc, 1.0).unwrap();
        assert_eq!(out.pixel(0, 0), Some(palette::CORAL));
        assert_eq!(out.pixel(31, 19), Some(palette::INK));
        // Unchanged or impossible sizes are no edit.
        assert!(resize(&doc, 32, 20, true).is_none());
        assert!(resize(&doc, 0, 20, true).is_none());
    }

    #[test]
    fn scale_annotations_switch_decides_about_styles() {
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 20.0, 10.0, 6.0));
        let mut object = doc.scene().get(id).unwrap().object.clone();
        object
            .as_any_mut()
            .downcast_mut::<Image>()
            .unwrap()
            .border_width = 6.0;
        run(
            &mut doc,
            &mut history,
            Edit {
                action: Action::Resize,
                subject: Subject::Many(1),
                command: Command::Replace {
                    objects: vec![(id, object)],
                },
                select: None,
            },
        );
        let border = |doc: &Document| {
            doc.scene()
                .get(id)
                .unwrap()
                .object
                .as_any()
                .downcast_ref::<Image>()
                .unwrap()
                .border_width
        };
        let doubled = resize(&doc, 128, 80, true).unwrap();
        run(&mut doc, &mut history, doubled);
        assert_eq!(border(&doc), 12.0);
        history.undo(&mut doc).unwrap();
        let kept = resize(&doc, 128, 80, false).unwrap();
        run(&mut doc, &mut history, kept);
        assert_eq!(border(&doc), 6.0);
        // The position follows the image either way.
        assert_eq!(arrange(&doc, id).rect, Rect::new(80.0, 40.0, 20.0, 12.0));
    }

    #[test]
    fn resize_with_annotations_scenario() {
        // Spec "Resize with annotations": 960 × 800 → 480 × 400, Scale annotations on.
        let mut doc = Document::new(Bitmap::filled(960, 800, palette::PAPER).unwrap());
        let mut history = History::new();
        let pip = Bitmap::filled(100, 50, palette::CORAL).unwrap();
        let cmd = Image::insert_command(&doc, pip, Point::new(300.0, 200.0));
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let id = doc.scene().ids()[0];
        let before = arrange(&doc, id).rect;
        apply(&mut doc, &mut history, |d| resize(d, 480, 400, true));
        assert_eq!(image_rect(&doc), IRect::new(0, 0, 480, 400));
        let after = arrange(&doc, id).rect;
        assert_eq!(
            after,
            Rect::new(
                before.x / 2.0,
                before.y / 2.0,
                before.width / 2.0,
                before.height / 2.0
            )
        );
        let border = doc
            .scene()
            .get(id)
            .unwrap()
            .object
            .as_any()
            .downcast_ref::<Image>()
            .unwrap()
            .border_width;
        assert_eq!(border, 3.0, "6 px border halved");
    }

    #[test]
    fn grouped_members_follow_a_rotation() {
        let (mut doc, mut history, _) = doc_with_object(Rect::new(4.0, 4.0, 10.0, 6.0));
        let pip = Bitmap::filled(8, 4, palette::TEAL).unwrap();
        let cmd = Image::insert_command(&doc, pip, Point::new(50.0, 30.0));
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let mut sel = Selection::new();
        sel.select_all(doc.scene());
        let group = super::super::ops::group(&doc, &sel).unwrap();
        run(&mut doc, &mut history, group);
        let gid = doc.scene().ids()[0];
        apply(&mut doc, &mut history, |d| rotate(d, Turn::Right));
        let node = doc.scene().get(gid).unwrap();
        let g = node.object.as_any().downcast_ref::<Group>().unwrap();
        assert!(close(g.rotation, 90.0));
        // First member's centre (9, 7) → (40 − 7, 9) = (33, 9).
        let c = g.children[0].object.arrange().rect.centre();
        assert!(close(c.x, 33.0) && close(c.y, 9.0));
    }

    #[test]
    fn canvas_size_background_and_corner_radius_are_undoable() {
        let (mut doc, mut history, _) = doc_with_object(Rect::new(4.0, 4.0, 10.0, 6.0));
        // Spec "Extend for callout": 200 px more on the right, filled with the canvas colour.
        let edit = set_canvas_rect(&doc, IRect::new(0, 0, 264, 40)).unwrap();
        assert_eq!(edit.action, Action::ExtendCanvas);
        run(&mut doc, &mut history, edit);
        let paper = Background::Color {
            color: palette::PAPER,
        };
        apply(&mut doc, &mut history, |d| set_background(d, paper));
        let out = export(&doc, 1.0).unwrap();
        assert_eq!(out.width(), 264);
        assert_eq!(out.pixel(200, 20), Some(palette::PAPER));
        assert!(set_background(&doc, paper).is_none());
        let edit = set_corner_radius(&doc, 500.0).unwrap();
        run(&mut doc, &mut history, edit);
        assert_eq!(
            doc.image().corner_radius,
            20.0,
            "at most half the shorter side"
        );
        for _ in 0..3 {
            history.undo(&mut doc).unwrap();
        }
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 64, 40));
        assert_eq!(doc.canvas().background, Background::Transparent);
        assert_eq!(doc.image().corner_radius, 0.0);
    }

    #[test]
    fn flatten_all_burns_objects_into_the_image() {
        // Spec "Flatten all": no editable objects remain and the image shows them burnt in.
        let (mut doc, mut history, _) = doc_with_object(Rect::new(40.0, 20.0, 10.0, 6.0));
        let pip = Bitmap::filled(8, 4, palette::TEAL).unwrap();
        let cmd = Image::insert_command(&doc, pip, Point::new(10.0, 10.0));
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let before = export(&doc, 1.0).unwrap();
        let edit = flatten_all(&doc).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::FlattenAll, Subject::Many(2))
        );
        run(&mut doc, &mut history, edit);
        assert_eq!(doc.object_count(), 0);
        let after = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..64 {
                assert_eq!(after.pixel(x, y), before.pixel(x, y), "{x},{y}");
            }
        }
        assert!(flatten_all(&doc).is_none(), "nothing left to flatten");
        history.undo(&mut doc).unwrap();
        assert_eq!(doc.object_count(), 2);
    }

    #[test]
    fn flatten_selection_keeps_the_rest_and_grows_into_the_extension() {
        let (mut doc, mut history, id) = doc_with_object(Rect::new(40.0, 20.0, 10.0, 6.0));
        let wider = set_canvas_rect(&doc, IRect::new(-20, 0, 84, 40)).unwrap();
        run(&mut doc, &mut history, wider);
        // A second object half in the left extension.
        let pip = Bitmap::filled(8, 4, palette::TEAL).unwrap();
        let cmd = Image::insert_command(&doc, pip, Point::new(-2.0, 10.0));
        history.execute(&mut doc, "Insert", cmd).unwrap();
        let other = doc.scene().ids()[1];
        let mut sel = Selection::new();
        sel.set(doc.scene(), [other]);
        let before = export(&doc, 1.0).unwrap();
        let edit = flatten(&doc, &sel).unwrap();
        assert_eq!(edit.subject, Subject::One(Image::TYPE));
        run(&mut doc, &mut history, edit);
        assert_eq!(doc.scene().ids(), vec![id]);
        // The image grew left to keep the object's pixels and everything was re-based.
        let image = image_rect(&doc);
        assert!(image.width > 64, "{image:?}");
        let shift = image.width - 64;
        assert_eq!(doc.canvas().rect.x, -20 + shift);
        assert_eq!(arrange(&doc, id).rect.x, 40.0 + f64::from(shift));
        let after = export(&doc, 1.0).unwrap();
        for y in 0..40 {
            for x in 0..84 {
                assert_eq!(after.pixel(x, y), before.pixel(x, y), "{x},{y}");
            }
        }
        assert_eq!(doc.image().corner_radius, 0.0);
    }
}
