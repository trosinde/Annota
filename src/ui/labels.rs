//! Translated texts for canvas editing: undo labels ("Undo Move Group", mockup 06), snap chips
//! ("Canvas centre", mockup 02) and measurement chips ("X 580 · Y 467", "Δ X +560 · Y +330",
//! "−15° · Shift: 15° steps"; mockups 02, 06, 03). The editing logic (`annota::editing`) only
//! returns values; the words are chosen here.

use gettextrs::{gettext, ngettext, pgettext};

use annota::editing::canvas_edit::Feedback as CanvasFeedback;
use annota::editing::drag::Feedback;
use annota::editing::ops::{Action, Edit, Subject};
use annota::editing::snap::Target;
use annota::tools::arrow::Arrow;
use annota::tools::callout::Callout;
use annota::tools::group::Group;
use annota::tools::highlighter::Highlighter;
use annota::tools::image::Image;
use annota::tools::pen::Pen;
use annota::tools::redaction::Redaction;
use annota::tools::shape::Shape;
use annota::tools::step::Step;
use annota::tools::text::Text;

/// Name of an object type as the user sees it ("Image", "Group", "Arrow").
pub fn object_name(type_name: &str) -> String {
    match type_name {
        Image::TYPE => pgettext("object type", "Image"),
        Group::TYPE => pgettext("object type", "Group"),
        Arrow::TYPE => pgettext("object type", "Arrow"),
        Arrow::LINE_TYPE => pgettext("object type", "Line"),
        Pen::TYPE => pgettext("object type", "Pen Stroke"),
        Shape::TYPE => pgettext("object type", "Shape"),
        Text::TYPE => pgettext("object type", "Text"),
        Callout::TYPE => pgettext("object type", "Callout"),
        Step::TYPE => pgettext("object type", "Step"),
        Highlighter::TYPE => pgettext("object type", "Highlight"),
        Redaction::TYPE => pgettext("object type", "Redaction"),
        _ => pgettext("object type", "Object"),
    }
}

fn subject(subject: Subject) -> String {
    match subject {
        Subject::One(type_name) => object_name(type_name),
        Subject::Many(n) => {
            let count = u32::try_from(n).unwrap_or(u32::MAX);
            ngettext("{n} Object", "{n} Objects", count).replace("{n}", &n.to_string())
        }
    }
}

/// The undo step's name, shown as "Undo <label>".
pub fn undo_label(edit: &Edit) -> String {
    let with_object = |template: String| template.replace("{object}", &subject(edit.subject));
    match edit.action {
        Action::Move => with_object(gettext("Move {object}")),
        Action::Resize => with_object(gettext("Resize {object}")),
        Action::Rotate => with_object(gettext("Rotate {object}")),
        Action::Reshape => with_object(gettext("Edit {object}")),
        Action::Delete => with_object(gettext("Delete {object}")),
        Action::Duplicate => with_object(gettext("Duplicate {object}")),
        Action::BringToFront => gettext("Bring to Front"),
        Action::BringForward => gettext("Bring Forward"),
        Action::SendBackward => gettext("Send Backward"),
        Action::SendToBack => gettext("Send to Back"),
        Action::Group => gettext("Group"),
        Action::Ungroup => gettext("Ungroup"),
        Action::Insert => with_object(gettext("Insert {object}")),
        Action::Add => with_object(gettext("Add {object}")),
        Action::Style => with_object(gettext("Change {object}")),
        Action::Crop => gettext("Crop"),
        Action::ResizeImage => gettext("Resize Image"),
        Action::RotateLeft => gettext("Rotate Left"),
        Action::RotateRight => gettext("Rotate Right"),
        Action::FlipHorizontally => gettext("Flip Horizontally"),
        Action::FlipVertically => gettext("Flip Vertically"),
        Action::ExtendCanvas => gettext("Extend Canvas"),
        Action::CanvasBackground => gettext("Canvas Background"),
        Action::CornerRadius => gettext("Corner Radius"),
        Action::Flatten => with_object(gettext("Flatten {object}")),
        Action::FlattenAll => gettext("Flatten All"),
    }
}

/// What a snap guide lines up with.
pub fn target_name(target: Target) -> String {
    match target {
        Target::CanvasCentre => gettext("Canvas centre"),
        Target::CanvasEdge => gettext("Canvas edge"),
        Target::ObjectCentre => gettext("Object centre"),
        Target::ObjectEdge => gettext("Object edge"),
    }
}

/// Text of the snap chip: the targets, most important first.
pub fn snap_chip(targets: &[Target]) -> Option<String> {
    if targets.is_empty() {
        return None;
    }
    Some(
        targets
            .iter()
            .map(|t| target_name(*t))
            .collect::<Vec<_>>()
            .join(" · "),
    )
}

/// A whole number with a real minus sign ("−15").
fn number(v: f64) -> String {
    let n = v.round();
    if n < 0.0 {
        format!("\u{2212}{}", -n)
    } else {
        format!("{}", n.abs())
    }
}

