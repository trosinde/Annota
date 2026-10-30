//! What happens with a finished capture: the `after-capture` preference.
//!
//! "Copy to clipboard" is final. "Open in editor" cannot open an editor yet — that arrives
//! with `add-annotation-editor` — so until then the capture is quick-saved as PNG into the
//! save folder and announced with a toast (see the change's `tasks.md` and spec). Images
//! pasted from the clipboard or opened from a file take the same editor path.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use super::frame::Frame;
use super::texture::texture_from_frame;
use crate::{application, config, preferences};

/// Where an image came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Region,
    Window,
    Screen,
    Clipboard,
    File,
}

/// GSettings enum `io.github.trosinde.Annota.AfterCapture`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterCapture {
    Editor,
    Clipboard,
    Both,
}

impl AfterCapture {
    pub fn from_setting(value: i32) -> Self {
        match value {
            1 => Self::Clipboard,
            2 => Self::Both,
            _ => Self::Editor,
        }
    }
}

/// The concrete steps for one image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Quick-save into the save folder (stands in for "open in editor").
    pub save: bool,
    /// Put the image on the clipboard.
    pub copy: bool,
}

pub fn plan(source: Source, after: AfterCapture) -> Plan {
    match source {
        // Pasted and opened images always become a new capture (spec: Image from clipboard
        // or file); copying them back to the clipboard would be pointless.
        Source::Clipboard | Source::File => Plan {
            save: true,
            copy: false,
        },
        Source::Region | Source::Window | Source::Screen => match after {
            AfterCapture::Editor => Plan {
                save: true,
                copy: false,
            },
            AfterCapture::Clipboard => Plan {
                save: false,
                copy: true,
            },
            AfterCapture::Both => Plan {
                save: true,
                copy: true,
            },
        },
    }
}

/// Makes a file-name stem safe for the save folder.
pub fn sanitize_stem(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| if c == '/' || c.is_control() { '-' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.');
    if trimmed.is_empty() {
        String::from("Annota")
    } else {
        trimmed.to_string()
    }
}

/// `dir/stem.ext`, or `dir/stem (2).ext`, `(3)` … if taken. Never overwrites.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// Toast title for a new image, e.g. "Region captured · 400 × 300".
pub fn title(source: Source, width: i32, height: i32) -> String {
    let template = match source {
        Source::Region => gettext("Region captured · {width} × {height}"),
        Source::Window => gettext("Window captured · {width} × {height}"),
        Source::Screen => gettext("Screen captured · {width} × {height}"),
        Source::Clipboard => gettext("New capture from clipboard"),
        Source::File => gettext("New capture from file"),
    };
    template
        .replace("{width}", &width.to_string())
        .replace("{height}", &height.to_string())
}

/// Saves `texture` as PNG into the configured save folder.
fn quick_save(texture: &gdk::Texture, settings: &gio::Settings) -> Result<PathBuf, String> {
    let folder = preferences::resolved_save_folder(&settings.string("save-folder"));
    std::fs::create_dir_all(&folder).map_err(|err| err.to_string())?;
    let now = glib::DateTime::now_local().map_err(|err| err.to_string())?;
    let stem = sanitize_stem(&preferences::expand_filename_pattern(
        &settings.string("filename-pattern"),
        &now,
    ));
    let path = unique_path(&folder, &stem, "png");
    texture.save_to_png(&path).map_err(|err| err.to_string())?;
    Ok(path)
}

/// Copies `texture` to the clipboard. On Wayland the clipboard empties when its owner exits,
/// so the application is held until another program takes the clipboard over.
pub fn copy_to_clipboard(app: &adw::Application, texture: &gdk::Texture) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    clipboard.set_texture(texture);

    let guard = Rc::new(RefCell::new(Some(app.hold())));
    let handler: Rc<RefCell<Option<glib::SignalHandlerId>>> = Rc::new(RefCell::new(None));
    let id = clipboard.connect_changed(glib::clone!(
        #[strong]
        guard,
        #[strong]
        handler,
        move |clipboard| {
            if !clipboard.is_local() {
                guard.borrow_mut().take();
                if let Some(id) = handler.borrow_mut().take() {
                    clipboard.disconnect(id);
                }
            }
        }
    ));
    handler.replace(Some(id));
}

