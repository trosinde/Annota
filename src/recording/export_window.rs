//! The Export Recording window (tasks 5.1–5.4, mockups 07–11, design.md D1; spec "Export after
//! recording", "Discard a recording", "Working file" / "Recording error").
//!
//! It opens for every recording that ended ([`open`], from `Hooks::on_ended`): Stop, the Record
//! shortcut, the desktop ending the session, the recorded window closed, an error (then with
//! the banner of mockup 11). It shows a poster frame and the duration (Discoverer and one frame
//! of the working file, both read off the main thread), File name (the working file's name),
//! Folder (the last folder exported to, `record-export-folder`; empty: the recordings folder),
//! Format MP4 | WebM (`record-export-format`) and Include audio (`record-include-audio`;
//! insensitive and off without audio). Export runs [`export::start`] with progress and Cancel
//! (08) and ends in "Saved" (09) with Show in Files and Close; the working file is removed then.
//! Discard, closing the window and `Esc` before the export has finished ask "Discard
//! Recording?" (10); Discard deletes the working file and closes. After "Saved" closing just
//! closes.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use futures_util::StreamExt;
use gettextrs::gettext;
use gstreamer as gst;
use gtk::{gdk, gio, glib};

use super::audio::AudioSource;
use super::control::{announce, format_elapsed};
use super::export::{self, Export, ExportError, ExportEvent, Format, Info};
use super::files;
use super::session::Ended;
use crate::config;
use crate::preferences;

const UI: &str = "/io/github/trosinde/Annota/ui/export-recording.ui";
const DISCARD_UI: &str = "/io/github/trosinde/Annota/ui/discard-recording-dialog.ui";
/// The poster frame's size in the window (redline 07).
const POSTER_SIZE: (f32, f32) = (192.0, 120.0);

/// Where the window is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// File name, folder, format, audio (07, 11).
    Settings,
    /// Progress and Cancel (08).
    Exporting,
    /// "Saved" (09); the working file is gone.
    Saved,
}

struct Widgets {
    window: adw::Window,
    error_banner: adw::Banner,
    poster: gtk::Picture,
    duration_badge: gtk::Label,
    name_label: gtk::Label,
    caption_label: gtk::Label,
    body_stack: gtk::Stack,
    settings_box: gtk::Box,
    name_row: adw::EntryRow,
    extension_label: gtk::Label,
    folder_row: adw::ActionRow,
    folder_button: gtk::Button,
    format_row: adw::ActionRow,
    mp4_button: gtk::ToggleButton,
    webm_button: gtk::ToggleButton,
    audio_row: adw::SwitchRow,
    saved_name: gtk::Label,
    saved_caption: gtk::Label,
    show_button: gtk::Button,
    close_button: gtk::Button,
    footer_stack: gtk::Stack,
    discard_button: gtk::Button,
    export_button: gtk::Button,
    percent_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    cancel_button: gtk::Button,
}

pub struct ExportWindow {
    w: Widgets,
    settings: gio::Settings,
    /// The working file.
    path: PathBuf,
    /// The recorded time (Discoverer's, once known).
    duration: Cell<gst::ClockTime>,
    audio: Vec<AudioSource>,
    info: Cell<Option<Info>>,
    folder: RefCell<PathBuf>,
    state: Cell<State>,
    export: RefCell<Option<Export>>,
    saved: RefCell<Option<PathBuf>>,
    /// The window closes for good (after Discard or Close): no question any more.
    closing: Cell<bool>,
    /// "Discard Recording?" while it is shown.
    alert: RefCell<Option<adw::AlertDialog>>,
    /// The recording ended by an error: the window opened with the banner of mockup 11.
    by_error: bool,
}

thread_local! {
    /// The open Export Recording windows (tests, and nothing else holds them).
    static OPEN: RefCell<Vec<Rc<ExportWindow>>> = const { RefCell::new(Vec::new()) };
}

/// Opens the Export Recording window for a recording that has ended, and presents it.
pub fn open(app: &adw::Application, ended: Ended) {
    let Some(w) = widgets() else {
        eprintln!(
            "annota: export-recording.ui is incomplete; the recording is kept at {}",
            ended.finished.path.display()
        );
        return;
    };
    let settings = gio::Settings::new(config::APP_ID);
    let folder = export_folder(&settings);
    let this = Rc::new(ExportWindow {
        w,
        settings,
        path: ended.finished.path.clone(),
        duration: Cell::new(ended.finished.duration),
        audio: ended.finished.audio.clone(),
        info: Cell::new(None),
        folder: RefCell::new(folder),
        state: Cell::new(State::Settings),
        export: RefCell::new(None),
        saved: RefCell::new(None),
        closing: Cell::new(false),
        alert: RefCell::new(None),
        by_error: ended.by_error(),
    });
    this.setup(app);
    OPEN.with(|open| open.borrow_mut().push(this.clone()));
    this.w.window.present();
    // Redline: File name has the initial focus, the cursor after the name (mockup 07).
    this.w.name_row.grab_focus();
    this.w.name_row.set_position(-1);
    this.load_poster();
}

