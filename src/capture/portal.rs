//! Screen access through the xdg-desktop-portal Screenshot interface (CLAUDE.md §2: no other
//! way to read the screen).
//!
//! Written against `ashpd` 0.11's documented API. Not exercised against a live portal in the
//! development environment (it is the maintainer's real desktop); see `manual-checks.md`.

use std::cell::RefCell;
use std::rc::Rc;

use ashpd::desktop::screenshot::Screenshot;
use ashpd::desktop::ResponseError;
use ashpd::zbus;
use ashpd::{PortalError, WindowIdentifier};
use futures_util::future::{FutureExt, LocalBoxFuture, Shared};
use gtk::{gdk, gio, glib, prelude::*};

use super::frame::Frame;
use super::texture;
use crate::cli::CaptureMode;
use crate::config;

/// Why a capture produced no image.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    /// The user dismissed the portal's own UI (or pressed Esc in Annota's overlay).
    #[error("the capture was cancelled")]
    Cancelled,
    /// The portal ended the request without an image and without saying why. A denial, a
    /// dialog that could not be shown and an internal failure all look like this; `capture`
    /// tells them apart with the permission store (`permission::after_refusal`).
    #[error("the portal refused the request")]
    Refused,
    /// The permission store says Annota may not take screenshots (spec: Portal permission
    /// handling).
    #[error("screen access was denied")]
    PermissionDenied,
    /// The permission is undecided and GNOME could not show its question.
    #[error("GNOME could not ask for screen access")]
    CouldNotAsk,
    /// No xdg-desktop-portal Screenshot implementation is running.
    #[error("no screenshot portal is available")]
    PortalUnavailable,
    /// The portal answered but its image could not be loaded.
    #[error("the screenshot could not be read: {0}")]
    Unreadable(String),
    /// Any other portal or D-Bus failure.
    #[error("the screenshot portal failed: {0}")]
    Failed(String),
}

/// D-Bus errors meaning "nobody implements this interface".
const MISSING_SERVICE_ERRORS: [&str; 4] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.UnknownMethod",
    "org.freedesktop.DBus.Error.UnknownInterface",
    "org.freedesktop.DBus.Error.UnknownObject",
];

/// Maps an `ashpd` error to what Annota shows the user.
///
/// Response code 2 ("other") is not a denial as such. xdg-desktop-portal also answers it when its
/// access dialog could not be shown, which writes nothing to the permission store, and when its
/// backend fails (`src/screenshot.c`), so it maps to [`CaptureError::Refused`].
pub fn map_portal_error(err: &ashpd::Error) -> CaptureError {
    match err {
        ashpd::Error::Response(ResponseError::Cancelled) => CaptureError::Cancelled,
        ashpd::Error::Response(ResponseError::Other) => CaptureError::Refused,
        ashpd::Error::Portal(PortalError::NotAllowed(_)) => CaptureError::Refused,
        ashpd::Error::Portal(PortalError::Cancelled(_)) => CaptureError::Cancelled,
        ashpd::Error::PortalNotFound(_) => CaptureError::PortalUnavailable,
        ashpd::Error::Zbus(dbus) | ashpd::Error::Portal(PortalError::ZBus(dbus))
            if is_missing_service(dbus) =>
        {
            CaptureError::PortalUnavailable
        }
        other => CaptureError::Failed(other.to_string()),
    }
}

/// Whether a D-Bus error means "no portal is running" rather than a failure of one.
fn is_missing_service(err: &zbus::Error) -> bool {
    match err {
        zbus::Error::MethodError(name, _, _) => MISSING_SERVICE_ERRORS.contains(&name.as_str()),
        zbus::Error::FDO(fdo) => matches!(
            fdo.as_ref(),
            zbus::fdo::Error::ServiceUnknown(_)
                | zbus::fdo::Error::UnknownMethod(_)
                | zbus::fdo::Error::UnknownInterface(_)
                | zbus::fdo::Error::UnknownObject(_)
        ),
        _ => false,
    }
}

/// How the portal is asked for the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrabOptions {
    /// `true` shows the desktop's own screenshot UI (window picker on GNOME).
    pub interactive: bool,
    pub modal: bool,
}

impl GrabOptions {
    /// Frozen full-desktop image for Annota's own overlay (region and screen modes).
    pub const FROZEN: GrabOptions = GrabOptions {
        interactive: false,
        modal: true,
    };
    /// The portal's interactive picker (window mode, mockup 05).
    pub const PICKER: GrabOptions = GrabOptions {
        interactive: true,
        modal: true,
    };

    /// Window mode uses the desktop's picker; region and screen freeze the whole desktop
    /// for Annota's own overlay.
    pub fn for_mode(mode: CaptureMode) -> GrabOptions {
        match mode {
            CaptureMode::Window => Self::PICKER,
            CaptureMode::Region | CaptureMode::Screen => Self::FROZEN,
        }
    }
}