/// A distance with its sign ("+560", "−20", "0").
fn signed(v: f64) -> String {
    let n = v.round();
    if n > 0.0 {
        format!("+{n}")
    } else {
        number(n)
    }
}

/// Text of the measurement chip for a gesture, if it shows one.
pub fn measure_chip(feedback: Feedback) -> Option<String> {
    match feedback {
        Feedback::Move {
            position, single, ..
        } if single => Some(
            gettext("X {x} · Y {y}")
                .replace("{x}", &number(position.x))
                .replace("{y}", &number(position.y)),
        ),
        Feedback::Move { delta, .. } => Some(
            gettext("Δ X {dx} · Y {dy}")
                .replace("{dx}", &signed(delta.x))
                .replace("{dy}", &signed(delta.y)),
        ),
        Feedback::Resize { width, height } => Some(
            gettext("{width} × {height}")
                .replace("{width}", &number(width))
                .replace("{height}", &number(height)),
        ),
        Feedback::Rotate { degrees, stepped } => {
            let template = if stepped {
                gettext("{angle}° · Shift: 15° steps")
            } else {
                gettext("{angle}°")
            };
            Some(template.replace("{angle}", &number(degrees)))
        }
        Feedback::Band(_) | Feedback::None => None,
    }
}

/// Chip of a canvas-edge or corner-radius drag: "+400 px" (mockup 10), "Corner radius 20 px"
/// (mockup 09).
pub fn canvas_chip(feedback: CanvasFeedback) -> Option<String> {
    match feedback {
        CanvasFeedback::Extend { extra, .. } => {
            Some(gettext("{n} px").replace("{n}", &signed(f64::from(extra))))
        }
        CanvasFeedback::Radius(radius) => {
            Some(gettext("Corner radius {n} px").replace("{n}", &number(radius)))
        }
        CanvasFeedback::None => None,
    }
}

/// The size chip under the crop frame (mockup 08): "960 × 800 px".
pub fn crop_chip(width: i32, height: i32) -> String {
    gettext("{width} × {height} px")
        .replace("{width}", &width.to_string())
        .replace("{height}", &height.to_string())
}

/// A tool's name as its button's tooltip shows it (mockup 01: "Arrow").
pub fn tool_name(tool: annota::editing::tool::Tool) -> String {
    use annota::editing::tool::Tool;
    match tool {
        Tool::Select => gettext("Select"),
        Tool::Arrow => gettext("Arrow"),
        Tool::Line => gettext("Line"),
        Tool::Pen => gettext("Pen"),
        Tool::Shape => gettext("Shape"),
        Tool::Text => gettext("Text"),
        Tool::Callout => gettext("Callout"),
        Tool::Step => gettext("Step"),
        Tool::Highlighter => gettext("Highlighter"),
        Tool::Redact => gettext("Redact"),
    }
}

/// While text is edited in place (mockup 08).
pub fn text_edit_chip() -> String {
    gettext("Esc or click outside to finish")
}

/// While a polygon is drawn (mockup 07): "4 points · Enter or double-click to finish".
pub fn polygon_chip(points: usize) -> String {
    let count = u32::try_from(points).unwrap_or(u32::MAX);
    ngettext(
        "{n} point · Enter or double-click to finish",
        "{n} points · Enter or double-click to finish",
        count,
    )
    .replace("{n}", &points.to_string())
}

/// While an arrow or line is drawn with Shift (redline): "0° · Shift: 0/45/90°".
pub fn angle_chip(degrees: f64) -> String {
    gettext("{angle}° · Shift: 0/45/90°").replace("{angle}", &number(degrees))
}

/// Under the step tool's placement preview (mockup 11): "Click to place 5".
pub fn step_chip(label: &str) -> String {
    gettext("Click to place {label}").replace("{label}", label)
}

/// The toast when deleting step markers renumbers others (mockup 13): "Step 2 deleted · 2
/// steps renumbered"; without the first part when several objects were deleted.
pub fn steps_renumbered(deleted: Option<&str>, renumbered: usize) -> String {
    let count = u32::try_from(renumbered).unwrap_or(u32::MAX);
    let renumbered = ngettext("{n} step renumbered", "{n} steps renumbered", count)
        .replace("{n}", &renumbered.to_string());
    match deleted {
        Some(label) => gettext("Step {label} deleted · {renumbered}")
            .replace("{label}", label)
            .replace("{renumbered}", &renumbered),
        None => renumbered,
    }
}

