//! Translated texts for canvas editing: undo labels ("Undo Move Group", mockup 06), snap chips
//! ("Canvas centre", mockup 02) and measurement chips ("X 580 · Y 467", "Δ X +560 · Y +330",
//! "−15° · Shift: 15° steps"; mockups 02, 06, 03). The editing logic (`annota::editing`) only
//! returns values; the words are chosen here.

use gettextrs::{gettext, ngettext, pgettext};

use annota::editing::drag::Feedback;
use annota::editing::ops::{Action, Edit, Subject};
use annota::editing::snap::Target;
use annota::tools::group::Group;
use annota::tools::image::Image;

/// Name of an object type as the user sees it ("Image", "Group").
pub fn object_name(type_name: &str) -> String {
    match type_name {
        Image::TYPE => pgettext("object type", "Image"),
        Group::TYPE => pgettext("object type", "Group"),
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
            "Delete Object"
        );
        assert_eq!(
            undo_label(&edit(Action::BringToFront, Subject::Many(2))),
            "Bring to Front"
        );
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
