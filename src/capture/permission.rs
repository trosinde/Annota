//! The screenshot permission (spec: Portal permission handling): what the portal's permission
//! store says, what a capture does about it, and the "Screen Access Denied" dialog (mockup 08).
//!
//! xdg-desktop-portal (`src/screenshot.c`) asks once, on the first non-interactive request of an
//! app with no stored answer, and stores Allow as `yes` and Deny as `no`. An interactive request
//! (window mode) is never checked. If the question cannot be shown, nothing is stored and the
//! answer is the same response code 2 as for a stored `no`, so only the store can tell them apart.
//!
//! The portal writes the answers. Annota reads them, and writes in one case only: Ask Again in the
//! dialog deletes Annota's own entry ([`forget_answer`]), because GNOME Settings has no switch for
//! it unless the app is a Flatpak or Snap (`get_portal_app_id` in gnome-control-center's
//! `cc-applications-panel.c`). Nothing else ever changes the store, and a delete grants nothing:
//! the next frozen capture finds no answer, so GNOME asks.

use std::collections::HashMap;
use std::time::Duration;

use adw::prelude::*;
use gtk::{gio, glib};

use crate::cli::CaptureMode;
use crate::{application, config};

const UI_PERMISSION: &str = "/io/github/trosinde/Annota/ui/permission-dialog.ui";

const STORE_BUS_NAME: &str = "org.freedesktop.impl.portal.PermissionStore";
const STORE_PATH: &str = "/org/freedesktop/impl/portal/PermissionStore";
const STORE_INTERFACE: &str = "org.freedesktop.impl.portal.PermissionStore";
const STORE_TABLE: &str = "screenshot";
const STORE_ID: &str = "screenshot";
/// A store that answers late must not hold up a capture for long.
const STORE_TIMEOUT: Duration = Duration::from_secs(2);
/// The store replies to a delete after writing its file. The user waits for this with the dialog
/// held open, so it stays short, but a slow disk must not look like a failure.
const STORE_DELETE_TIMEOUT: Duration = Duration::from_secs(5);
/// The store's error for a table or entry that doesn't exist yet.
const STORE_NOT_FOUND: &str = "org.freedesktop.portal.Error.NotFound";

thread_local! {
    /// The "Screen Access Denied" dialog while it is open, so a second capture doesn't stack another.
    static OPEN_DIALOG: glib::WeakRef<adw::Dialog> = glib::WeakRef::new();
}

/// What the permission store says about Annota and screenshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreState {
    /// `yes`: non-interactive requests work without a question.
    Allowed,
    /// `no`: the portal refuses without a question.
    Denied,
    /// No answer stored (or `ask`, which nothing in GNOME writes): the portal will ask.
    NotDecided,
    /// The store could not be read, e.g. from inside a sandbox.
    Unreadable,
}

/// The state of `app_id` among the store's entries. Like the portal, reads the first value only.
pub fn state_of(entries: &HashMap<String, Vec<String>>, app_id: &str) -> StoreState {
    let value = entries.get(app_id).and_then(|values| values.first());
    match value.map(String::as_str) {
        Some("yes") => StoreState::Allowed,
        Some("no") => StoreState::Denied,
        _ => StoreState::NotDecided,
    }
}

/// Whether a store error says that there is no such table or entry.
fn is_not_found(message: &str) -> bool {
    message.contains(STORE_NOT_FOUND)
}

/// The state when `Lookup` failed. "No such entry" is an answer; anything else says nothing.
pub fn state_after_error(message: &str) -> StoreState {
    if is_not_found(message) {
        StoreState::NotDecided
    } else {
        StoreState::Unreadable
    }
}

/// Reads Annota's screenshot permission. Read-only: `Lookup` never changes the store.
pub async fn read_store() -> StoreState {
    let connection = match gio::bus_get_future(gio::BusType::Session).await {
        Ok(connection) => connection,
        Err(err) => {
            eprintln!("annota: cannot read the permission store: {err}");
            return StoreState::Unreadable;
        }
    };
    let answer = connection
        .call_future(
            Some(STORE_BUS_NAME),
            STORE_PATH,
            STORE_INTERFACE,
            "Lookup",
            Some(&(STORE_TABLE, STORE_ID).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            STORE_TIMEOUT.as_millis() as i32,
        )
        .await;
    match answer {
        Ok(answer) => answer
            .try_child_get::<HashMap<String, Vec<String>>>(0)
            .ok()
            .flatten()
            .map_or(StoreState::Unreadable, |entries| {
                state_of(&entries, config::APP_ID)
            }),
        Err(err) => {
            let state = state_after_error(err.message());
            if state == StoreState::Unreadable {
                eprintln!("annota: cannot read the permission store: {err}");
            }
            state
        }
    }
}

/// How a capture starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstStep {
    /// Hide Annota's windows and grab.
    Grab,
    /// The portal will ask: keep a window on screen for the question first.
    Ask,
    /// A stored "no": the portal would refuse, so the dialog comes at once. No window hides, no
    /// countdown runs.
    Denied,
}