/// The toast after flattening (mockup 18): "7 objects flattened into the image".
pub fn flattened(n: usize) -> String {
    let count = u32::try_from(n).unwrap_or(u32::MAX);
    ngettext(
        "{n} object flattened into the image",
        "{n} objects flattened into the image",
        count,
    )
    .replace("{n}", &n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::command::Command;
    use annota::document::geometry::Point;

    fn edit(action: Action, subject: Subject) -> Edit {
        Edit {
            action,
            subject,
            command: Command::Batch(Vec::new()),
            select: None,
        }
    }

    #[test]
    fn undo_labels_name_the_action() {
        // Mockup 06: "Undo Move Group".
        assert_eq!(
            undo_label(&edit(Action::Move, Subject::One("group"))),
            "Move Group"
        );
        assert_eq!(
            undo_label(&edit(Action::Rotate, Subject::One("image"))),
            "Rotate Image"
        );
        assert_eq!(
            undo_label(&edit(Action::Resize, Subject::Many(3))),
            "Resize 3 Objects"
        );
        assert_eq!(
            undo_label(&edit(Action::Delete, Subject::One("arrow"))),
            "Delete Arrow"
        );
        assert_eq!(
            undo_label(&edit(Action::Add, Subject::One("step"))),
            "Add Step"
        );
        assert_eq!(
            undo_label(&edit(Action::Reshape, Subject::One("text"))),
            "Edit Text"
        );
        assert_eq!(
            undo_label(&edit(Action::Delete, Subject::One("newer-thing"))),
            "Delete Object"
        );
        assert_eq!(
            undo_label(&edit(Action::BringToFront, Subject::Many(2))),
            "Bring to Front"
        );
        assert_eq!(
            undo_label(&edit(Action::RotateRight, Subject::Many(0))),
            "Rotate Right"
        );
        assert_eq!(
            undo_label(&edit(Action::Flatten, Subject::Many(3))),
            "Flatten 3 Objects"
        );
        assert_eq!(
            undo_label(&edit(Action::FlattenAll, Subject::Many(7))),
            "Flatten All"
        );
    }

    #[test]
    fn image_operation_texts_match_the_mockups() {
        use annota::editing::canvas_edit::Side;
        // Mockup 10.
        assert_eq!(
            canvas_chip(CanvasFeedback::Extend {
                side: Side::Right,
                extra: 400
            })
            .as_deref(),
            Some("+400 px")
        );
        // Mockup 09.
        assert_eq!(
            canvas_chip(CanvasFeedback::Radius(20.0)).as_deref(),
            Some("Corner radius 20 px")
        );
        assert_eq!(canvas_chip(CanvasFeedback::None), None);
        // Mockup 08.
        assert_eq!(crop_chip(960, 800), "960 × 800 px");
        // Mockup 18.
        assert_eq!(flattened(7), "7 objects flattened into the image");
        assert_eq!(flattened(1), "1 object flattened into the image");
    }

    #[test]
    fn drawing_texts_match_the_mockups() {
        assert_eq!(text_edit_chip(), "Esc or click outside to finish");
        assert_eq!(
            polygon_chip(4),
            "4 points · Enter or double-click to finish"
        );
        assert_eq!(polygon_chip(1), "1 point · Enter or double-click to finish");
        assert_eq!(angle_chip(0.0), "0° · Shift: 0/45/90°");
        assert_eq!(angle_chip(45.0), "45° · Shift: 0/45/90°");
        assert_eq!(step_chip("5"), "Click to place 5");
        assert_eq!(
            steps_renumbered(Some("2"), 2),
            "Step 2 deleted · 2 steps renumbered"
        );
        assert_eq!(steps_renumbered(None, 1), "1 step renumbered");
        assert_eq!(object_name("redaction"), "Redaction");
        assert_eq!(object_name("pen"), "Pen Stroke");
    }

    #[test]
    fn chips_match_the_mockups() {
        // Mockup 02.
        assert_eq!(
            snap_chip(&[Target::CanvasCentre]).as_deref(),
            Some("Canvas centre")
        );
        assert_eq!(snap_chip(&[]), None);
        assert_eq!(
            measure_chip(Feedback::Move {
                position: Point::new(580.0, 467.0),
                delta: Point::new(3.0, 4.0),
                single: true
            })
            .as_deref(),
            Some("X 580 · Y 467")
        );
        // Mockup 06.
        assert_eq!(
            measure_chip(Feedback::Move {
                position: Point::new(0.0, 0.0),
                delta: Point::new(560.0, 330.0),
                single: false
            })
            .as_deref(),
            Some("Δ X +560 · Y +330")
        );
        assert_eq!(
            measure_chip(Feedback::Move {
                position: Point::new(0.0, 0.0),
                delta: Point::new(-20.0, 0.0),
                single: false
            })
            .as_deref(),
            Some("Δ X \u{2212}20 · Y 0")
        );
        // Mockup 03.
        assert_eq!(
            measure_chip(Feedback::Rotate {
                degrees: -15.0,
                stepped: true
            })
            .as_deref(),
            Some("\u{2212}15° · Shift: 15° steps")
        );
        assert_eq!(
            measure_chip(Feedback::Rotate {
                degrees: 22.4,
                stepped: false
            })
            .as_deref(),
            Some("22°")
        );
        assert_eq!(
            measure_chip(Feedback::Resize {
                width: 440.0,
                height: 66.4
            })
            .as_deref(),
            Some("440 × 66")
        );
        assert_eq!(measure_chip(Feedback::None), None);
    }
}