/// The last folder exported to; the recordings folder when there is none.
fn export_folder(settings: &gio::Settings) -> PathBuf {
    let last = settings.string("record-export-folder");
    if last.trim().is_empty() {
        files::recordings_folder(&settings.string("record-folder"))
    } else {
        PathBuf::from(last.as_str())
    }
}

/// "Microphone + system audio", "Microphone", "System audio" or "No audio" (redline 07).
pub fn audio_text(audio: &[AudioSource]) -> String {
    let microphone = audio.contains(&AudioSource::Microphone);
    let system = audio.contains(&AudioSource::SystemAudio);
    match (microphone, system) {
        (true, true) => gettext("Microphone + system audio"),
        (true, false) => gettext("Microphone"),
        (false, true) => gettext("System audio"),
        (false, false) => gettext("No audio"),
    }
}

/// "1280 × 800 · 0:52 · Microphone + system audio"; without the size until it is known.
pub fn caption(
    size: Option<(u32, u32)>,
    duration: gst::ClockTime,
    audio: &[AudioSource],
) -> String {
    let mut parts = Vec::new();
    if let Some((w, h)) = size {
        parts.push(format!("{w} × {h}"));
    }
    parts.push(format_elapsed(duration));
    parts.push(audio_text(audio));
    parts.join(" · ")
}

/// "14.2 MB · ~/Videos/Annota" (redline 09).
pub fn saved_caption(bytes: u64, folder: &Path) -> String {
    format!(
        "{} · {}",
        glib::format_size(bytes),
        preferences::display_path(folder)
    )
}

