//! Copy, drag-out and Share (add-export-share tasks 2.3 and 2.4; spec export-share
//! "Clipboard", "Drag and drop", "Share"; mockups 01, 07, 08, 09, 14): the pieces the editor
//! puts together, as free of the window as they can be.
//!
//! * **Clipboard**: the flattened document as a `GdkTexture` on the system clipboard
//!   ([`texture`], `capture::delivery::copy_to_clipboard`); GTK offers a texture as `image/png`
//!   (and other image types) to whoever pastes. No file is written.
//! * **Drag-out**: a temporary PNG named after the document (`annota::export::handoff`), offered
//!   as `text/uri-list` (a `GdkFileList`) together with the same bytes as `image/png`
//!   ([`drag_content`]); the drag image is mockup 09's 160 × 100 thumbnail with a "PNG" badge
//!   ([`ghost`]).
//! * **Share → Email… / Open With…**: the same temporary PNG, handed to xdg-desktop-portal's
//!   `Email` (as an attachment) or `OpenURI` (`OpenFile` with `ask`, the system's app chooser)
//!   through `ashpd` ([`portal_sharer`]). That is the whole portal boundary of this module; the
//!   editor reaches it through a [`Sharer`], so tests run everything around it with a stand-in.
//!   Like the other portal calls it is **not exercised against a live portal** in development.
//! * **Share → Copy to Folder…** needs no portal of its own: GTK's file chooser (GTK decides
//!   whether that is the FileChooser portal), then a PNG export to the chosen path.

use std::future::Future;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;

use ashpd::WindowIdentifier;
use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use annota::document::bitmap::Bitmap;
use annota::document::Document;
use annota::export::handoff::Handoff;
use annota::export::ExportError;

use super::{saving, text, tray};
use crate::capture::portal::{self, CaptureError};

/// "Image copied to clipboard" stays this long (redline "Toasts": 3 s).
pub const COPIED_TOAST_SECONDS: u32 = 3;
/// The drag image (redline "Drag"): a 160 × 100 thumbnail.
pub const GHOST_WIDTH: i32 = 160;
pub const GHOST_HEIGHT: i32 = 100;
/// The ghost's thumbnail is rendered at twice its size, sharp on HiDPI screens.
const GHOST_SCALE: i32 = 2;

/// Where Email… and Open With… send the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The `Email` portal: the user's mail app, a new message with the PNG attached.
    Email,
    /// The `OpenURI` portal's `OpenFile`, asking which app (the system's app chooser).
    OpenWith,
}

/// Why a portal did not take the file.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShareError {
    /// The user closed the portal's own UI (the app chooser, for instance).
    #[error("cancelled")]
    Cancelled,
    /// No xdg-desktop-portal (or no implementation of this portal) is running.
    #[error("no portal for this is available")]
    Unavailable,
    /// Any other failure, as a diagnostic.
    #[error("{0}")]
    Failed(String),
}

impl ShareError {
    /// Sorts an `ashpd` error the way the screenshot portal's errors are sorted
    /// (`capture::portal::map_portal_error`: cancelled, missing portal, the rest).
    fn from_portal(err: &ashpd::Error) -> Self {
        match portal::map_portal_error(err) {
            CaptureError::Cancelled => ShareError::Cancelled,
            CaptureError::PortalUnavailable => ShareError::Unavailable,
            _ => ShareError::Failed(err.to_string()),
        }
    }
}

/// Hands a file to another application through the portals; [`portal_sharer`] in Annota, a
/// stand-in in tests.
pub type Sharer =
    Rc<dyn Fn(Target, PathBuf) -> Pin<Box<dyn Future<Output = Result<(), ShareError>>>>>;

/// The real portal calls.
pub fn portal_sharer() -> Sharer {
    Rc::new(|target, path| Box::pin(share_through_portal(target, path)))
}

/// Opens `path` and hands it to the portal of `target`, waiting for the portal's answer (for
/// Email: the mail app was asked to compose; for Open With: an app was chosen and started).
///
/// Written against `ashpd` 0.11's documented `desktop::email::EmailRequest` and
/// `desktop::open_uri::OpenFileRequest`; not exercised against a live portal in development (the
/// maintainer's real desktop) — see `manual-checks.md`.
async fn share_through_portal(target: Target, path: PathBuf) -> Result<(), ShareError> {
    portal::ensure_registered().await;
    let file = std::fs::File::open(&path).map_err(|err| ShareError::Failed(err.to_string()))?;
    let request = match target {
        Target::Email => {
            ashpd::desktop::email::EmailRequest::default()
                .identifier(None::<WindowIdentifier>)
                .attach(OwnedFd::from(file))
                .send()
                .await
        }
        Target::OpenWith => {
            ashpd::desktop::open_uri::OpenFileRequest::default()
                .identifier(None::<WindowIdentifier>)
                .ask(true)
                .send_file(&file)
                .await
        }
    }
    .map_err(|err| ShareError::from_portal(&err))?;
    request
        .response()
        .map_err(|err| ShareError::from_portal(&err))
}