thread_local! {
    static REGISTRATION: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>> =
        const { RefCell::new(None) };
}

/// Registers Annota's app ID with xdg-desktop-portal (≥ 1.19) once per process, so that
/// the access dialog and the stored permission refer to Annota rather than "unknown app".
/// Older portals don't have the registry; that's fine and ignored.
///
/// Every caller waits for the one registration to finish. A request overtaking it would reach
/// the portal without Annota's ID: at startup the shortcut setup registers while a cold
/// `annota --capture` is already on its way to its first grab.
pub async fn ensure_registered() {
    let registration = REGISTRATION.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(|| register().boxed_local().shared())
            .clone()
    });
    registration.await;
}

async fn register() {
    if let Ok(app_id) = ashpd::AppID::try_from(config::APP_ID) {
        // Older portals (before 1.19) have no registry; nothing to do then.
        let _ = ashpd::register_host_app(app_id).await;
    }
}

/// The parent of a portal request, for as long as the request runs.
///
/// `ashpd` 0.11.1's identifier unexports a Wayland handle on drop and unwraps the window's surface
/// for that, so it panics if the window has been closed. A request can run for as long as the user
/// leaves GNOME's question open, so the request gets a plain copy of the handle
/// ([`ParentWindow::for_request`]) and this keeps the real identifier, which goes when the request
/// ends or, before the surface does, when the window closes.
struct ParentWindow {
    window: gtk::Window,
    identifier: Rc<RefCell<Option<WindowIdentifier>>>,
    on_unrealize: Option<glib::SignalHandlerId>,
}

impl ParentWindow {
    async fn new(window: &gtk::Window) -> Self {
        let identifier = WindowIdentifier::from_native(window).await;
        let identifier = Rc::new(RefCell::new(identifier));
        // Runs ahead of GTK's own handler, which destroys the surface.
        let on_unrealize = window.connect_unrealize({
            let identifier = identifier.clone();
            move |window| release(&identifier, window)
        });
        Self {
            window: window.clone(),
            identifier,
            on_unrealize: Some(on_unrealize),
        }
    }

    fn for_request(&self) -> Option<WindowIdentifier> {
        self.identifier.borrow().as_ref().and_then(plain_copy)
    }

    #[cfg(test)]
    fn is_held(&self) -> bool {
        self.identifier.borrow().is_some()
    }
}

impl Drop for ParentWindow {
    fn drop(&mut self) {
        if let Some(handler) = self.on_unrealize.take() {
            self.window.disconnect(handler);
        }
        release(&self.identifier, &self.window);
    }
}

/// Gives up the identifier. Without a surface, which only happens if the window was closed while
/// `from_native` waited for the compositor, its drop would panic. Forgetting it leaks one reference
/// to a closed window, and the export went with the surface.
fn release(identifier: &RefCell<Option<WindowIdentifier>>, window: &gtk::Window) {
    let Some(identifier) = identifier.borrow_mut().take() else {
        return;
    };
    if window.surface().is_none() {
        std::mem::forget(identifier);
    }
}

/// The same handle without `ashpd`'s drop. The `X11` variant holds either kind of handle.
fn plain_copy(identifier: &WindowIdentifier) -> Option<WindowIdentifier> {
    let handle = identifier.to_string().parse().ok()?;
    Some(WindowIdentifier::X11(handle))
}

/// One Screenshot request; the answer is the file the portal wrote.
async fn request_file(
    parent: Option<WindowIdentifier>,
    options: GrabOptions,
) -> Result<gio::File, CaptureError> {
    ensure_registered().await;
    let request = Screenshot::request()
        .identifier(parent)
        .interactive(options.interactive)
        .modal(options.modal)
        .send()
        .await
        .map_err(|err| map_portal_error(&err))?;
    let screenshot = request.response().map_err(|err| map_portal_error(&err))?;
    Ok(gio::File::for_uri(screenshot.uri().as_str()))
}

/// Makes the portal ask for the screenshot permission, over `window`.
///
/// With a parent handle `xdg-desktop-portal-gnome` shows the question as a dialog of that window.
/// Without one it passes the question on to GNOME Shell, which refuses unless Annota is the
/// focused app. On "Allow" the portal grabs the screen at once, with `window` in the picture, so
/// that image is thrown away unread and `capture` grabs again with its windows hidden.
pub async fn ask_permission(window: &gtk::Window) -> Result<(), CaptureError> {
    let parent = ParentWindow::new(window).await;
    let identifier = parent.for_request();
    if identifier.is_none() {
        eprintln!("annota: no parent window handle for the screenshot permission dialog");
    }
    let file = request_file(identifier, GrabOptions::FROZEN).await?;
    drop(parent);
    discard_portal_file(&file);
    Ok(())
}