impl ExportWindow {
    fn setup(self: &Rc<Self>, app: &adw::Application) {
        let w = &self.w;
        w.window.set_application(Some(app));
        center_banner_text(&w.error_banner);
        self.show_opening_banner();
        hide_edit_icon(&w.name_row);
        let name = files::stem(&self.path);
        w.name_label.set_label(&name);
        w.name_row.set_text(&name);
        self.show_summary();
        self.show_folder();

        // Format: the saved choice; each change is saved.
        let format = Format::from_nick(&self.settings.string("record-export-format"));
        w.webm_button.set_active(format == Format::Webm);
        w.mp4_button.set_active(format == Format::Mp4);
        self.show_format();
        for button in [&w.mp4_button, &w.webm_button] {
            let weak = Rc::downgrade(self);
            button.connect_toggled(move |button| {
                if let (Some(this), true) = (weak.upgrade(), button.is_active()) {
                    let _ = this
                        .settings
                        .set_string("record-export-format", this.format().nick());
                    this.show_format();
                }
            });
        }

        // Include audio: the saved choice; without audio insensitive and off (nothing saved).
        if self.audio.is_empty() {
            w.audio_row.set_active(false);
            w.audio_row.set_sensitive(false);
        } else {
            self.settings
                .bind("record-include-audio", &w.audio_row, "active")
                .build();
        }

        let weak = Rc::downgrade(self);
        w.name_row.connect_changed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.show_name_valid();
            }
        });
        let weak = Rc::downgrade(self);
        w.folder_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.choose_folder();
            }
        });
        let weak = Rc::downgrade(self);
        w.export_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.export();
            }
        });
        let weak = Rc::downgrade(self);
        w.cancel_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.cancel_export();
            }
        });
        let weak = Rc::downgrade(self);
        w.discard_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.ask_discard();
            }
        });
        let weak = Rc::downgrade(self);
        w.show_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                if let Some(path) = this.saved.borrow().clone() {
                    show_in_files(&this.w.window, &path);
                }
            }
        });
        let weak = Rc::downgrade(self);
        w.close_button.connect_clicked(move |_| {
            if let Some(this) = weak.upgrade() {
                this.w.window.close();
            }
        });

        // Closing (the close button, Alt+F4, Esc) before the export finished asks first.
        let weak = Rc::downgrade(self);
        w.window.connect_close_request(move |_| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if this.closing.get() || this.state.get() == State::Saved {
                this.forget();
                return glib::Propagation::Proceed;
            }
            this.ask_discard();
            glib::Propagation::Stop
        });
        let escape = gtk::ShortcutController::new();
        escape.set_scope(gtk::ShortcutScope::Managed);
        escape.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(|widget, _| {
                if let Some(window) = widget.downcast_ref::<gtk::Window>() {
                    window.close();
                }
                glib::Propagation::Stop
            })),
        ));
        w.window.add_controller(escape);
    }

    /// The banner as the window opened: mockup 11's after an error, else none.
    fn show_opening_banner(&self) {
        let banner = &self.w.error_banner;
        if self.by_error {
            banner.set_title(&gettext(
                "The recording stopped because of an error. What was recorded until then is kept.",
            ));
        }
        banner.set_revealed(self.by_error);
    }

    fn format(&self) -> Format {
        if self.w.webm_button.is_active() {
            Format::Webm
        } else {
            Format::Mp4
        }
    }

    fn show_format(&self) {
        let format = self.format();
        self.w
            .extension_label
            .set_label(&format!(".{}", format.extension()));
        self.w.format_row.set_subtitle(&match format {
            Format::Mp4 => gettext("H.264 + AAC · plays almost everywhere"),
            Format::Webm => gettext("VP9 + Opus · open format for the web"),
        });
    }

    fn show_folder(&self) {
        let folder = self.folder.borrow().clone();
        self.w
            .folder_row
            .set_subtitle(&preferences::display_path(&folder));
    }

    /// Duration badge and caption.
    fn show_summary(&self) {
        let duration = self.duration.get();
        self.w.duration_badge.set_label(&format_elapsed(duration));
        let size = self.info.get().map(|i| (i.width, i.height));
        self.w
            .caption_label
            .set_label(&caption(size, duration, &self.audio));
    }

    /// The file name to write, if usable: not empty, no folder separator.
    fn file_name(&self) -> Option<String> {
        let text = self.w.name_row.text();
        let name = text.trim();
        (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
    }

    fn show_name_valid(&self) {
        let valid = self.file_name().is_some();
        self.w.export_button.set_sensitive(valid);
        if valid {
            self.w.name_row.remove_css_class("error");
        } else {
            self.w.name_row.add_css_class("error");
        }
    }

    /// Discoverer and the poster frame, off the main thread.
    fn load_poster(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let path = self.path.clone();
        let duration = self.duration.get();
        glib::spawn_future_local(async move {
            let probed = gio::spawn_blocking(move || {
                let info = export::probe(&path);
                let at = info.as_ref().map_or(duration, |i| i.duration);
                (info, export::poster(&path, at))
            })
            .await;
            let Some(this) = weak.upgrade() else {
                return;
            };
            let Ok((info, poster)) = probed else {
                return;
            };
            match info {
                Ok(info) => {
                    if !info.duration.is_zero() {
                        this.duration.set(info.duration);
                    }
                    this.info.set(Some(info));
                    this.show_summary();
                }
                Err(err) => eprintln!("annota: recording: cannot read the working file: {err}"),
            }
            match poster {
                Ok(poster) => {
                    let texture = gdk::MemoryTexture::new(
                        poster.width,
                        poster.height,
                        gdk::MemoryFormat::R8g8b8a8,
                        &glib::Bytes::from_owned(poster.data),
                        poster.stride,
                    );
                    this.w
                        .poster
                        .set_paintable(poster_paintable(&texture).as_ref());
                }
                Err(err) => eprintln!("annota: recording: no poster frame: {err}"),
            }
        });
    }

    fn choose_folder(self: &Rc<Self>) {
        let initial = self.folder.borrow().clone();
        let mut builder = gtk::FileDialog::builder()
            .title(gettext("Choose Folder"))
            .modal(true);
        if initial.is_dir() {
            builder = builder.initial_folder(&gio::File::for_path(&initial));
        }
        let chooser = builder.build();
        let window = self.w.window.clone().upcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(folder) = chooser.select_folder_future(Some(&window)).await else {
                return;
            };
            if let (Some(this), Some(path)) = (weak.upgrade(), folder.path()) {
                this.set_folder(path);
            }
        });
    }

    /// The folder to export to (the folder chooser; tests).
    pub fn set_folder(&self, folder: PathBuf) {
        self.folder.replace(folder);
        self.show_folder();
    }

    fn set_state(&self, state: State) {
        let w = &self.w;
        self.state.set(state);
        match state {
            State::Settings => {
                w.settings_box.set_sensitive(true);
                w.body_stack.set_visible_child_name("settings");
                w.footer_stack.set_visible(true);
                w.footer_stack.set_visible_child_name("buttons");
                w.window.set_default_widget(Some(&w.export_button));
            }
            State::Exporting => {
                // Rows insensitive, progress + Cancel (focused) in the footer; a failure of an
                // earlier export is not said any more.
                self.show_opening_banner();
                w.settings_box.set_sensitive(false);
                w.progress_bar.set_fraction(0.0);
                w.percent_label.set_label(&percent(0.0));
                w.footer_stack.set_visible_child_name("progress");
                w.window.set_default_widget(None::<&gtk::Widget>);
                w.cancel_button.grab_focus();
            }
            State::Saved => {
                // "Discard Recording?" asked during the export has nothing left to discard.
                let alert = self.alert.borrow().clone();
                if let Some(alert) = alert {
                    alert.force_close();
                }
                w.body_stack.set_visible_child_name("saved");
                w.footer_stack.set_visible(false);
                w.window.set_default_widget(Some(&w.close_button));
                w.close_button.grab_focus();
            }
        }
    }

    fn export(self: &Rc<Self>) {
        if self.state.get() != State::Settings {
            return;
        }
        let Some(name) = self.file_name() else {
            return;
        };
        let folder = self.folder.borrow().clone();
        let format = self.format();
        let include_audio = self.w.audio_row.is_active() && !self.audio.is_empty();
        let tracks = self
            .info
            .get()
            .map_or(self.audio.len(), |info| info.audio_tracks);
        let started = export::start(
            &self.path,
            &folder,
            &name,
            format,
            include_audio,
            tracks,
            self.duration.get(),
        );
        let (export, mut events) = match started {
            Ok(started) => started,
            Err(err) => {
                self.failed(&err);
                return;
            }
        };
        self.export.replace(Some(export));
        self.set_state(State::Exporting);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Some(event) = events.next().await {
                let Some(this) = weak.upgrade() else {
                    return;
                };
                match event {
                    ExportEvent::Progress(done) => this.show_progress(done),
                    ExportEvent::Done(result) => {
                        this.export.replace(None);
                        this.finished(result);
                        return;
                    }
                }
            }
        });
    }

    fn show_progress(&self, done: f64) {
        self.w.progress_bar.set_fraction(done);
        let text = percent(done);
        if self.w.percent_label.label() != text {
            self.w.percent_label.set_label(&text);
        }
    }

    fn finished(self: &Rc<Self>, result: Result<PathBuf, ExportError>) {
        if self.closing.get() {
            return;
        }
        match result {
            Ok(path) => {
                // The working file goes once the export is written.
                if let Err(err) = std::fs::remove_file(&self.path) {
                    eprintln!("annota: recording: the working file was not removed: {err}");
                }
                let folder = self.folder.borrow().clone();
                let _ = self
                    .settings
                    .set_string("record-export-folder", &folder.to_string_lossy());
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let size = std::fs::metadata(&path).map_or(0, |m| m.len());
                self.w.saved_name.set_label(&name);
                self.w
                    .saved_caption
                    .set_label(&saved_caption(size, &folder));
                self.saved.replace(Some(path));
                self.set_state(State::Saved);
            }
            Err(ExportError::Cancelled) => {
                // Back to the settings; the working file is kept.
                self.set_state(State::Settings);
                self.w.export_button.grab_focus();
            }
            Err(err) => self.failed(&err),
        }
    }

    /// The export did not happen: logged, said in the banner, back to the settings.
    fn failed(&self, err: &ExportError) {
        eprintln!("annota: recording: export failed: {err}");
        self.set_state(State::Settings);
        let text = gettext("The recording could not be exported. It is kept.");
        self.w.error_banner.set_title(&text);
        self.w.error_banner.set_revealed(true);
        announce(&self.w.window, &text);
        self.w.export_button.grab_focus();
    }

    fn cancel_export(&self) {
        if let Some(export) = self.export.borrow().as_ref() {
            export.cancel();
        }
    }

    /// "Discard Recording?" (10): “<name>” (<duration>) will be deleted.
    fn ask_discard(self: &Rc<Self>) {
        if self.alert.borrow().is_some() {
            return;
        }
        let Some(alert) =
            gtk::Builder::from_resource(DISCARD_UI).object::<adw::AlertDialog>("discard_dialog")
        else {
            eprintln!("annota: discard-recording-dialog.ui is incomplete");
            return;
        };
        let body = gettext("“{name}” ({duration}) will be deleted. This cannot be undone.")
            .replace("{name}", &files::stem(&self.path))
            .replace("{duration}", &format_elapsed(self.duration.get()));
        alert.set_body(&body);
        let weak = Rc::downgrade(self);
        alert.connect_response(None, move |_, response| {
            if let (Some(this), "discard") = (weak.upgrade(), response) {
                this.discard();
            }
        });
        let weak = Rc::downgrade(self);
        alert.connect_closed(move |_| {
            if let Some(this) = weak.upgrade() {
                this.alert.replace(None);
            }
        });
        self.alert.replace(Some(alert.clone()));
        // Inside this window, as mockup 10 draws it: libadwaita (1.6 and later) shows the
        // dialogs of a window that cannot be resized in a window of their own, so the window is
        // resizable only while the alert is presented (measured with libadwaita 1.9: the alert
        // stays inside, the window keeps its 560 px and cannot be resized). Harmless with 1.5,
        // which always presents inside.
        let window = &self.w.window;
        window.set_resizable(true);
        alert.present(Some(window));
        window.set_resizable(false);
    }

    /// Discard: the working file is deleted (a running export is cancelled), the window closes.
    fn discard(&self) {
        self.cancel_export();
        if let Err(err) = std::fs::remove_file(&self.path) {
            eprintln!("annota: recording: the working file was not deleted: {err}");
        }
        self.closing.set(true);
        self.w.window.close();
    }

    fn forget(&self) {
        OPEN.with(|open| {
            open.borrow_mut()
                .retain(|other| other.w.window != self.w.window)
        });
    }
}