/// Whether a failed portal call is told in a toast: not when the user cancelled (closed the
/// app chooser, for instance).
pub fn tells(err: &ShareError) -> bool {
    !matches!(err, ShareError::Cancelled)
}

/// The folder of the temporary hand-off files: `$XDG_RUNTIME_DIR/annota/handoff` — private,
/// in memory, emptied at logout (GLib falls back to the cache folder without a runtime dir).
pub fn handoff_root() -> PathBuf {
    glib::user_runtime_dir().join("annota").join("handoff")
}

/// The file name stem of a hand-off: the document's name as the header shows it (redline
/// "Drag": "File name = document title + .png").
pub fn stem(name: &str) -> String {
    saving::file_stem(name).unwrap_or_else(text::untitled)
}

/// The document's flattened image at 100 % — the pixels of a PNG export.
pub fn flatten(doc: &Document) -> Result<Bitmap, ExportError> {
    Ok(annota::render::flatten(doc, 1.0)?)
}

/// `bitmap` at full size as a texture (for the clipboard).
pub fn texture(bitmap: &Bitmap) -> Option<gdk::Texture> {
    tray::scaled_pixels(bitmap, bitmap.width(), bitmap.height()).map(tray::texture)
}

/// The drag image's thumbnail of `bitmap`.
pub fn ghost_texture(bitmap: &Bitmap) -> Option<gdk::Texture> {
    tray::scaled_pixels(
        bitmap,
        GHOST_WIDTH * GHOST_SCALE,
        GHOST_HEIGHT * GHOST_SCALE,
    )
    .map(tray::texture)
}

/// What a drag of the title area carries (redline "Drag": `text/uri-list` (temporary PNG) +
/// `image/png`): the file as a `GdkFileList` — serialised as `text/uri-list`, which Files and
/// most apps take — and the PNG's bytes for apps that only accept image data.
pub fn drag_content(handoff: &Handoff) -> gdk::ContentProvider {
    let files = gdk::FileList::from_array(&[gio::File::for_path(&handoff.path)]);
    let png = glib::Bytes::from(handoff.png.as_slice());
    gdk::ContentProvider::new_union(&[
        gdk::ContentProvider::for_value(&files.to_value()),
        gdk::ContentProvider::for_bytes("image/png", &png),
    ])
}

