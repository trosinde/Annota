//! Application lifecycle: single instance, command line, main window, app actions.

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::capture::{self, import, shortcuts};
use crate::cli::{CaptureMode, DELAY_RANGE, EXIT_USAGE};
use crate::config;
use crate::preferences;

const UI_WINDOW: &str = "/io/github/trosinde/Annota/ui/window.ui";
/// Style class of the editor window (`data/ui/editor-window.blp`), which is not the main window.
const EDITOR_WINDOW_CLASS: &str = "annota-editor";

pub fn run() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(config::APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .resource_base_path("/io/github/trosinde/Annota")
        .build();

    add_options(&app);
    app.connect_handle_local_options(handle_local_options);
    app.connect_command_line(handle_command_line);
    app.connect_activate(|app| {
        present_window(app);
    });
    app.connect_startup(|app| {
        setup_actions(app);
        shortcuts::start_portal_shortcuts(app);
    });
    // Quitting keeps every open document's unsaved changes as an autosave (until Save exists).
    app.connect_shutdown(|_| crate::ui::editor::shut_down());

    app.run()
}

fn add_options(app: &adw::Application) {
    app.add_main_option(
        "capture",
        glib::Char::from(b'c'),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Start a capture: region, window or screen",
        Some("MODE"),
    );
    app.add_main_option(
        "delay",
        glib::Char::from(b'd'),
        glib::OptionFlags::NONE,
        glib::OptionArg::Int,
        "Delay the capture by 0 to 60 seconds",
        Some("SECONDS"),
    );
    app.add_main_option(
        "open",
        glib::Char::from(b'o'),
        glib::OptionFlags::NONE,
        glib::OptionArg::Filename,
        "Open an image or .annota file",
        Some("FILE"),
    );
    app.add_main_option(
        "version",
        glib::Char::from(b'v'),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Show the version and exit",
        None,
    );
}

/// Runs in the launching process: validate arguments before forwarding them.
fn handle_local_options(_app: &adw::Application, options: &glib::VariantDict) -> i32 {
    if options.contains("version") {
        println!("annota {}", config::VERSION);
        return 0;
    }
    if let Some(mode) = options.lookup::<String>("capture").ok().flatten() {
        if CaptureMode::parse(&mode).is_none() {
            eprintln!(
                "annota: invalid capture mode '{mode}'. Valid modes: {}",
                CaptureMode::VALID
            );
            return EXIT_USAGE;
        }
    }
    if let Some(delay) = options.lookup::<i32>("delay").ok().flatten() {
        if !DELAY_RANGE.contains(&delay) {
            eprintln!("annota: delay must be between 0 and 60 seconds, got {delay}");
            return EXIT_USAGE;
        }
    }
    -1 // continue: forward to the primary instance
}

/// Runs in the primary instance (also for requests forwarded from a second launch).
fn handle_command_line(app: &adw::Application, cmdline: &gio::ApplicationCommandLine) -> i32 {
    let options = cmdline.options_dict();

    // `--capture` starts the capture without raising the window first, so Annota itself is
    // not in the picture (spec app-shell: Single application instance). Autosaves of a previous
    // run are offered when the capture opens the editor (`editor::open_capture`), over it.
    if let Some(mode) = options
        .lookup::<String>("capture")
        .ok()
        .flatten()
        .and_then(|m| CaptureMode::parse(&m))
    {
        let delay = options
            .lookup::<i32>("delay")
            .ok()
            .flatten()
            .filter(|d| DELAY_RANGE.contains(d))
            .map(|d| d as u32);
        capture::start(app, capture::Request::with_defaults(mode, delay));
        return 0;
    }

    // Autosaves left by a previous run are offered once per process (mockup 14): here at the
    // primary instance's start, or when the editor first opens (captures, `--open`).
    let recovering = !cmdline.is_remote() && crate::ui::editor::offer_recovery(app);

    // `--open FILE` opens it in the editor (relative to the launching process' directory,
    // also when forwarded from a second launch).
    if let Some(path) = options.lookup::<std::path::PathBuf>("open").ok().flatten() {
        let path = cmdline
            .create_file_for_arg(path.as_os_str())
            .path()
            .unwrap_or(path);
        if !crate::ui::editor::open_file(app, &path, false) {
            let window = present_window(app);
            show_toast(
                &window,
                &adw::Toast::new(&gettext("The editor could not be opened")),
            );
        }
        return 0;
    }

    if !recovering {
        present_window(app);
    }
    0
}

