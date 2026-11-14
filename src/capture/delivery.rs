//! What happens with a finished capture: the `after-capture` preference.
//!
//! "Copy to clipboard" is final: no window opens. "Open in editor" (and "Both") open the
//! capture as a new document in the editor window (add-annotation-editor task 4.5; mockups 12
//! and 13 of add-screen-capture), announced with a toast. Images pasted from the clipboard take
//! the same editor path; opened and dropped files go to the editor through `import`. Only if
//! the editor window cannot be built is the capture quick-saved as PNG into the save folder
//! instead (the stand-in this change used before the editor existed), so it is never lost.
//!
//! On Wayland a copy only lands when Annota may own the clipboard (see [`copy_lands`]), so there
//! the copy waits for an Annota window to have the focus, and a toast says "Copied to clipboard"
//! only when the copy could land; otherwise it offers Copy, whose click gives Annota what the
//! compositor asks for.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

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
    /// Open it as a new document in the editor.
    pub open: bool,
    /// Put the image on the clipboard.
    pub copy: bool,
}

pub fn plan(source: Source, after: AfterCapture) -> Plan {
    match source {
        // Pasted and opened images always become a new capture (spec: Image from clipboard
        // or file); copying them back to the clipboard would be pointless.
        Source::Clipboard | Source::File => Plan {
            open: true,
            copy: false,
        },
        Source::Region | Source::Window | Source::Screen => match after {
            AfterCapture::Editor => Plan {
                open: true,
                copy: false,
            },
            AfterCapture::Clipboard => Plan {
                open: false,
                copy: true,
            },
            AfterCapture::Both => Plan {
                open: true,
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

/// The new document's name: "Pasted Image" for the clipboard (mockup 13), the file-name
/// pattern for captures (mockup 12: "Capture 2026-10-30 14-22-08").
pub fn document_name(source: Source, pattern_name: &str) -> String {
    match source {
        Source::Clipboard => gettext("Pasted Image"),
        _ => pattern_name.to_string(),
    }
}

/// What the toast of a new document in the editor offers (mockups 12, 13): pasted and opened
/// images can be dropped again with Undo; a capture that is not on the clipboard can still be
/// copied, as the document is when the button is pressed (`Editor::copy_announced`), so edits
/// made meanwhile are in the image and the redaction warning applies; one that was `copied` is
/// on the clipboard already. A capture is not copied when the preference says "Open in Editor",
/// or when "Both" asked for a copy that could not land ([`copy_lands`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastOffer {
    Undo,
    Copy,
    Nothing,
}

pub fn toast_offer(source: Source, copied: bool) -> ToastOffer {
    match source {
        Source::Clipboard | Source::File => ToastOffer::Undo,
        _ if copied => ToastOffer::Nothing,
        _ => ToastOffer::Copy,
    }
}

/// The toast title of a delivered capture: "Copied to clipboard" is added only when the image
/// was put on the clipboard in a way that lands ([`copy_lands`]), never because it was asked for.
pub fn announced_title(heading: &str, copied: bool) -> String {
    if copied {
        format!("{heading} · {}", gettext("Copied to clipboard"))
    } else {
        heading.to_string()
    }
}

/// Whether a copy made now reaches the clipboard.
///
/// On X11 it always does. On Wayland the compositor hands the clipboard (`set_selection`) only
/// to the client whose surface has the keyboard focus, and only with the serial of an input event
/// of that client; GTK passes the serial of the last key or button press Annota received, or 0
/// if there was none. Measured on GNOME 50.1 (mutter 50.1, GTK 4.22) on 2026-11-14: a focused
/// window copying with serial 0 is ignored, as is any copy while no Annota surface has the focus;
/// a focused window copying with the serial of an earlier key press lands, even one received by
/// another, since destroyed, Annota window, and even after input to other programs since then.
/// The keyboard focus alone and a pointer entering the window give no serial. So a capture
/// started by `annota --capture` or a global shortcut, without any key or click in Annota (the
/// window chooser and the screenshot UI are GNOME's own), cannot be copied by itself at all.
pub fn copy_lands(wayland: bool, focused: bool, input_seen: bool) -> bool {
    !wayland || (focused && input_seen)
}

/// Whether Annota runs on Wayland, where [`copy_lands`] needs focus and an input serial.
pub fn on_wayland() -> bool {
    gdk::Display::default().is_some_and(|display| display.type_().name() == "GdkWaylandDisplay")
}

thread_local! {
    /// Whether any Annota window has received a key or button press (see [`watch_input`]).
    static INPUT_SEEN: Cell<bool> = const { Cell::new(false) };
}

/// Whether `kind` is an event whose serial GTK hands to the compositor with a copy.
pub fn gives_serial(kind: gdk::EventType) -> bool {
    matches!(
        kind,
        gdk::EventType::ButtonPress | gdk::EventType::KeyPress | gdk::EventType::TouchBegin
    )
}

/// Notes key and button presses in `window`, for [`copy_lands`]. Every Annota window gets this
/// (`application::run`); it only looks and never stops an event.
pub fn watch_input(window: &gtk::Window) {
    let events = gtk::EventControllerLegacy::new();
    events.set_propagation_phase(gtk::PropagationPhase::Capture);
    events.connect_event(|_, event| {
        if gives_serial(event.event_type()) {
            INPUT_SEEN.with(|seen| seen.set(true));
        }
        glib::Propagation::Proceed
    });
    window.add_controller(events);
}

/// How long a window presented for a copy may take to get the focus (measured: about 0.5 s).
const FOCUS_WAIT: Duration = Duration::from_secs(2);
const FOCUS_POLL: Duration = Duration::from_millis(50);

/// Copies `texture` once it can land: on Wayland after `window` has the focus (waiting at most
/// [`FOCUS_WAIT`]). Returns whether it was copied; if it cannot land, nothing is copied.
async fn copy_when_it_lands(
    app: &adw::Application,
    texture: &gdk::Texture,
    window: Option<&gtk::Window>,
) -> bool {
    let wayland = on_wayland();
    if wayland {
        if let Some(window) = window {
            let start = Instant::now();
            while !window.is_active() && start.elapsed() < FOCUS_WAIT {
                glib::timeout_future(FOCUS_POLL).await;
            }
        }
    }
    let focused = window.is_some_and(|w| w.is_active());
    let input_seen = INPUT_SEEN.with(Cell::get);
    if !copy_lands(wayland, focused, input_seen) {
        eprintln!(
            "annota: the capture was not copied (focused: {focused}, input seen: {input_seen})"
        );
        return false;
    }
    copy_to_clipboard(app, texture);
    true
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
    hold_until_replaced(app, &clipboard);
}

/// Puts `content` on the clipboard (Annota's copied objects, add-export-share task 2.4a), held
/// like [`copy_to_clipboard`]'s image.
pub fn copy_content_to_clipboard(app: &adw::Application, content: &gdk::ContentProvider) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    if let Err(err) = clipboard.set_content(Some(content)) {
        eprintln!("annota: copying to the clipboard failed: {err}");
        return;
    }
    hold_until_replaced(app, &clipboard);
}

/// Keeps the application running while it owns the clipboard (Wayland empties it when its
/// owner exits), until another program takes the clipboard over.
fn hold_until_replaced(app: &adw::Application, clipboard: &gdk::Clipboard) {
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
/// "Copy to clipboard" then still confirms with a toast, otherwise no window opens — except on
/// Wayland, where only a focused Annota window can copy ([`copy_lands`]), so a window is there to
/// take the focus and to say truthfully whether the image is on the clipboard.
pub fn deliver(app: &adw::Application, frame: Frame, source: Source, window_was_open: bool) {
    let settings = gio::Settings::new(config::APP_ID);
    let plan = plan(
        source,
        AfterCapture::from_setting(settings.enum_("after-capture")),
    );
    let texture = texture_from_frame(&frame);
    let heading = title(source, frame.width(), frame.height());

    if !plan.open {
        let window =
            (window_was_open || on_wayland()).then(|| application::present_for_message(app));
        // Held until the copy is made: with no window nothing else keeps Annota running.
        let hold = app.hold();
        let app = app.clone();
        glib::spawn_future_local(async move {
            let copied =
                copy_when_it_lands(&app, &texture, window.as_ref().map(|w| w.upcast_ref())).await;
            if let Some(window) = window {
                application::show_toast(&window, &copy_toast(&app, &texture, &heading, copied));
            }
            drop(hold);
        });
        return;
    }

    // The editor: a new document, "Not saved" (mockups 12, 13).
    let pattern_name = glib::DateTime::now_local()
        .map(|now| {
            sanitize_stem(&preferences::expand_filename_pattern(
                &settings.string("filename-pattern"),
                &now,
            ))
        })
        .unwrap_or_else(|_| String::from("Annota"));
    let name = document_name(source, &pattern_name);
    let bitmap = annota::document::bitmap::Bitmap::from_argb32(
        frame.width(),
        frame.height(),
        frame.stride(),
        frame.data().to_vec(),
    );
    match bitmap {
        Ok(bitmap) if !plan.copy => {
            let offer = toast_offer(source, false);
            if crate::ui::editor::open_capture(app, bitmap, &name, &heading, offer) {
                return;
            }
            eprintln!("annota: the editor window could not be built; saving the capture");
        }
        Ok(bitmap) => {
            if let Some(opened) = crate::ui::editor::open_new_capture(app, bitmap, &name) {
                // "Both": copied once the editor has the focus, announced after that.
                let app = app.clone();
                glib::spawn_future_local(async move {
                    let window = opened.window();
                    let copied = copy_when_it_lands(&app, &texture, Some(&window)).await;
                    opened.announce(
                        &announced_title(&heading, copied),
                        toast_offer(source, copied),
                    );
                });
                return;
            }
            eprintln!("annota: the editor window could not be built; saving the capture");
        }
        Err(err) => eprintln!("annota: the capture cannot become a document: {err}"),
    }
    save_instead(app, &texture, &settings, source, &heading);
}

/// The toast of "Copy to clipboard": "… · Copied to clipboard" if the copy landed, else the
/// heading with Copy, whose click copies (and gives Annota the input the compositor asks for).
fn copy_toast(
    app: &adw::Application,
    texture: &gdk::Texture,
    heading: &str,
    copied: bool,
) -> adw::Toast {
    let toast = adw::Toast::builder()
        .title(announced_title(heading, copied))
        .build();
    toast.set_use_markup(false);
    if !copied {
        toast.set_button_label(Some(&gettext("Copy")));
        let texture = texture.clone();
        toast.connect_button_clicked(glib::clone!(
            #[weak]
            app,
            move |_| copy_to_clipboard(&app, &texture)
        ));
    }
    toast
}

/// The capture could not open in the editor: it is quick-saved as PNG into the save folder and
/// a toast names the file, so it is not lost. A capture is not copied on this path, also not with
/// "Both", so its toast offers Copy.
fn save_instead(
    app: &adw::Application,
    texture: &gdk::Texture,
    settings: &gio::Settings,
    source: Source,
    heading: &str,
) {
    let window = application::present_window(app);
    match quick_save(texture, settings) {
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
                Source::Region | Source::Window | Source::Screen => {
                    toast.set_button_label(Some(&gettext("Copy")));
                    let texture = texture.clone();
                    toast.connect_button_clicked(glib::clone!(
                        #[weak]
                        app,
                        move |_| copy_to_clipboard(&app, &texture)
                    ));
                }
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
                open: true,
                copy: false
            }
        );
        assert_eq!(
            plan(capture, AfterCapture::Clipboard),
            Plan {
                open: false,
                copy: true
            }
        );
        assert_eq!(
            plan(capture, AfterCapture::Both),
            Plan {
                open: true,
                copy: true
            }
        );
    }

    #[test]
    fn editor_documents_are_named_and_announced_like_the_mockups() {
        // Mockup 13: "Pasted Image", toast with Undo; mockup 12: the pattern name, toast with
        // Copy; with "Both" the capture is on the clipboard already.
        assert_eq!(
            document_name(Source::Clipboard, "Annota 2026-10-30"),
            "Pasted Image"
        );
        assert_eq!(
            document_name(Source::Region, "Capture 2026-10-30 14-22-08"),
            "Capture 2026-10-30 14-22-08"
        );
        assert_eq!(toast_offer(Source::Region, false), ToastOffer::Copy);
        assert_eq!(toast_offer(Source::Region, true), ToastOffer::Nothing);
        assert_eq!(toast_offer(Source::Clipboard, false), ToastOffer::Undo);
    }

    // spec: capture / After-capture destination / Both
    // spec: capture / After-capture destination / Copy to clipboard
    // spec: capture / After-capture destination / Copy to clipboard on Wayland
    #[test]
    fn a_copy_lands_on_wayland_only_with_focus_and_an_input_serial() {
        // Measured on GNOME 50.1 (2026-11-14, fix-window-capture-screencast "Decided during
        // apply"): mutter ignores set_selection without keyboard focus, and with serial 0, which
        // GTK sends until Annota has received a key or button press.
        assert!(copy_lands(false, false, false), "X11 takes every copy");
        assert!(copy_lands(true, true, true));
        assert!(
            !copy_lands(true, false, true),
            "no Annota surface has the focus"
        );
        assert!(
            !copy_lands(true, true, false),
            "a capture from a shortcut: no key or click in Annota yet"
        );
        assert!(gives_serial(gdk::EventType::KeyPress));
        assert!(gives_serial(gdk::EventType::ButtonPress));
        assert!(!gives_serial(gdk::EventType::MotionNotify));
        assert!(!gives_serial(gdk::EventType::EnterNotify));
        assert!(!gives_serial(gdk::EventType::FocusChange));
    }

    // spec: capture / After-capture destination / Both
    // spec: capture / After-capture destination / Copy to clipboard
    // spec: capture / After-capture destination / A copy that cannot be made
    #[test]
    fn the_toast_claims_a_copy_only_when_it_landed() {
        // A copy asked for but not made never says "Copied to clipboard" and offers Copy.
        let heading = title(Source::Window, 800, 500);
        assert_eq!(
            announced_title(&heading, true),
            "Window captured · 800 × 500 · Copied to clipboard"
        );
        assert_eq!(toast_offer(Source::Window, true), ToastOffer::Nothing);
        assert_eq!(
            announced_title(&heading, false),
            "Window captured · 800 × 500"
        );
        assert_eq!(toast_offer(Source::Window, false), ToastOffer::Copy);
        assert_eq!(toast_offer(Source::Screen, false), ToastOffer::Copy);
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
                    open: true,
                    copy: false
                }
            );
            assert!(plan(Source::File, after).open);
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

    // spec: capture / Window capture / Window mode from the welcome page
    #[test]
    fn a_window_capture_is_delivered_like_region_and_screen() {
        // fix-window-capture-screencast task 4.3: the ScreenCast frame goes through the same
        // delivery, opens in the editor with the toast "Window captured · W × H" and Copy.
        let editor = plan(Source::Window, AfterCapture::Editor);
        assert_eq!(editor, plan(Source::Region, AfterCapture::Editor));
        assert_eq!(toast_offer(Source::Window, editor.copy), ToastOffer::Copy);
        assert_eq!(
            title(Source::Window, 2248, 2848),
            "Window captured · 2248 × 2848"
        );
    }
}