/// Window mode is an interactive request, which the portal never checks. An unreadable store
/// doesn't ask first: if the permission does exist, Annota's window would flash up every time.
pub fn first_step(mode: CaptureMode, store: StoreState) -> FirstStep {
    match (mode, store) {
        (CaptureMode::Region | CaptureMode::Screen, StoreState::NotDecided) => FirstStep::Ask,
        (CaptureMode::Region | CaptureMode::Screen, StoreState::Denied) => FirstStep::Denied,
        _ => FirstStep::Grab,
    }
}

/// What a [`CaptureError::Refused`](super::portal::CaptureError::Refused) means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A stored "no": the "Screen Access Denied" dialog.
    Denied,
    /// Nothing stored and no question shown yet: ask, with a window on screen.
    Ask,
    /// A window was on screen and still no answer was stored.
    CouldNotAsk,
    /// The permission is not the reason.
    Failed,
}

/// Sorts a refusal by the store as it is now. `asked`: a window was on screen for the question.
pub fn after_refusal(mode: CaptureMode, store: StoreState, asked: bool) -> Refusal {
    match (mode, store) {
        (CaptureMode::Window, _) | (_, StoreState::Allowed) => Refusal::Failed,
        (_, StoreState::Denied) => Refusal::Denied,
        (_, StoreState::NotDecided | StoreState::Unreadable) if asked => Refusal::CouldNotAsk,
        (_, StoreState::NotDecided | StoreState::Unreadable) => Refusal::Ask,
    }
}

/// What `DeletePermission` is called with: Annota's own entry and no other app's.
fn delete_parameters() -> glib::Variant {
    (STORE_TABLE, STORE_ID, config::APP_ID).to_variant()
}

/// Removes Annota's stored answer. An entry that is already gone is what was asked for.
async fn forget_answer() -> Result<(), glib::Error> {
    let connection = gio::bus_get_future(gio::BusType::Session).await?;
    let answer = connection
        .call_future(
            Some(STORE_BUS_NAME),
            STORE_PATH,
            STORE_INTERFACE,
            "DeletePermission",
            Some(&delete_parameters()),
            None,
            gio::DBusCallFlags::NONE,
            STORE_DELETE_TIMEOUT.as_millis() as i32,
        )
        .await;
    match answer {
        Err(err) if !is_not_found(err.message()) => Err(err),
        _ => Ok(()),
    }
}

/// Shows the "Screen Access Denied" dialog, or raises it if it is already open.
pub fn present(app: &adw::Application) {
    let open = OPEN_DIALOG.with(glib::WeakRef::upgrade);
    if let Some(window) = open.and_then(|dialog| dialog.root()) {
        if let Ok(window) = window.downcast::<gtk::Window>() {
            window.present();
            return;
        }
    }
    let window = application::present_for_message(app);
    let builder = gtk::Builder::from_resource(UI_PERMISSION);
    let (Some(dialog), Some(close), Some(ask_again), Some(failed)) = (
        builder.object::<adw::Dialog>("permission_dialog"),
        builder.object::<gtk::Button>("close_button"),
        builder.object::<gtk::Button>("ask_again_button"),
        builder.object::<gtk::Label>("reset_failed_label"),
    ) else {
        eprintln!("annota: permission-dialog.ui is incomplete");
        return;
    };
    close.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    ask_again.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        close,
        #[weak]
        failed,
        move |ask_again| {
            // Insensitive while a delete runs. `clicked` can still be emitted, so check here too.
            if ask_again.is_sensitive() {
                reset(&dialog, ask_again, &close, &failed);
            }
        }
    ));
    dialog.set_default_widget(Some(&ask_again));
    dialog.set_focus(Some(&ask_again));
    OPEN_DIALOG.with(|open| open.set(Some(&dialog)));
    dialog.connect_closed(|_| OPEN_DIALOG.with(|open| open.set(None::<&adw::Dialog>)));
    dialog.present(Some(&window));
    // The mockup draws the focus ring. GTK would keep it back until the first key press.
    window.set_focus_visible(true);
}

