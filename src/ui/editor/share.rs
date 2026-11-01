//! Copy, drag-out and Share (add-export-share tasks 2.3 and 2.4; spec export-share
//! "Clipboard", "Drag and drop", "Share"; mockups 01, 07, 08, 09, 14): the pieces the editor
//! puts together, as free of the window as they can be.
//!
//! * **Clipboard**: the flattened document as a `GdkTexture` on the system clipboard
//!   ([`texture`], `capture::delivery::copy_to_clipboard`); GTK offers a texture as `image/png`
//!   (and other image types) to whoever pastes. No file is written.
//! * **Object clipboard** (task 2.4a): with a selection, Copy adds the selected objects to that
//!   image, as `annota::document::clipboard`'s self-contained payload under its own type
//!   ([`selection_content`]); `Ctrl+V` reads that type first ([`paste_kind`], [`read_payload`])
//!   and otherwise opens a clipboard image as a new document, as `Shift+Ctrl+V` does. Any other
//!   app pastes the image.
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
use gettextrs::{gettext, ngettext};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use annota::document::archive::ArchiveError;
use annota::document::bitmap::Bitmap;
use annota::document::clipboard;
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

/// What `Ctrl+V` pastes, by the types the clipboard offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteKind {
    /// Annota's copied objects: into the current document.
    Objects,
    /// Anything else: a clipboard image opens as a new document (`app.paste-image`, which
    /// also says so when there is no image).
    Image,
}

/// Annota's object type wins over anything else the clipboard offers.
pub fn paste_kind<S: AsRef<str>>(mime_types: &[S]) -> PasteKind {
    if mime_types
        .iter()
        .any(|m| m.as_ref() == clipboard::MIME_TYPE)
    {
        PasteKind::Objects
    } else {
        PasteKind::Image
    }
}

/// Copied objects under Annota's object type, and only that (one half of
/// [`selection_content`]).
pub fn objects_content(payload: Vec<u8>) -> gdk::ContentProvider {
    gdk::ContentProvider::for_bytes(clipboard::MIME_TYPE, &glib::Bytes::from_owned(payload))
}

/// What Copy with a selection puts on the clipboard (maintainer decision, 2026-10-31): the
/// selected objects under Annota's object type — what `Ctrl+V` in Annota pastes — together
/// with the whole flattened image as a texture, which GTK offers as `image/png` (and other
/// image types) to every other app, exactly as Copy without a selection does. One union, like
/// [`drag_content`]'s.
pub fn selection_content(payload: Vec<u8>, image: &gdk::Texture) -> gdk::ContentProvider {
    gdk::ContentProvider::new_union(&[
        objects_content(payload),
        gdk::ContentProvider::for_value(&image.to_value()),
    ])
}

/// A payload larger than this is not read (the clipboard can hold anything another program
/// puts there under Annota's type).
pub const MAX_PAYLOAD: usize = 1 << 30;
const READ_CHUNK: usize = 1 << 16;

/// Reads Annota's copied objects from `clipboard`, at most [`MAX_PAYLOAD`] bytes.
pub async fn read_payload(clipboard: &gdk::Clipboard) -> Result<Vec<u8>, glib::Error> {
    let (stream, _) = clipboard
        .read_future(&[clipboard::MIME_TYPE], glib::Priority::DEFAULT)
        .await?;
    let mut payload = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(READ_CHUNK, glib::Priority::DEFAULT)
            .await?;
        if chunk.is_empty() {
            break;
        }
        if payload.len() + chunk.len() > MAX_PAYLOAD {
            return Err(glib::Error::new(
                gio::IOErrorEnum::MessageTooLarge,
                "the copied objects are too large",
            ));
        }
        payload.extend_from_slice(&chunk);
    }
    Ok(payload)
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

/// After Copy with a selection (not drawn; worded like mockup 07's "Image copied to
/// clipboard"): both are there — the image for other apps, the objects for Annota's `Ctrl+V`
/// ([`selection_content`]). `n` counts the selected objects, a group as one.
pub fn objects_copied_toast(n: usize) -> String {
    ngettext(
        "Image and object copied to clipboard",
        "Image and {n} objects copied to clipboard",
        u32::try_from(n).unwrap_or(u32::MAX),
    )
    .replace("{n}", &n.to_string())
}