/// Hands a finished image to its destination and tells the user what happened.
///
/// `window_was_open` says whether an Annota window was on screen when the capture started;
/// "Copy to clipboard" then still confirms with a toast, otherwise no window opens.
pub fn deliver(app: &adw::Application, frame: Frame, source: Source, window_was_open: bool) {
    let settings = gio::Settings::new(config::APP_ID);
    let plan = plan(
        source,
        AfterCapture::from_setting(settings.enum_("after-capture")),
    );
    let texture = texture_from_frame(&frame);
    let heading = title(source, frame.width(), frame.height());

    if plan.copy {
        copy_to_clipboard(app, &texture);
    }

    if !plan.save {
        if window_was_open {
            let window = application::present_window(app);
            let text = format!("{heading} · {}", gettext("Copied to clipboard"));
            application::show_toast(&window, &adw::Toast::builder().title(text).build());
        }
        return;
    }

    let window = application::present_window(app);
    match quick_save(&texture, &settings) {
        Ok(path) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let saved = gettext("Saved as {name}").replace("{name}", &name);
            let toast = adw::Toast::builder()
                .title(format!("{heading} · {saved}"))
                .build();
            toast.set_use_markup(false);
            match source {
                Source::Clipboard | Source::File => {
                    toast.set_button_label(Some(&gettext("Undo")));
                    toast.connect_button_clicked(move |_| {
                        let file = gio::File::for_path(&path);
                        file.trash_async(
                            glib::Priority::DEFAULT,
                            gio::Cancellable::NONE,
                            |result| {
                                if let Err(err) = result {
                                    eprintln!("annota: could not undo the new capture: {err}");
                                }
                            },
                        );
                    });
                }
                Source::Region | Source::Window | Source::Screen if !plan.copy => {
                    toast.set_button_label(Some(&gettext("Copy")));
                    toast.connect_button_clicked(glib::clone!(
                        #[weak]
                        app,
                        #[strong]
                        texture,
                        move |_| copy_to_clipboard(&app, &texture)
                    ));
                }
                _ => {}
            }
            application::show_toast(&window, &toast);
        }
        Err(err) => {
            let text = gettext("Could not save the capture: {error}").replace("{error}", &err);
            let toast = adw::Toast::builder().title(text).build();
            toast.set_use_markup(false);
            application::show_toast(&window, &toast);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn after_capture_plans() {
        let capture = Source::Region;
        assert_eq!(
            plan(capture, AfterCapture::Editor),
            Plan {
                save: true,
                copy: false
            }
        );
        assert_eq!(
            plan(capture, AfterCapture::Clipboard),
            Plan {
                save: false,
                copy: true
            }
        );
        assert_eq!(
            plan(capture, AfterCapture::Both),
            Plan {
                save: true,
                copy: true
            }
        );
    }

    #[test]
    fn imports_always_become_a_new_capture() {
        for after in [
            AfterCapture::Editor,
            AfterCapture::Clipboard,
            AfterCapture::Both,
        ] {
            assert_eq!(
                plan(Source::Clipboard, after),
                Plan {
                    save: true,
                    copy: false
                }
            );
            assert!(plan(Source::File, after).save);
        }
    }

    #[test]
    fn setting_values_map_to_enum() {
        assert_eq!(AfterCapture::from_setting(0), AfterCapture::Editor);
        assert_eq!(AfterCapture::from_setting(1), AfterCapture::Clipboard);
        assert_eq!(AfterCapture::from_setting(2), AfterCapture::Both);
        assert_eq!(AfterCapture::from_setting(9), AfterCapture::Editor);
    }

    #[test]
    fn stems_are_sanitised() {
        assert_eq!(sanitize_stem("Annota 2026-10-30"), "Annota 2026-10-30");
        assert_eq!(sanitize_stem("a/b"), "a-b");
        assert_eq!(sanitize_stem("  "), "Annota");
        assert_eq!(sanitize_stem("..hidden"), "hidden");
    }

    #[test]
    fn unique_path_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("annota-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = unique_path(&dir, "Shot", "png");
        assert_eq!(first, dir.join("Shot.png"));
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(&dir, "Shot", "png");
        assert_eq!(second, dir.join("Shot (2).png"));
        std::fs::write(&second, b"x").unwrap();
        assert_eq!(unique_path(&dir, "Shot", "png"), dir.join("Shot (3).png"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn titles_carry_the_size() {
        assert_eq!(
            title(Source::Region, 400, 300),
            "Region captured · 400 × 300"
        );
        assert_eq!(title(Source::Clipboard, 1, 1), "New capture from clipboard");
    }
}
