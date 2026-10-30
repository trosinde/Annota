//! Translated texts of the editor window, kept free of widgets so they can be tested: the
//! header bar subtitle and zoom label (mockups 01, 07), the sidebar headings (mockups 04, 06,
//! 16), the rows of "Restore Unsaved Captures?" (mockup 14) and the "Newer Annota Version
//! Needed" alert (mockup 15).

use gettextrs::{gettext, ngettext, pgettext};
use gtk::glib;

use annota::document::archive::{FormatVersion, FORMAT};
use annota::render::view;
use annota::tools::group::Group;
use annota::tools::image::Image;

use crate::ui::labels::object_name;

/// Whether a document differs from its file (header subtitle, tray dot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Same as the file it was opened from.
    Clean,
    /// Has changes that are not in a file.
    Edited,
    /// A new capture nobody has saved or changed yet (add-screen-capture mockup 12).
    NotSaved,
}

impl Status {
    /// Whether the tray shows the "unsaved" dot.
    pub fn unsaved(self) -> bool {
        self != Status::Clean
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// "W × H" of the canvas.
pub fn size(width: i32, height: i32) -> String {
    gettext("{width} × {height}")
        .replace("{width}", &width.to_string())
        .replace("{height}", &height.to_string())
}

/// The header bar subtitle (redline "Window and shell"): `W × H · Edited`, `W × H · 200 %` while
/// not fitted, `Crop · W × H` in crop mode (task 4.3 sets `crop`).
pub fn subtitle(
    canvas: (i32, i32),
    status: Status,
    zoom: Option<f64>,
    crop: Option<(i32, i32)>,
) -> String {
    if let Some((w, h)) = crop {
        return gettext("Crop · {size}").replace("{size}", &size(w, h));
    }
    let size = size(canvas.0, canvas.1);
    let extra = match (zoom, status) {
        (Some(zoom), _) => Some(percent(zoom)),
        (None, Status::Edited) => Some(gettext("Edited")),
        (None, Status::NotSaved) => Some(gettext("Not saved")),
        (None, Status::Clean) => None,
    };
    match extra {
        Some(extra) => format!("{size} · {extra}"),
        None => size,
    }
}

/// "200 %".
pub fn percent(zoom: f64) -> String {
    gettext("{percent} %").replace("{percent}", &view::percent(zoom).to_string())
}

/// The zoom level button: "Fit · 50 %" while fitted, else "200 %" (mockups 01, 07).
pub fn zoom_label(zoom: f64, fitted: bool) -> String {
    if fitted {
        gettext("Fit · {percent}").replace("{percent}", &percent(zoom))
    } else {
        percent(zoom)
    }
}

/// Tooltip text of the Undo or Redo button: "Undo Move Group" (mockup 06), plain "Undo" when
/// there is nothing to undo.
pub fn history_tooltip(redo: bool, step: Option<&str>) -> String {
    match (redo, step) {
        (false, Some(step)) => gettext("Undo {action}").replace("{action}", step),
        (false, None) => gettext("Undo"),
        (true, Some(step)) => gettext("Redo {action}").replace("{action}", step),
        (true, None) => gettext("Redo"),
    }
}

/// Name for a document without a title or file.
pub fn untitled() -> String {
    gettext("Untitled Capture")
}

/// "Image", "Group" — or "3 Images", "3 Objects" for several.
fn objects_of_type(type_name: Option<&str>, n: usize) -> String {
    let text = match type_name {
        Some(Image::TYPE) => ngettext("{n} Image", "{n} Images", count(n)),
        Some(Group::TYPE) => ngettext("{n} Group", "{n} Groups", count(n)),
        _ => ngettext("{n} Object", "{n} Objects", count(n)),
    };
    text.replace("{n}", &n.to_string())
}

/// "{n} objects" (lower case, captions).
pub fn object_count(n: usize) -> String {
    ngettext("{n} object", "{n} objects", count(n)).replace("{n}", &n.to_string())
}

/// Sidebar heading and the dim caption beside it for a selection of `n` objects whose common
/// type is `common` (redline "Sidebar contents"; `members` is the member count of a single
/// group): "Image · 1 selected", "Group · 2 objects", "3 Images · Shared properties".
pub fn selection_heading(common: Option<&str>, n: usize, members: usize) -> (String, String) {
    match (common, n) {
        (Some(Group::TYPE), 1) => (object_name(Group::TYPE), object_count(members)),
        (Some(type_name), 1) => (object_name(type_name), gettext("1 selected")),
        _ => (objects_of_type(common, n), gettext("Shared properties")),
    }
}

/// "Select All Images" (redline: Select All <Type>); `None` for types without one.
pub fn select_all_label(type_name: &str) -> Option<String> {
    match type_name {
        Image::TYPE => Some(gettext("Select All Images")),
        Group::TYPE => Some(gettext("Select All Groups")),
        _ => None,
    }
}

/// Row title of the per-type summary of a large document (mockup 16): "Images", "Groups".
pub fn type_plural(type_name: &str) -> String {
    match type_name {
        Image::TYPE => pgettext("object types", "Images"),
        Group::TYPE => pgettext("object types", "Groups"),
        _ => pgettext("object types", "Other"),
    }
}

/// "N editable objects" (sidebar, Objects group).
pub fn editable_objects(n: usize) -> String {
    ngettext("{n} editable object", "{n} editable objects", count(n)).replace("{n}", &n.to_string())
}

/// Second line of a "Restore Unsaved Captures?" row: "Autosaved today at 10:42 · 7 objects".
/// `saved` and `now` are local times.
pub fn autosaved(saved: &glib::DateTime, now: &glib::DateTime, objects: usize) -> String {
    let time = saved
        .format("%H:%M")
        .map(|s| s.to_string())
        .unwrap_or_default();
    let day = |d: &glib::DateTime| (d.year(), d.day_of_year());
    let yesterday = now.add_days(-1).ok();
    let template = if day(saved) == day(now) {
        gettext("Autosaved today at {time} · {objects}")
    } else if yesterday.is_some_and(|y| day(&y) == day(saved)) {
        gettext("Autosaved yesterday at {time} · {objects}")
    } else {
        gettext("Autosaved on {date} at {time} · {objects}")
    };
    let date = saved
        .format("%x")
        .map(|s| s.to_string())
        .unwrap_or_default();
    template
        .replace("{date}", &date)
        .replace("{time}", &time)
        .replace("{objects}", &object_count(objects))
}

/// "0.1" of "0.1.0".
fn short_version(version: &str) -> String {
    version.split('.').take(2).collect::<Vec<_>>().join(".")
}

/// Body of "Newer Annota Version Needed" (mockup 15): what the file needs, what this is, and
/// that nothing was changed.
pub fn newer_version_body(
    file_name: &str,
    found: FormatVersion,
    requires: Option<&str>,
    version: &str,
) -> String {
    let first = match requires {
        Some(requires) => gettext(
            "“{file}” uses document format {format}. It was saved with Annota {requires} or newer.",
        )
        .replace("{requires}", requires),
        None => gettext("“{file}” uses document format {format}."),
    }
    .replace("{file}", file_name)
    .replace("{format}", &found.to_string());
    let second = match requires {
        Some(requires) => gettext(
            "This is Annota {version}, which opens format {major}.x. Install Annota {requires} or newer to open this file. The file was not changed.",
        )
        .replace("{requires}", requires),
        None => gettext(
            "This is Annota {version}, which opens format {major}.x. Install a newer Annota to open this file. The file was not changed.",
        ),
    }
    .replace("{version}", &short_version(version))
    .replace("{major}", &FORMAT.major.to_string());
    format!("{first}\n\n{second}")
}

/// Toast when crop, rotate, flip, resize image or flatten is tried on a document with objects of
/// a type this version does not know: they could not follow, so these stay unavailable.
pub fn newer_objects_toast() -> String {
    gettext("This document has objects from a newer Annota. Update Annota to crop, rotate, flip, resize or flatten it.")
}

/// Tooltip of a control that is unavailable for that reason; `name` is what it does
/// ("Rotate Left"), or `None` for a control with a visible label.
pub fn newer_objects_tooltip(name: Option<&str>) -> String {
    let why = gettext("Needs a newer Annota: this document has objects from a newer version");
    match name {
        Some(name) => gettext("{action} · {reason}")
            .replace("{action}", name)
            .replace("{reason}", &why),
        None => why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_objects_texts() {
        assert!(newer_objects_toast().contains("Update Annota"));
        assert_eq!(
            newer_objects_tooltip(Some("Crop (C)")),
            "Crop (C) · Needs a newer Annota: this document has objects from a newer version"
        );
        assert!(newer_objects_tooltip(None).starts_with("Needs a newer Annota"));
    }

    #[test]
    fn subtitle_follows_the_redline() {
        assert_eq!(
            subtitle((1600, 1000), Status::Edited, None, None),
            "1600 × 1000 · Edited"
        );
        assert_eq!(
            subtitle((1600, 1000), Status::Clean, None, None),
            "1600 × 1000"
        );
        assert_eq!(
            subtitle((400, 300), Status::NotSaved, None, None),
            "400 × 300 · Not saved"
        );
        // Not fitted: the zoom replaces the status (mockup 07).
        assert_eq!(
            subtitle((1600, 1000), Status::Edited, Some(2.0), None),
            "1600 × 1000 · 200 %"
        );
        assert_eq!(
            subtitle((1600, 1000), Status::Edited, Some(2.0), Some((800, 600))),
            "Crop · 800 × 600"
        );
    }

    #[test]
    fn history_tooltips_name_the_step() {
        assert_eq!(
            history_tooltip(false, Some("Move Group")),
            "Undo Move Group"
        );
        assert_eq!(
            history_tooltip(true, Some("Insert Image")),
            "Redo Insert Image"
        );
        assert_eq!(history_tooltip(false, None), "Undo");
    }

    #[test]
    fn zoom_label_shows_fit() {
        assert_eq!(zoom_label(0.5, true), "Fit · 50 %");
        assert_eq!(zoom_label(2.0, false), "200 %");
        assert_eq!(zoom_label(0.4204, false), "42 %");
    }

    #[test]
    fn selection_headings() {
        assert_eq!(
            selection_heading(Some(Image::TYPE), 1, 0),
            ("Image".to_string(), "1 selected".to_string())
        );
        assert_eq!(
            selection_heading(Some(Group::TYPE), 1, 2),
            ("Group".to_string(), "2 objects".to_string())
        );
        assert_eq!(
            selection_heading(Some(Image::TYPE), 3, 0),
            ("3 Images".to_string(), "Shared properties".to_string())
        );
        assert_eq!(selection_heading(None, 3, 0).0, "3 Objects", "mixed types");
        assert_eq!(
            select_all_label(Image::TYPE).as_deref(),
            Some("Select All Images")
        );
        assert_eq!(select_all_label("arrow"), None);
        assert_eq!(editable_objects(7), "7 editable objects");
        assert_eq!(editable_objects(1), "1 editable object");
    }

    #[test]
    fn autosaved_rows_name_the_day() {
        let tz = glib::TimeZone::utc();
        let at = |d: i32, h: i32, m: i32| glib::DateTime::new(&tz, 2026, 10, d, h, m, 0.0).unwrap();
        let now = at(30, 12, 0);
        assert_eq!(
            autosaved(&at(30, 10, 42), &now, 7),
            "Autosaved today at 10:42 · 7 objects"
        );
        assert_eq!(
            autosaved(&at(29, 23, 5), &now, 1),
            "Autosaved yesterday at 23:05 · 1 object"
        );
        let older = autosaved(&at(12, 8, 0), &now, 12);
        assert!(
            older.starts_with("Autosaved on ") && older.ends_with(" at 08:00 · 12 objects"),
            "{older}"
        );
    }

    #[test]
    fn newer_version_body_matches_mockup_15() {
        let body = newer_version_body(
            "Release notes v2.annota",
            FormatVersion { major: 2, minor: 0 },
            Some("2.1"),
            "0.1.0",
        );
        assert_eq!(
            body,
            "“Release notes v2.annota” uses document format 2.0. It was saved with Annota 2.1 \
             or newer.\n\nThis is Annota 0.1, which opens format 1.x. Install Annota 2.1 or \
             newer to open this file. The file was not changed."
        );
        let body = newer_version_body(
            "x.annota",
            FormatVersion { major: 3, minor: 1 },
            None,
            "0.1.0",
        );
        assert!(body.contains("format 3.1.") && body.contains("Install a newer Annota"));
    }
}
