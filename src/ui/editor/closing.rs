//! "Save Changes?" (add-export-share task 2.6, mockup 15; redline "Save changes before closing
//! (15)"; spec document-format "Save changes before closing"): asked for each document with
//! unsaved changes when the editor window closes or Annota quits. Which documents count, the
//! body text and the answers are here, free of widgets; [`ask`] shows the alert from
//! `save-changes-dialog.ui`. The order, switching documents and what Save and Discard do are
//! the editor's (`Editor::confirm_close`).

use adw::prelude::*;
use gettextrs::gettext;

use super::session::Origin;
use crate::ui::canvas::HistoryInfo;

const UI_SAVE_CHANGES: &str = "/io/github/trosinde/Annota/ui/save-changes-dialog.ui";

/// What the user answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// Cancel, Esc or closing the alert: the whole close or quit stops.
    Cancel,
    /// Close without saving; the autosave goes.
    Discard,
    /// What `Ctrl+S` does, then close.
    Save,
}

impl Answer {
    /// The alert's response id (`cancel`, `discard`, `save`); anything else is Cancel.
    pub fn from_response(id: &str) -> Self {
        match id {
            "save" => Answer::Save,
            "discard" => Answer::Discard,
            _ => Answer::Cancel,
        }
    }
}

/// Whether closing a document loses something, so "Save Changes?" asks first. A document of a
/// `.annota` file, or an image opened from a file, only once it differs from that file (an
/// untouched image is on disk already); a new capture or pasted image, and a document restored
/// from an autosave, always — they exist nowhere else.
pub fn needs_saving(origin: Origin, info: &HistoryInfo) -> bool {
    match origin {
        Origin::File | Origin::Image => info.dirty,
        Origin::New | Origin::Restored => true,
    }
}

/// The redline's two bodies: a document that has its own `.annota` file ("has unsaved changes")
/// or one that was never saved (a new capture, a pasted image, an image opened from a file and
/// changed: "has never been saved"). `name` is the header bar's title.
pub fn body(name: &str, saved_before: bool) -> String {
    let text = if saved_before {
        gettext("“{name}” has unsaved changes. If you close without saving, they will be lost.")
    } else {
        gettext("“{name}” has never been saved. If you close without saving, it will be lost.")
    };
    text.replace("{name}", name)
}

/// Shows "Save Changes?" for the document `name` over `parent` and waits for the answer. A
/// broken UI resource answers Cancel, so nothing is lost unasked.
pub async fn ask(parent: &impl IsA<gtk::Widget>, name: &str, saved_before: bool) -> Answer {
    let builder = gtk::Builder::from_resource(UI_SAVE_CHANGES);
    let Some(dialog) = builder.object::<adw::AlertDialog>("save_changes_dialog") else {
        eprintln!("annota: save-changes-dialog.ui is missing object 'save_changes_dialog'");
        return Answer::Cancel;
    };
    dialog.set_body(&body(name, saved_before));
    let response = dialog.choose_future(parent).await;
    Answer::from_response(&response)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(revision: u64, dirty: bool) -> HistoryInfo {
        HistoryInfo {
            revision,
            dirty,
            ..HistoryInfo::default()
        }
    }

    #[test]
    fn which_documents_are_asked_about() {
        // Redline: "Documents without unsaved changes close silently."
        assert!(!needs_saving(Origin::File, &info(0, false)));
        assert!(needs_saving(Origin::File, &info(2, true)));
        // An image opened from a file: only once it is changed (its pixels are on disk).
        assert!(!needs_saving(Origin::Image, &info(0, false)));
        assert!(needs_saving(Origin::Image, &info(1, true)));
        // A new capture or pasted image exists nowhere else, changed or not.
        assert!(needs_saving(Origin::New, &info(0, true)));
        assert!(needs_saving(Origin::New, &info(3, true)));
        // Restored from an autosave: it holds changes no file has.
        assert!(needs_saving(Origin::Restored, &info(0, true)));
    }

    #[test]
    fn the_body_tells_saved_before_from_never_saved() {
        // Mockup 15 and the redline's two variants.
        assert_eq!(
            body("Checkout flow", true),
            "“Checkout flow” has unsaved changes. If you close without saving, they will be lost."
        );
        assert_eq!(
            body("Capture 2026-10-31 09-00-00", false),
            "“Capture 2026-10-31 09-00-00” has never been saved. If you close without saving, it will be lost."
        );
    }

    #[test]
    fn responses_become_answers() {
        assert_eq!(Answer::from_response("save"), Answer::Save);
        assert_eq!(Answer::from_response("discard"), Answer::Discard);
        assert_eq!(Answer::from_response("cancel"), Answer::Cancel);
        // Closed some other way: the close-response, Cancel.
        assert_eq!(Answer::from_response(""), Answer::Cancel);
    }
}