/// The poster (redline 07): 192 × 120, the frame scaled to cover it (centred, cut at the
/// edges), drawn from the full-resolution texture so it stays sharp at scale 2. A paintable of
/// that intrinsic size keeps the picture from asking for the texture's own size.
fn poster_paintable(texture: &gdk::MemoryTexture) -> Option<gdk::Paintable> {
    let (w, h) = (POSTER_SIZE.0, POSTER_SIZE.1);
    let (tw, th) = (texture.width() as f32, texture.height() as f32);
    if tw <= 0.0 || th <= 0.0 {
        return None;
    }
    let scale = (w / tw).max(h / th);
    let (sw, sh) = (tw * scale, th * scale);
    let snapshot = gtk::Snapshot::new();
    let bounds = gtk::graphene::Rect::new(0.0, 0.0, w, h);
    snapshot.push_clip(&bounds);
    snapshot.append_texture(
        texture,
        &gtk::graphene::Rect::new((w - sw) / 2.0, (h - sh) / 2.0, sw, sh),
    );
    snapshot.pop();
    snapshot.to_paintable(Some(&gtk::graphene::Size::new(w, h)))
}

/// The banner's title centred and wrapping, as mockup 11 draws it (two centred lines at 560
/// px). `AdwBanner` has no property for it; its title is its only label.
fn center_banner_text(banner: &adw::Banner) {
    if let Some(label) = descendants(banner.upcast_ref())
        .into_iter()
        .find_map(|w| w.downcast::<gtk::Label>().ok())
    {
        label.set_justify(gtk::Justification::Center);
        label.set_wrap(true);
        label.set_xalign(0.5);
    }
}