/// Asks the Screenshot portal for an image and loads it.
pub async fn grab(options: GrabOptions) -> Result<Frame, CaptureError> {
    let file = request_file(None, options).await?;
    let loaded = gdk::Texture::from_file(&file);
    discard_portal_file(&file);
    let texture = loaded.map_err(|err| CaptureError::Unreadable(err.to_string()))?;
    texture::frame_from_texture(&texture)
        .ok_or_else(|| CaptureError::Unreadable(String::from("empty image")))
}

/// Asks the Screenshot portal for the colour of one pixel anywhere on the screen (`PickColor`,
/// add-core-annotation-tools redline "Eyedropper"): the desktop shows its own pointer — and a
/// loupe if it has one — and answers with the colour as three doubles 0–1 (r, g, b).
///
/// This is the whole portal boundary of the eyedropper; everything around it (what is picked
/// for, the chip, the colour's use, errors) is in `ui::editor` and tested with a stand-in.
/// Written against `ashpd` 0.11's `desktop::Color::pick()`; like the screenshot calls above it
/// is **not exercised against a live portal** in the development environment (the
/// maintainer's real desktop).
pub async fn pick_colour() -> Result<(f64, f64, f64), CaptureError> {
    ensure_registered().await;
    let request = ashpd::desktop::Color::pick()
        .identifier(None::<WindowIdentifier>)
        .send()
        .await
        .map_err(|err| map_portal_error(&err))?;
    let colour = request.response().map_err(|err| map_portal_error(&err))?;
    Ok((colour.red(), colour.green(), colour.blue()))
}

/// The portal writes its image to a file of its own choosing. Annota keeps captures only
/// where the user asked (CLAUDE.md §7), so that intermediate file is removed once loaded.
fn discard_portal_file(file: &gio::File) {
    let is_regular = file.query_file_type(
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) == gio::FileType::Regular;
    if is_regular && file.is_native() {
        let _ = file.delete(gio::Cancellable::NONE);
    }
}

/// The parent handle's lifetime on a live display, for `live_tests`.
#[cfg(test)]
pub(super) async fn held_until_the_window_closes(window: &gtk::Window) {
    let parent = ParentWindow::new(window).await;
    assert!(parent.is_held());
    assert!(parent.for_request().is_some());
    let surface_at_release = Rc::new(std::cell::Cell::new(false));
    let seen = surface_at_release.clone();
    window.connect_unrealize(move |window| seen.set(window.surface().is_some()));
    window.destroy();
    assert!(!parent.is_held(), "gone with the window, not at the answer");
    assert!(window.surface().is_none());
    assert!(
        surface_at_release.get(),
        "the release must come while the surface still exists"
    );
    drop(parent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_silent() {
        assert_eq!(
            map_portal_error(&ashpd::Error::Response(ResponseError::Cancelled)),
            CaptureError::Cancelled
        );
        assert_eq!(
            map_portal_error(&ashpd::Error::Portal(PortalError::Cancelled(String::new()))),
            CaptureError::Cancelled
        );
    }

    #[test]
    fn response_code_2_is_a_refusal_not_a_denial() {
        // Only the permission store can say "denied" (`permission::after_refusal`).
        assert_eq!(
            map_portal_error(&ashpd::Error::Response(ResponseError::Other)),
            CaptureError::Refused
        );
        assert_eq!(
            map_portal_error(&ashpd::Error::Portal(PortalError::NotAllowed(
                String::from("Screenshot not allowed")
            ))),
            CaptureError::Refused
        );
    }

    #[test]
    fn missing_portal_is_reported_as_such() {
        let missing = || {
            zbus::Error::FDO(Box::new(zbus::fdo::Error::ServiceUnknown(String::from(
                "The name org.freedesktop.portal.Desktop was not provided by any .service files",
            ))))
        };
        assert_eq!(
            map_portal_error(&ashpd::Error::Zbus(missing())),
            CaptureError::PortalUnavailable
        );
        assert_eq!(
            map_portal_error(&ashpd::Error::Portal(PortalError::ZBus(missing()))),
            CaptureError::PortalUnavailable
        );
    }

    #[test]
    fn other_errors_keep_their_message() {
        let err = map_portal_error(&ashpd::Error::NoResponse);
        assert!(matches!(err, CaptureError::Failed(msg) if msg.contains("no response")));
    }

    #[test]
    fn a_plain_copy_keeps_the_handle() {
        let x11 = WindowIdentifier::from_xid(1024);
        assert_eq!(plain_copy(&x11).unwrap().to_string(), "x11:0x400");
        let wayland = WindowIdentifier::X11("wayland:abc".parse().unwrap());
        assert_eq!(plain_copy(&wayland).unwrap().to_string(), "wayland:abc");
    }

    #[test]
    fn window_mode_uses_the_interactive_picker() {
        // Scenario "Window on Wayland": the portal picker is shown.
        assert!(GrabOptions::for_mode(CaptureMode::Window).interactive);
        assert!(!GrabOptions::for_mode(CaptureMode::Region).interactive);
        assert!(!GrabOptions::for_mode(CaptureMode::Screen).interactive);
    }
}