/// Ask Again. Close and Esc wait for the answer: once the delete is sent they could no longer
/// keep the stored answer as they promise. The wait ends at [`STORE_DELETE_TIMEOUT`].
fn reset(dialog: &adw::Dialog, ask_again: &gtk::Button, close: &gtk::Button, failed: &gtk::Label) {
    failed.set_visible(false);
    ask_again.set_sensitive(false);
    close.set_sensitive(false);
    dialog.set_can_close(false);
    let (dialog, ask_again, close, failed) = (
        dialog.clone(),
        ask_again.clone(),
        close.clone(),
        failed.clone(),
    );
    glib::spawn_future_local(async move {
        let result = forget_answer().await;
        dialog.set_can_close(true);
        match result {
            Ok(()) => {
                dialog.close();
            }
            Err(err) => {
                eprintln!("annota: cannot reset the screenshot permission: {err}");
                failed.set_visible(true);
                close.set_sensitive(true);
                ask_again.set_sensitive(true);
                dialog.set_focus(Some(&ask_again));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "io.github.trosinde.Annota";

    fn entries(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(id, values)| {
                let values = values.iter().map(|v| (*v).to_string()).collect();
                ((*id).to_string(), values)
            })
            .collect()
    }

    #[test]
    fn the_store_entry_decides_for_this_app_only() {
        use StoreState::*;
        assert_eq!(state_of(&entries(&[(ID, &["yes"])]), ID), Allowed);
        assert_eq!(state_of(&entries(&[(ID, &["no"])]), ID), Denied);
        assert_eq!(state_of(&entries(&[(ID, &["ask"])]), ID), NotDecided);
        assert_eq!(state_of(&entries(&[(ID, &[])]), ID), NotDecided);
        assert_eq!(state_of(&entries(&[]), ID), NotDecided);
        assert_eq!(
            state_of(&entries(&[("org.other", &["no"])]), ID),
            NotDecided
        );
        // Unsandboxed apps without an ID share the "" entry; the portal knows Annota's own ID.
        assert_eq!(state_of(&entries(&[("", &["yes"])]), ID), NotDecided);
    }

    #[test]
    fn not_found_is_an_answer_other_errors_are_not() {
        // As `Lookup` really answers on the maintainer's GNOME 50 with nothing stored.
        assert_eq!(
            state_after_error(
                "GDBus.Error:org.freedesktop.portal.Error.NotFound: No entry for screenshot"
            ),
            StoreState::NotDecided
        );
        assert_eq!(
            state_after_error(
                "GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable"
            ),
            StoreState::Unreadable
        );
        assert_eq!(
            state_after_error("Timeout was reached"),
            StoreState::Unreadable
        );
    }

    #[test]
    fn only_an_undecided_frozen_grab_asks_first() {
        use CaptureMode::*;
        use StoreState::*;
        assert_eq!(first_step(Region, NotDecided), FirstStep::Ask);
        assert_eq!(first_step(Screen, NotDecided), FirstStep::Ask);
        // Once permission exists no Annota window stays on the screen; a "no" needs no grab.
        for mode in [Region, Screen] {
            assert_eq!(first_step(mode, Allowed), FirstStep::Grab);
            assert_eq!(first_step(mode, Unreadable), FirstStep::Grab);
            assert_eq!(first_step(mode, Denied), FirstStep::Denied);
        }
        // The window picker is never checked by the portal.
        for store in [Allowed, Denied, NotDecided, Unreadable] {
            assert_eq!(first_step(Window, store), FirstStep::Grab);
        }
    }

    #[test]
    fn only_a_stored_no_is_a_denial() {
        use CaptureMode::*;
        use StoreState::*;
        for asked in [false, true] {
            assert_eq!(after_refusal(Region, Denied, asked), Refusal::Denied);
            assert_eq!(after_refusal(Region, Allowed, asked), Refusal::Failed);
            // The picker isn't checked, so even a stored "no" didn't cause this.
            assert_eq!(after_refusal(Window, Denied, asked), Refusal::Failed);
            assert_eq!(after_refusal(Window, NotDecided, asked), Refusal::Failed);
        }
        // Nothing stored: the journal's "Only the focused app …" case.
        assert_eq!(
            after_refusal(Region, NotDecided, true),
            Refusal::CouldNotAsk
        );
        assert_eq!(
            after_refusal(Screen, Unreadable, true),
            Refusal::CouldNotAsk
        );
        // One more try, with a window; `asked` then ends it.
        assert_eq!(after_refusal(Region, NotDecided, false), Refusal::Ask);
        assert_eq!(after_refusal(Screen, Unreadable, false), Refusal::Ask);
    }

    #[test]
    fn the_delete_names_annotas_entry_only() {
        assert_eq!(
            delete_parameters().get::<(String, String, String)>(),
            Some((
                String::from("screenshot"),
                String::from("screenshot"),
                String::from(ID)
            ))
        );
    }

    #[test]
    fn only_not_found_counts_as_already_deleted() {
        assert!(is_not_found(
            "GDBus.Error:org.freedesktop.portal.Error.NotFound: No entry for screenshot"
        ));
        assert!(!is_not_found(
            "GDBus.Error:org.freedesktop.DBus.Error.AccessDenied: Sandboxed"
        ));
        assert!(!is_not_found("Timeout was reached"));
    }
}