/// The File name row without its edit pencil, and without the room it takes (mockups 08, 10:
/// the dimmed extension ends the row). `AdwEntryRow` has no property for it.
fn hide_edit_icon(row: &adw::EntryRow) {
    for widget in descendants(row.upcast_ref()) {
        if widget.has_css_class("edit-icon") {
            widget.set_visible(false);
            // libadwaita shows it again when the row's state changes.
            widget.connect_visible_notify(|w| {
                if w.is_visible() {
                    w.set_visible(false);
                }
            });
        }
    }
}

/// `widget`'s descendants, depth first.
fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = Vec::new();
    let mut child = widget.first_child();
    while let Some(c) = child {
        all.push(c.clone());
        all.extend(descendants(&c));
        child = c.next_sibling();
    }
    all
}

/// "46 %".
fn percent(done: f64) -> String {
    gettext("{percent} %").replace("{percent}", &((done * 100.0).floor() as u32).to_string())
}

/// Show in Files (09): the file manager opens the folder with `path` selected
/// (`org.freedesktop.FileManager1.ShowItems` on the session bus); if that call fails, GTK opens
/// the containing folder.
fn show_in_files(window: &adw::Window, path: &Path) {
    let file = gio::File::for_path(path);
    let uri = file.uri().to_string();
    let window = window.clone().upcast::<gtk::Window>();
    glib::spawn_future_local(async move {
        let shown = match gio::bus_get_future(gio::BusType::Session).await {
            Ok(bus) => bus
                .call_future(
                    Some("org.freedesktop.FileManager1"),
                    "/org/freedesktop/FileManager1",
                    "org.freedesktop.FileManager1",
                    "ShowItems",
                    Some(&(vec![uri], "").to_variant()),
                    None,
                    gio::DBusCallFlags::NONE,
                    10_000,
                )
                .await
                .map(|_| ()),
            Err(err) => Err(err),
        };
        if let Err(err) = shown {
            eprintln!("annota: FileManager1.ShowItems failed ({err}); opening the folder");
            gtk::FileLauncher::new(Some(&file)).open_containing_folder(
                Some(&window),
                gio::Cancellable::NONE,
                |result| {
                    if let Err(err) = result {
                        eprintln!("annota: cannot show the file: {err}");
                    }
                },
            );
        }
    });
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: export-recording.ui is missing object '{}'", $name);
                return None;
            }
        }
    };
}