/// Raises the main window, creating it on first use (never the editor window).
pub fn present_window(app: &adw::Application) -> adw::ApplicationWindow {
    let existing = app
        .windows()
        .into_iter()
        .filter(|w| !w.has_css_class(EDITOR_WINDOW_CLASS))
        .find_map(|w| w.downcast::<adw::ApplicationWindow>().ok());
    if let Some(window) = existing {
        window.present();
        return window;
    }
    let builder = gtk::Builder::from_resource(UI_WINDOW);
    let window: adw::ApplicationWindow = builder
        .object("window")
        .unwrap_or_else(|| adw::ApplicationWindow::new(app));
    window.set_application(Some(app));
    setup_launcher(app, &window, &builder);
    setup_welcome(app, &window, &builder);
    setup_drop_target(app, &window, &builder);
    window.present();
    window
}

/// Fills `container` with one keycap per key of `accel` ("Alt" "Print").
fn fill_keycaps(container: &gtk::Box, accel: &str) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
    let label = accel_label(accel);
    for key in label.split('+').filter(|k| !k.is_empty()) {
        let cap = gtk::Label::builder()
            .label(key)
            .css_classes(["annota-welcome-keycap"])
            .build();
        container.append(&cap);
    }
}

/// The welcome page's mode cards (bootstrap-app-shell mockup 01): each starts its capture,
/// which opens in the editor; their keycaps show the current shortcuts. The first card has the
/// keyboard focus.
fn setup_welcome(app: &adw::Application, window: &adw::ApplicationWindow, builder: &gtk::Builder) {
    let settings = gio::Settings::new(config::APP_ID);
    let mut first = None;
    for (mode, card, keys) in [
        (
            CaptureMode::Region,
            "welcome_region_card",
            "welcome_region_keys",
        ),
        (
            CaptureMode::Window,
            "welcome_window_card",
            "welcome_window_keys",
        ),
        (
            CaptureMode::Screen,
            "welcome_screen_card",
            "welcome_screen_keys",
        ),
    ] {
        let (Some(card), Some(keys)) = (
            builder.object::<gtk::Button>(card),
            builder.object::<gtk::Box>(keys),
        ) else {
            eprintln!("annota: window.ui is missing the welcome card '{card}'");
            continue;
        };
        let key = shortcuts::settings_key(mode);
        fill_keycaps(&keys, &settings.string(key));
        settings.connect_changed(
            Some(key),
            glib::clone!(
                #[weak]
                keys,
                move |settings, key| fill_keycaps(&keys, &settings.string(key))
            ),
        );
        // The card keeps the settings object (and with it the keycap updates) alive.
        let keep = settings.clone();
        card.connect_clicked(glib::clone!(
            #[weak]
            app,
            move |_| {
                let _ = &keep;
                app.activate_action("capture", Some(&mode.as_str().to_variant()))
            }
        ));
        first.get_or_insert(card);
    }
    if let Some(open_keys) = builder.object::<gtk::Box>("welcome_open_keys") {
        fill_keycaps(&open_keys, "<Control>o");
    }
    if let Some(card) = first {
        gtk::prelude::GtkWindowExt::set_focus(window, Some(&card));
    }
}

pub fn show_toast(window: &adw::ApplicationWindow, toast: &adw::Toast) {
    if let Some(overlay) = window.content().and_then(|c| find_toast_overlay(&c)) {
        overlay.add_toast(toast.clone());
    }
}