/// The copied objects could not be pasted (not drawn): from an Annota with a newer major
/// format, or damaged.
pub fn paste_failed_toast(err: &ArchiveError) -> String {
    match err {
        ArchiveError::NewerFormat { .. } => {
            gettext("These objects were copied from a newer Annota and cannot be pasted here")
        }
        _ => gettext("The copied objects could not be pasted"),
    }
}

/// The selected objects could not be put on the clipboard (an embedded image that cannot be
/// encoded, out of memory).
pub fn objects_copy_failed_toast() -> String {
    gettext("The objects could not be copied")
}

/// The image could not be drawn (too large for the renderer, out of memory).
pub fn copy_failed_toast() -> String {
    gettext("The image could not be copied")
}

/// Copy on a new capture's toast, after that capture was closed (not drawn).
pub fn capture_closed_toast() -> String {
    gettext("The capture was closed, so nothing was copied")
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
    fn annota_objects_paste_first_anything_else_opens_as_an_image() {
        // Scenario "Plain image still opens as a new document": only Annota's own type pastes
        // objects; an image, text or nothing falls back to the new-document paste.
        assert_eq!(clipboard::MIME_TYPE, "application/x-annota-objects");
        assert_eq!(
            paste_kind(&["image/png", clipboard::MIME_TYPE]),
            PasteKind::Objects
        );
        assert_eq!(paste_kind(&[clipboard::MIME_TYPE]), PasteKind::Objects);
        assert_eq!(
            paste_kind(&["image/png", "image/jpeg", "text/uri-list"]),
            PasteKind::Image
        );
        assert_eq!(paste_kind(&["text/plain;charset=utf-8"]), PasteKind::Image);
        assert_eq!(paste_kind::<&str>(&[]), PasteKind::Image);
        // A look-alike is not Annota's.
        assert_eq!(
            paste_kind(&["application/x-annota-objects-v2"]),
            PasteKind::Image
        );
    }

    #[test]
    fn object_copy_and_paste_texts() {
        // Copy with a selection gives the image too: the toast says both.
        assert_eq!(
            objects_copied_toast(1),
            "Image and object copied to clipboard"
        );
        assert_eq!(
            objects_copied_toast(3),
            "Image and 3 objects copied to clipboard"
        );
        assert_ne!(objects_copied_toast(1), copied_toast());
        let newer = ArchiveError::NewerFormat {
            found: annota::document::archive::FormatVersion { major: 2, minor: 0 },
            requires: Some("2.1".into()),
        };
        assert!(paste_failed_toast(&newer).contains("newer Annota"));
        assert_eq!(
            paste_failed_toast(&ArchiveError::Invalid("x".into())),
            "The copied objects could not be pasted"
        );
    }

    #[test]
    fn copy_with_a_selection_offers_the_objects_and_the_whole_image() {
        // Maintainer decision 2026-10-31: Annota's type for Ctrl+V in Annota, and the flattened
        // image (a texture: GTK serialises it as `image/png` for other apps, exactly as for
        // Copy without a selection) — both in one clipboard content. GTK's texture → PNG
        // serialiser needs a display; that half is checked by
        // `object_copy_and_paste_on_a_live_display`.
        let doc =
            Document::new(Bitmap::filled(40, 30, annota::document::color::palette::AZURE).unwrap());
        let image = texture(&flatten(&doc).unwrap()).unwrap();
        let provider = selection_content(b"the objects".to_vec(), &image);
        let formats = provider.formats();
        assert!(formats.contain_mime_type(clipboard::MIME_TYPE));
        assert!(formats.contains_type(gdk::Texture::static_type()));
        // Annota's paste still finds its own type first.
        assert_eq!(paste_kind(&formats.mime_types()), PasteKind::Objects);
        // The image half is the very texture, the whole document's.
        let offered = provider
            .value(gdk::Texture::static_type())
            .unwrap()
            .get::<gdk::Texture>()
            .unwrap();
        assert_eq!(offered, image);
        assert_eq!((offered.width(), offered.height()), (40, 30));
        // The object half is the payload, byte for byte.
        let sink = gio::MemoryOutputStream::new_resizable();
        glib::MainContext::new()
            .block_on(provider.write_mime_type_future(
                clipboard::MIME_TYPE,
                &sink,
                glib::Priority::DEFAULT,
            ))
            .unwrap();
        sink.close(gio::Cancellable::NONE).unwrap();
        assert_eq!(&sink.steal_as_bytes()[..], b"the objects");
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
