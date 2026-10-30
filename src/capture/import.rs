//! New captures from the clipboard (`Shift+Ctrl+V`) or from PNG/JPG/WebP files opened with
//! `Ctrl+O` or dropped onto Annota (spec: Image from clipboard or file; mockups 13, 14). They
//! open in the editor, like `.annota` documents opened or dropped the same way.

use std::path::Path;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use super::delivery::{self, Source};
use super::texture::frame_from_texture;
use crate::application;

/// File types Annota opens as a new capture.
pub const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
pub const IMAGE_MIME_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

fn toast(app: &adw::Application, text: &str) {
    let window = application::present_window(app);
    let toast = adw::Toast::builder().title(text).build();
    toast.set_use_markup(false);
    application::show_toast(&window, &toast);
}

/// `Shift+Ctrl+V`: the clipboard image becomes a new capture.
pub fn paste(app: &adw::Application) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let app = app.clone();
    glib::spawn_future_local(async move {
        let texture = clipboard.read_texture_future().await.ok().flatten();
        match texture.as_ref().and_then(frame_from_texture) {
            Some(frame) => delivery::deliver(&app, frame, Source::Clipboard, true),
            None => toast(&app, &gettext("The clipboard does not contain an image")),
        }
    });
}

pub fn is_annota_document(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("annota"))
}

/// Opens one file: an image as a new capture, an `.annota` document as itself, both in the
/// editor.
pub fn open_file(app: &adw::Application, file: &gio::File) {
    let path = file.path();
    let supported = path
        .as_deref()
        .is_some_and(|p| is_supported_image(p) || is_annota_document(p));
    let Some(path) = path.filter(|_| supported) else {
        toast(
            app,
            &gettext("Only PNG, JPG and WebP images and Annota documents can be opened"),
        );
        return;
    };
    if crate::ui::editor::open_file(app, &path, true) {
        return;
    }
    if is_annota_document(&path) {
        toast(app, &gettext("The editor could not be opened"));
        return;
    }
    // Without the editor an image still becomes a capture (quick-saved, see `delivery`).
    match gdk::Texture::from_file(file) {
        Ok(texture) => match frame_from_texture(&texture) {
            Some(frame) => delivery::deliver(app, frame, Source::File, true),
            None => toast(app, &gettext("This image is empty")),
        },
        Err(err) => {
            eprintln!("annota: cannot open image: {err}");
            toast(app, &gettext("This image could not be opened"));
        }
    }
}

/// `Ctrl+O`: pick an image with the file chooser portal.
pub fn choose_file(app: &adw::Application) {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&gettext("Images and Annota documents")));
    for mime in IMAGE_MIME_TYPES {
        filter.add_mime_type(mime);
    }
    filter.add_suffix("annota");
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title(gettext("Open"))
        .modal(true)
        .filters(&filters)
        .default_filter(&filter)
        .build();
    let parent = app.active_window();
    let app = app.clone();
    glib::spawn_future_local(async move {
        if let Ok(file) = dialog.open_future(parent.as_ref()).await {
            open_file(&app, &file);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_png_jpg_webp_only() {
        assert!(is_supported_image(Path::new("/tmp/booking-chat.png")));
        assert!(is_supported_image(Path::new("shot.JPG")));
        assert!(is_supported_image(Path::new("shot.jpeg")));
        assert!(is_supported_image(Path::new("shot.webp")));
        assert!(!is_supported_image(Path::new("shot.annota")));
        assert!(!is_supported_image(Path::new("shot.gif")));
        assert!(!is_supported_image(Path::new("shot")));
        assert!(is_annota_document(Path::new("/tmp/Checkout flow.ANNOTA")));
        assert!(!is_annota_document(Path::new("shot.png")));
    }
}