fn find_toast_overlay(widget: &gtk::Widget) -> Option<adw::ToastOverlay> {
    if let Ok(overlay) = widget.clone().downcast::<adw::ToastOverlay>() {
        return Some(overlay);
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_toast_overlay(&c) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

/// Human-readable form of a GTK accelerator, e.g. "Shift+Ctrl+V"; empty if none.
pub fn accel_label(accel: &str) -> String {
    gtk::accelerator_parse(accel)
        .map(|(key, mods)| gtk::accelerator_get_label(key, mods).to_string())
        .unwrap_or_default()
}

/// The capture launcher popover in the header bar (mockup 01).
fn setup_launcher(app: &adw::Application, window: &adw::ApplicationWindow, builder: &gtk::Builder) {
    macro_rules! get {
        ($name:expr) => {
            match builder.object($name) {
                Some(object) => object,
                None => {
                    eprintln!("annota: window.ui is missing object '{}'", $name);
                    return;
                }
            }
        };
    }
    let menu_button: gtk::MenuButton = get!("capture_menu_button");
    let popover: gtk::Popover = get!("capture_launcher");
    let toggles: [(CaptureMode, gtk::ToggleButton, gtk::Label); 3] = [
        (
            CaptureMode::Region,
            get!("launcher_region_button"),
            get!("launcher_region_accel"),
        ),
        (
            CaptureMode::Window,
            get!("launcher_window_button"),
            get!("launcher_window_accel"),
        ),
        (
            CaptureMode::Screen,
            get!("launcher_screen_button"),
            get!("launcher_screen_accel"),
        ),
    ];
    let delay: gtk::SpinButton = get!("launcher_delay");
    let pointer: gtk::Switch = get!("launcher_pointer");
    let capture_button: gtk::Button = get!("launcher_capture_button");
    let capture_label: gtk::Label = get!("launcher_capture_label");
    let capture_accel: gtk::Label = get!("launcher_capture_accel");
    let paste_accel: gtk::Label = get!("launcher_paste_accel");
    let open_accel: gtk::Label = get!("launcher_open_accel");

    let settings = gio::Settings::new(config::APP_ID);
    settings.bind("capture-delay", &delay, "value").build();
    settings.bind("include-pointer", &pointer, "active").build();
    delay.connect_output(|spin| {
        spin.set_text(
            &gettext("{seconds} s").replace("{seconds}", &spin.value_as_int().to_string()),
        );
        glib::Propagation::Stop
    });
    delay.connect_input(|spin| {
        let digits: String = spin.text().chars().filter(char::is_ascii_digit).collect();
        Some(digits.parse::<f64>().map_err(|_| ()))
    });
    paste_accel.set_text(&accel_label("<Shift><Control>v"));
    open_accel.set_text(&accel_label("<Control>o"));

    let selected_mode = {
        let toggles = toggles.clone();
        move || {
            toggles
                .iter()
                .find(|(_, toggle, _)| toggle.is_active())
                .map(|(mode, _, _)| *mode)
                .unwrap_or(CaptureMode::Region)
        }
    };
    let refresh = {
        let toggles = toggles.clone();
        let settings = settings.clone();
        let selected_mode = selected_mode.clone();
        move || {
            for (mode, _, label) in &toggles {
                label.set_text(&accel_label(
                    &settings.string(shortcuts::settings_key(*mode)),
                ));
            }
            let mode = selected_mode();
            capture_label.set_text(&match mode {
                CaptureMode::Region => gettext("Capture Region"),
                CaptureMode::Window => gettext("Capture Window"),
                CaptureMode::Screen => gettext("Capture Screen"),
            });
            capture_accel.set_text(&accel_label(
                &settings.string(shortcuts::settings_key(mode)),
            ));
        }
    };

    let initial = CaptureMode::from_index(settings.enum_("capture-mode"));
    for (mode, toggle, _) in &toggles {
        toggle.set_active(*mode == initial);
    }
    refresh();
    for (mode, toggle, _) in &toggles {
        let mode = *mode;
        let settings = settings.clone();
        let refresh = refresh.clone();
        toggle.connect_toggled(move |toggle| {
            if toggle.is_active() {
                let _ = settings.set_enum("capture-mode", mode.index());
                refresh();
            }
        });
    }
    popover.connect_show(move |_| refresh());

    capture_button.connect_clicked(glib::clone!(
        #[weak]
        app,
        #[weak]
        popover,
        move |_| {
            popover.popdown();
            app.activate_action("capture", Some(&selected_mode().as_str().to_variant()));
        }
    ));

    let new_capture = gio::ActionEntry::builder("new-capture")
        .activate(move |_: &adw::ApplicationWindow, _, _| menu_button.popup())
        .build();
    window.add_action_entries([new_capture]);
}

/// Drop a PNG/JPG/WebP onto the window to open it as a new capture (mockup 14), or an
/// `.annota` document to open it; both in the editor.
fn setup_drop_target(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    builder: &gtk::Builder,
) {
    let Some(drop_zone) = builder.object::<gtk::Box>("drop_zone") else {
        eprintln!("annota: window.ui is missing object 'drop_zone'");
        return;
    };
    let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    target.connect_enter(glib::clone!(
        #[weak]
        drop_zone,
        #[upgrade_or]
        gdk::DragAction::empty(),
        move |_, _, _| {
            drop_zone.set_visible(true);
            gdk::DragAction::COPY
        }
    ));
    target.connect_leave(glib::clone!(
        #[weak]
        drop_zone,
        move |_| drop_zone.set_visible(false)
    ));
    target.connect_drop(glib::clone!(
        #[weak]
        app,
        #[weak]
        drop_zone,
        #[upgrade_or]
        false,
        move |_, value, _, _| {
            drop_zone.set_visible(false);
            let Ok(list) = value.get::<gdk::FileList>() else {
                return false;
            };
            match list.files().first() {
                Some(file) => {
                    import::open_file(&app, file);
                    true
                }
                None => false,
            }
        }
    ));
    window.add_controller(target);
}

fn setup_actions(app: &adw::Application) {
    let quit = gio::ActionEntry::builder("quit")
        .activate(|app: &adw::Application, _, _| app.quit())
        .build();
    let about = gio::ActionEntry::builder("about")
        .activate(|app: &adw::Application, _, _| show_about(app))
        .build();
    let prefs = gio::ActionEntry::builder("preferences")
        .activate(|app: &adw::Application, _, _| show_preferences(app))
        .build();
    let capture = gio::ActionEntry::builder("capture")
        .parameter_type(Some(glib::VariantTy::STRING))
        .activate(|app: &adw::Application, _, parameter| {
            let mode = parameter
                .and_then(|p| p.get::<String>())
                .and_then(|m| CaptureMode::parse(&m));
            if let Some(mode) = mode {
                capture::start(app, capture::Request::with_defaults(mode, None));
            }
        })
        .build();
    let paste = gio::ActionEntry::builder("paste-image")
        .activate(|app: &adw::Application, _, _| import::paste(app))
        .build();
    let open = gio::ActionEntry::builder("open-image")
        .activate(|app: &adw::Application, _, _| import::choose_file(app))
        .build();
    // Ctrl+L, main menu of the editor and the tray's Open Library button: the capture library
    // is add-capture-library's; until it exists, say so in the window it was asked from.
    let library = gio::ActionEntry::builder("open-library")
        .activate(|app: &adw::Application, _, _| {
            let window = app
                .active_window()
                .and_then(|w| w.downcast::<adw::ApplicationWindow>().ok());
            if let Some(window) = window {
                show_toast(
                    &window,
                    &adw::Toast::new(&gettext("The capture library arrives in milestone 0.1")),
                );
            }
        })
        .build();
    app.add_action_entries([quit, about, prefs, capture, paste, open, library]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);
    app.set_accels_for_action("app.paste-image", &["<Shift><Control>v"]);
    app.set_accels_for_action("app.open-image", &["<Control>o"]);
    app.set_accels_for_action("app.open-library", &["<Control>l"]);
    app.set_accels_for_action("win.new-capture", &["<Control>n"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
}

fn show_preferences(app: &adw::Application) {
    if let Some(window) = app.active_window() {
        preferences::present(&window);
    }
}

fn show_about(app: &adw::Application) {
    let about = adw::AboutDialog::builder()
        .application_name("Annota")
        .application_icon(config::APP_ID)
        .version(config::VERSION)
        .developer_name("The Annota Contributors")
        .developers(vec!["Thorsten Rosin"])
        .website("https://github.com/trosinde/Annota")
        .issue_url("https://github.com/trosinde/Annota/issues")
        .license_type(gtk::License::MitX11)
        .copyright("© 2026 The Annota Contributors")
        .comments(gettext(
            "Works entirely on this computer. No account, no tracking.",
        ))
        .build();
    about.present(app.active_window().as_ref());
}
