//! The screenshot permission (spec: Portal permission handling): what the portal's permission
//! store says, what a capture does about it, and the "Screen Access Denied" dialog (mockup 08).
//!
//! xdg-desktop-portal (`src/screenshot.c`) asks once, on the first non-interactive request of an
//! app with no stored answer, and stores Allow as `yes` and Deny as `no`. An interactive request
//! (window mode) is never checked. If the question cannot be shown, nothing is stored and the
//! answer is the same response code 2 as for a stored `no`, so only the store can tell them apart.
//!
//! Annota only reads the store. The portal writes it, and only the user resets it
//! ([`reset_command`]): GNOME Settings has no switch for it unless the app is a Flatpak or Snap
//! (`get_portal_app_id` in gnome-control-center's `cc-applications-panel.c`).

use std::collections::HashMap;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

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
/// The store's error for a table or entry that doesn't exist yet.
const STORE_NOT_FOUND: &str = "org.freedesktop.portal.Error.NotFound";

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

/// The state when `Lookup` failed. "No such entry" is an answer; anything else says nothing.
pub fn state_after_error(message: &str) -> StoreState {
    if message.contains(STORE_NOT_FOUND) {
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
}

/// Window mode is an interactive request, which the portal never checks. An unreadable store
/// doesn't ask first: if the permission does exist, Annota's window would flash up every time.
pub fn first_step(mode: CaptureMode, store: StoreState) -> FirstStep {
    match (mode, store) {
        (CaptureMode::Region | CaptureMode::Screen, StoreState::NotDecided) => FirstStep::Ask,
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

/// The command that removes Annota's stored answer, so that GNOME asks again.
pub fn reset_command() -> String {
    format!(
        "gdbus call --session --dest {STORE_BUS_NAME} --object-path {STORE_PATH} \
         --method {STORE_INTERFACE}.DeletePermission {STORE_TABLE} {STORE_ID} {}",
        config::APP_ID
    )
}

pub fn present(app: &adw::Application) {
    let window = application::present_window(app);
    let builder = gtk::Builder::from_resource(UI_PERMISSION);
    let (Some(dialog), Some(close), Some(copy), Some(command)) = (
        builder.object::<adw::Dialog>("permission_dialog"),
        builder.object::<gtk::Button>("close_button"),
        builder.object::<gtk::Button>("copy_command_button"),
        builder.object::<gtk::Label>("reset_command_label"),
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
    command.set_text(&reset_command());
    copy.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        window,
        move |_| {
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&reset_command());
            }
            dialog.close();
            application::show_toast(&window, &adw::Toast::new(&gettext("Copied to clipboard")));
        }
    ));
    dialog.set_default_widget(Some(&copy));
    dialog.set_focus(Some(&copy));
    dialog.present(Some(&window));
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
        // Once permission exists, or was denied, no Annota window stays on the screen.
        for mode in [Region, Screen] {
            assert_eq!(first_step(mode, Allowed), FirstStep::Grab);
            assert_eq!(first_step(mode, Denied), FirstStep::Grab);
            assert_eq!(first_step(mode, Unreadable), FirstStep::Grab);
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
    fn the_reset_command_deletes_annotas_entry_only() {
        assert_eq!(
            reset_command(),
            "gdbus call --session --dest org.freedesktop.impl.portal.PermissionStore \
             --object-path /org/freedesktop/impl/portal/PermissionStore \
             --method org.freedesktop.impl.portal.PermissionStore.DeletePermission \
             screenshot screenshot io.github.trosinde.Annota"
        );
    }
}