fn widgets() -> Option<Widgets> {
    let builder = gtk::Builder::from_resource(UI);
    Some(Widgets {
        window: obj!(builder, "export_window"),
        error_banner: obj!(builder, "error_banner"),
        poster: obj!(builder, "poster_picture"),
        duration_badge: obj!(builder, "duration_badge"),
        name_label: obj!(builder, "name_label"),
        caption_label: obj!(builder, "caption_label"),
        body_stack: obj!(builder, "body_stack"),
        settings_box: obj!(builder, "settings_box"),
        name_row: obj!(builder, "name_row"),
        extension_label: obj!(builder, "extension_label"),
        folder_row: obj!(builder, "folder_row"),
        folder_button: obj!(builder, "folder_button"),
        format_row: obj!(builder, "format_row"),
        mp4_button: obj!(builder, "mp4_button"),
        webm_button: obj!(builder, "webm_button"),
        audio_row: obj!(builder, "audio_row"),
        saved_name: obj!(builder, "saved_name"),
        saved_caption: obj!(builder, "saved_caption"),
        show_button: obj!(builder, "show_button"),
        close_button: obj!(builder, "close_button"),
        footer_stack: obj!(builder, "footer_stack"),
        discard_button: obj!(builder, "discard_button"),
        export_button: obj!(builder, "export_button"),
        percent_label: obj!(builder, "percent_label"),
        progress_bar: obj!(builder, "progress_bar"),
        cancel_button: obj!(builder, "cancel_button"),
    })
}

/// Tests: the open Export Recording windows, oldest first.
#[cfg(test)]
pub(crate) fn open_windows() -> Vec<Rc<ExportWindow>> {
    OPEN.with(|open| open.borrow().clone())
}

#[cfg(test)]
impl ExportWindow {
    pub(crate) fn window(&self) -> &adw::Window {
        &self.w.window
    }
    pub(crate) fn state(&self) -> State {
        self.state.get()
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    /// "Discard Recording?", while it is shown.
    pub(crate) fn discard_alert(&self) -> Option<adw::AlertDialog> {
        self.alert.borrow().clone().filter(|a| a.is_mapped())
    }
    pub(crate) fn poster_shown(&self) -> bool {
        self.w.poster.paintable().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: screen-recording / Export after recording / Export as MP4
    #[test]
    fn the_caption_names_size_duration_and_audio() {
        let s = gst::ClockTime::from_seconds;
        let both = [AudioSource::Microphone, AudioSource::SystemAudio];
        assert_eq!(
            caption(Some((1280, 800)), s(52), &both),
            "1280 × 800 · 0:52 · Microphone + system audio"
        );
        assert_eq!(
            caption(None, s(10), &[AudioSource::Microphone]),
            "0:10 · Microphone"
        );
        assert_eq!(audio_text(&[AudioSource::SystemAudio]), "System audio");
        assert_eq!(audio_text(&[]), "No audio");
        let home = glib::home_dir();
        assert_eq!(
            saved_caption(14_200_000, &home.join("Videos/Annota")),
            // GLib's size, with a no-break space before the unit.
            "14.2\u{a0}MB · ~/Videos/Annota"
        );
        assert_eq!(percent(0.468), "46 %");
    }
}
