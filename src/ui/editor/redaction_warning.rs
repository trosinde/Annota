//! "Make Redactions Solid?" (add-export-share task 2.7, mockup 10; spec annotation-tools
//! "Redaction export warning"; decision D7): asked before a document with blur or pixelate
//! redactions leaves Annota. The texts are here, free of widgets; [`present`] shows the alert
//! from `redaction-warning.ui`. Which redactions there are and the conversion itself are
//! `annota::editing::redactions`; the editor's `guard_redactions` puts the three together and
//! is the one entry point every way out of Annota calls.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::glib;

use annota::editing::redactions::Reversible;

const UI_WARNING: &str = "/io/github/trosinde/Annota/ui/redaction-warning.ui";

/// The way out of Annota the warning stands in front of; it names the middle button. Quick
/// save and Save As don't go through this warning at all (`.annota` keeps the original image
/// regardless of the choice here, so it wouldn't apply) — see `Editor::guard_redactions`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// The export dialog (Ctrl+E): "Export Anyway" (mockup 10).
    Export,
}

/// What the user chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    /// Cancel or Escape: nothing leaves Annota.
    Cancel,
    /// Export (or save) as it is.
    Anyway,
    /// Convert to Solid (one undo step), then go on.
    Convert,
}

/// Called once with the user's choice.
type Respond = Box<dyn FnOnce(Response)>;

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Mockup 10's body: "4 redactions use pixelate. Pixelated text can sometimes be
/// reconstructed. Solid boxes hide it for good." — "use blur" / "Blurred text …" for blur, and
/// both named when there are both.
pub fn body(found: Reversible) -> String {
    let n = found.total();
    let text = match (found.blur, found.pixelate) {
        (0, _) => ngettext(
            "{n} redaction uses pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good.",
            "{n} redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good.",
            count(n),
        ),
        (_, 0) => ngettext(
            "{n} redaction uses blur. Blurred text can sometimes be reconstructed. Solid boxes hide it for good.",
            "{n} redactions use blur. Blurred text can sometimes be reconstructed. Solid boxes hide it for good.",
            count(n),
        ),
        _ => ngettext(
            "{n} redaction uses blur or pixelate. Blurred or pixelated text can sometimes be reconstructed. Solid boxes hide it for good.",
            "{n} redactions use blur or pixelate. Blurred or pixelated text can sometimes be reconstructed. Solid boxes hide it for good.",
            count(n),
        ),
    };
    text.replace("{n}", &n.to_string())
}

/// The middle button: "Export Anyway" (mockup 10).
pub fn anyway_label(purpose: Purpose) -> String {
    match purpose {
        Purpose::Export => gettext("Export Anyway"),
    }
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: redaction-warning.ui is missing object '{}'", $name);
                return None;
            }
        }
    };
}

/// Shows the alert over `parent` for `found` (not empty) and calls `respond` once with the
/// choice; closing the dialog any other way (Escape) is Cancel. Returns the dialog, or `None`
/// if the UI resource is broken (then `respond` gets Cancel right away, so nothing leaves
/// Annota unasked).
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    found: Reversible,
    purpose: Purpose,
    respond: impl FnOnce(Response) + 'static,
) -> Option<adw::Dialog> {
    let respond: Rc<Cell<Option<Respond>>> = Rc::new(Cell::new(Some(Box::new(respond))));
    let answer = {
        let respond = respond.clone();
        move |response: Response| {
            if let Some(respond) = respond.take() {
                respond(response);
            }
        }
    };
    let built = (|| {
        let builder = gtk::Builder::from_resource(UI_WARNING);
        let dialog: adw::Dialog = obj!(builder, "redaction_warning");
        let body_label: gtk::Label = obj!(builder, "body_label");
        let cancel: gtk::Button = obj!(builder, "cancel_button");
        let anyway: gtk::Button = obj!(builder, "anyway_button");
        let convert: gtk::Button = obj!(builder, "convert_button");
        Some((dialog, body_label, cancel, anyway, convert))
    })();
    let Some((dialog, body_label, cancel, anyway, convert)) = built else {
        answer(Response::Cancel);
        return None;
    };
    body_label.set_text(&body(found));
    anyway.set_label(&anyway_label(purpose));
    for (button, response) in [
        (&cancel, Response::Cancel),
        (&anyway, Response::Anyway),
        (&convert, Response::Convert),
    ] {
        let answer = answer.clone();
        button.connect_clicked(glib::clone!(
            #[weak]
            dialog,
            move |_| {
                // Answer first: `closed` below would otherwise answer Cancel.
                answer(response);
                dialog.close();
            }
        ));
    }
    dialog.connect_closed(move |_| answer(Response::Cancel));
    dialog.set_default_widget(Some(&convert));
    dialog.present(Some(parent));
    // Enter converts (the suggested response).
    dialog.set_focus(Some(&convert));
    Some(dialog)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_names_the_count_and_the_kind() {
        // Mockup 10.
        assert_eq!(
            body(Reversible {
                blur: 0,
                pixelate: 4
            }),
            "4 redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good."
        );
        assert_eq!(
            body(Reversible {
                blur: 1,
                pixelate: 0
            }),
            "1 redaction uses blur. Blurred text can sometimes be reconstructed. Solid boxes hide it for good."
        );
        assert_eq!(
            body(Reversible {
                blur: 2,
                pixelate: 1
            }),
            "3 redactions use blur or pixelate. Blurred or pixelated text can sometimes be reconstructed. Solid boxes hide it for good."
        );
    }

    #[test]
    fn the_middle_button_names_the_way_out() {
        assert_eq!(anyway_label(Purpose::Export), "Export Anyway");
    }
}