/// Mockup 09's drag image: the thumbnail (160 × 100, radius 6) with a "PNG" badge at its
/// bottom-right corner.
pub fn ghost(texture: Option<&gdk::Texture>) -> gtk::Widget {
    let picture = gtk::Picture::builder()
        .can_shrink(true)
        .content_fit(gtk::ContentFit::Cover)
        .width_request(GHOST_WIDTH)
        .height_request(GHOST_HEIGHT)
        .overflow(gtk::Overflow::Hidden)
        .css_classes(["annota-drag-ghost"])
        .build();
    picture.set_paintable(texture);
    let badge = gtk::Label::builder()
        .label("PNG")
        .halign(gtk::Align::End)
        .valign(gtk::Align::End)
        .css_classes(["annota-drag-badge"])
        .build();
    // The texture is rendered at twice the size (sharp on HiDPI): without a cap the picture
    // would ask for that size. Capped in both directions, it is 160 × 100 for any aspect ratio
    // (cropped to fill, like the tray's drag image).
    let width = adw::Clamp::builder()
        .orientation(gtk::Orientation::Horizontal)
        .maximum_size(GHOST_WIDTH)
        .tightening_threshold(GHOST_WIDTH)
        .child(&picture)
        .build();
    let height = adw::Clamp::builder()
        .orientation(gtk::Orientation::Vertical)
        .maximum_size(GHOST_HEIGHT)
        .tightening_threshold(GHOST_HEIGHT)
        .child(&width)
        .build();
    // Mockup 09: the drag image at 92 %.
    let overlay = gtk::Overlay::builder().child(&height).opacity(0.92).build();
    overlay.add_overlay(&badge);
    overlay.upcast()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Mockup 07.
pub fn copied_toast() -> String {
    gettext("Image copied to clipboard")
}

/// Mockup 14: “Opening your mail app with “Checkout flow.png” attached…”.
pub fn email_toast(path: &Path) -> String {
    gettext("Opening your mail app with “{name}” attached…").replace("{name}", &file_name(path))
}

/// After Copy to Folder… wrote the PNG (not drawn; worded like mockup 06's “Saved as …”).
pub fn copied_to_folder_toast(path: &Path) -> String {
    gettext("Copied as “{name}”").replace("{name}", &file_name(path))
}

/// Mockup 08: the title area's tooltip.
pub fn drag_tooltip() -> String {
    gettext("Drag image to another app")
}

/// After the redaction warning stopped a drag (not drawn: the drag that hit the warning cannot
/// go on, the next one can).
pub fn drag_ready_toast() -> String {
    gettext("You can drag the image out now")
}

/// The image could not be drawn (too large for the renderer, out of memory).
pub fn copy_failed_toast() -> String {
    gettext("The image could not be copied")
}

/// The temporary PNG for a drag, Email… or Open With… could not be written.
pub fn prepare_failed_toast() -> String {
    gettext("The image could not be prepared for sharing")
}

/// A portal call failed (not cancelled).
pub fn failed_toast(target: Target) -> String {
    match target {
        Target::Email => gettext("Could not open your mail app"),
        Target::OpenWith => gettext("Could not open the image in another app"),
    }
}

/// The name Copy to Folder… suggests: `<document name>.png`.
pub fn copy_to_folder_name(name: &str) -> String {
    format!("{}.png", stem(name))
}

/// A path chosen in Copy to Folder…, with `.png` appended unless it has it already.
pub fn with_png_extension(path: &Path) -> PathBuf {
    let has = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"));
    if has {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".png");
        PathBuf::from(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toasts_follow_the_mockups() {
        // Mockup 07, 14 and 08.
        assert_eq!(copied_toast(), "Image copied to clipboard");
        assert_eq!(
            email_toast(Path::new(
                "/run/user/1000/annota/handoff/x/Checkout flow.png"
            )),
            "Opening your mail app with “Checkout flow.png” attached…"
        );
        assert_eq!(drag_tooltip(), "Drag image to another app");
        assert_eq!(
            copied_to_folder_toast(Path::new("/home/mara/Tickets/Checkout flow.png")),
            "Copied as “Checkout flow.png”"
        );
        assert_ne!(failed_toast(Target::Email), failed_toast(Target::OpenWith));
    }

    #[test]
    fn handoffs_are_named_after_the_document() {
        // Redline "Drag": file name = document title + .png.
        assert_eq!(stem("Checkout flow"), "Checkout flow");
        assert_eq!(stem("a/b"), "a-b");
        assert_eq!(stem("  "), text::untitled());
        assert_eq!(copy_to_folder_name("Checkout flow"), "Checkout flow.png");
        assert_eq!(
            with_png_extension(Path::new("/t/Shot")),
            Path::new("/t/Shot.png")
        );
        assert_eq!(
            with_png_extension(Path::new("/t/Shot.PNG")),
            Path::new("/t/Shot.PNG")
        );
        assert_eq!(
            with_png_extension(Path::new("/t/Shot.jpg")),
            Path::new("/t/Shot.jpg.png")
        );
    }

    #[test]
    fn a_cancelled_portal_is_not_an_error_to_tell() {
        use ashpd::desktop::ResponseError;
        use ashpd::PortalError;
        let sorted = |err: ashpd::Error| ShareError::from_portal(&err);
        assert_eq!(
            sorted(ashpd::Error::Response(ResponseError::Cancelled)),
            ShareError::Cancelled
        );
        assert_eq!(
            sorted(ashpd::Error::Portal(PortalError::Cancelled(String::new()))),
            ShareError::Cancelled
        );
        let missing = ashpd::zbus::Error::FDO(Box::new(ashpd::zbus::fdo::Error::ServiceUnknown(
            String::from("org.freedesktop.portal.Desktop"),
        )));
        assert_eq!(sorted(ashpd::Error::Zbus(missing)), ShareError::Unavailable);
        assert!(matches!(
            sorted(ashpd::Error::Response(ResponseError::Other)),
            ShareError::Failed(_)
        ));
        assert!(!tells(&ShareError::Cancelled));
        assert!(tells(&ShareError::Unavailable));
        assert!(tells(&ShareError::Failed("x".into())));
    }

    #[test]
    fn handoffs_go_to_the_private_runtime_folder() {
        let root = handoff_root();
        assert!(root.ends_with("annota/handoff"), "{}", root.display());
        assert!(root.starts_with(glib::user_runtime_dir()));
    }
}
