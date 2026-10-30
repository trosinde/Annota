//! Development preview: one document on the canvas, nothing else (`annota --dev-canvas FILE`,
//! debug builds only). Proves the renderer, zoom and pan against real files until the editor
//! window exists (add-annotation-editor task 4.1 replaces this).

use std::path::Path;

use adw::prelude::*;
use gtk::{gdk, glib};

use annota::document::archive;
use annota::document::bitmap::Bitmap;
use annota::document::Document;

use super::canvas::Canvas;
use crate::capture::texture::frame_from_texture;

const UI_PREVIEW: &str = "/io/github/trosinde/Annota/ui/canvas-preview.ui";

/// Looks up a builder object, or logs and returns (never panics; see `preferences.rs`).
macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: canvas-preview.ui is missing object '{}'", $name);
                return;
            }
        }
    };
}

/// Opens `path` (`.annota` or any image GdkPixbuf/GdkTexture can read) in a preview window.
pub fn open(app: &adw::Application, path: &Path) {
    let doc = match load(path) {
        Ok(doc) => doc,
        Err(err) => {
            eprintln!("annota: cannot open {}: {err}", path.display());
            return;
        }
    };
    let builder = gtk::Builder::from_resource(UI_PREVIEW);
    let window: adw::Window = obj!(builder, "canvas_preview");
    let toolbar: adw::ToolbarView = obj!(builder, "toolbar_view");
    let title: adw::WindowTitle = obj!(builder, "window_title");
    window.set_application(Some(app));

    let name = doc.metadata().title.clone().unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    title.set_title(&name);
    window.set_title(Some(&name));
    let size = doc.canvas().rect;
    let subtitle = move |zoom: f64, fitted: bool| {
        if fitted {
            format!("{} × {}", size.width, size.height)
        } else {
            format!(
                "{} × {} · {} %",
                size.width,
                size.height,
                annota::render::view::percent(zoom)
            )
        }
    };
    title.set_subtitle(&subtitle(1.0, true));

    let canvas = Canvas::new(doc);
    canvas.connect_view_changed(glib::clone!(
        #[weak]
        title,
        move |zoom, fitted| title.set_subtitle(&subtitle(zoom, fitted))
    ));
    toolbar.set_content(Some(canvas.widget()));

    // The controller (owned by the window) keeps the canvas alive as long as the window.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = canvas.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if pressed.key_pressed(key, modifiers) {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    keys.connect_key_released(move |_, key, _, _| canvas.key_released(key));
    window.add_controller(keys);
    window.present();
}

fn load(path: &Path) -> Result<Document, String> {
    let is_annota = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("annota"));
    if is_annota {
        return archive::load(path).map_err(|err| err.to_string());
    }
    let texture = gdk::Texture::from_filename(path).map_err(|err| err.to_string())?;
    let frame = frame_from_texture(&texture).ok_or("empty image")?;
    let bitmap = Bitmap::from_argb32(
        frame.width(),
        frame.height(),
        frame.stride(),
        frame.data().to_vec(),
    )
    .map_err(|err| err.to_string())?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Document::with_title(bitmap, stem))
}
