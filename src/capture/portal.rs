//! Screen access through the xdg-desktop-portal Screenshot interface (CLAUDE.md §2: no other
//! way to read the screen).
//!
//! Written against `ashpd` 0.11's documented API. Not exercised against a live portal in the
//! development environment (it is the maintainer's real desktop); see `manual-checks.md`.

use std::cell::Cell;

use ashpd::desktop::screenshot::Screenshot;
use ashpd::desktop::ResponseError;
use ashpd::zbus;
use ashpd::{PortalError, WindowIdentifier};
use gtk::{gdk, gio, prelude::*};

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
    /// The portal refused screen access for Annota (spec: Portal permission handling).
    #[error("screen access was denied")]
    PermissionDenied,
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
/// A request answered with response code 2 ("other") is how xdg-desktop-portal reports that
/// the permission store denies Annota screenshots, so it maps to [`CaptureError::PermissionDenied`].
pub fn map_portal_error(err: &ashpd::Error) -> CaptureError {
    match err {
        ashpd::Error::Response(ResponseError::Cancelled) => CaptureError::Cancelled,
        ashpd::Error::Response(ResponseError::Other) => CaptureError::PermissionDenied,
        ashpd::Error::Portal(PortalError::NotAllowed(_)) => CaptureError::PermissionDenied,
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
    static HOST_REGISTERED: Cell<bool> = const { Cell::new(false) };
}

/// Registers Annota's app ID with xdg-desktop-portal (≥ 1.19) once per process, so that
/// permissions and GNOME Settings › Apps › Annota refer to Annota rather than "unknown app".
/// Older portals don't have the registry; that's fine and ignored.
pub async fn ensure_registered() {
    if HOST_REGISTERED.with(|done| done.replace(true)) {
        return;
    }
    if let Ok(app_id) = ashpd::AppID::try_from(config::APP_ID) {
        // Older portals (before 1.19) have no registry; nothing to do then.
        let _ = ashpd::register_host_app(app_id).await;
    }
}

/// Asks the Screenshot portal for an image and loads it.
pub async fn grab(options: GrabOptions) -> Result<Frame, CaptureError> {
    ensure_registered().await;
    let request = Screenshot::request()
        .identifier(None::<WindowIdentifier>)
        .interactive(options.interactive)
        .modal(options.modal)
        .send()
        .await
        .map_err(|err| map_portal_error(&err))?;
    let screenshot = request.response().map_err(|err| map_portal_error(&err))?;
    let file = gio::File::for_uri(screenshot.uri().as_str());
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
    fn permission_errors_map_to_denied() {
        // Scenario "Permission denied": the portal returns a permission error.
        assert_eq!(
            map_portal_error(&ashpd::Error::Response(ResponseError::Other)),
            CaptureError::PermissionDenied
        );
        assert_eq!(
            map_portal_error(&ashpd::Error::Portal(PortalError::NotAllowed(
                String::from("Screenshot not allowed")
            ))),
            CaptureError::PermissionDenied
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
    fn window_mode_uses_the_interactive_picker() {
        // Scenario "Window on Wayland": the portal picker is shown.
        assert!(GrabOptions::for_mode(CaptureMode::Window).interactive);
        assert!(!GrabOptions::for_mode(CaptureMode::Region).interactive);
        assert!(!GrabOptions::for_mode(CaptureMode::Screen).interactive);
    }
}
