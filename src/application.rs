//! Application lifecycle: single instance, command line, main window, app actions.

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use crate::capture::{self, import, shortcuts};
use crate::cli::{CaptureMode, DELAY_RANGE, EXIT_USAGE};
use crate::config;
use crate::preferences;

const UI_WINDOW: &str = "/io/github/trosinde/Annota/ui/window.ui";

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
    // Development only: show a document in the canvas preview (until the editor exists).
    #[cfg(debug_assertions)]
    app.add_main_option(
        "dev-canvas",
        glib::Char::from(b'\0'),
        glib::OptionFlags::HIDDEN,
        glib::OptionArg::Filename,
        "Development: show an image or .annota file in the canvas preview",
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
    // not in the picture (spec app-shell: Single application instance).
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

    #[cfg(debug_assertions)]
    if let Some(path) = options
        .lookup::<std::path::PathBuf>("dev-canvas")
        .ok()
        .flatten()
    {
        // Relative to the launching process' directory, also when forwarded.
        if let Some(path) = cmdline.create_file_for_arg(path.as_os_str()).path() {
            crate::ui::preview::open(app, &path);
        }
        return 0;
    }

    let window = present_window(app);
    if options.contains("open") {
        show_toast(
            &window,
            &adw::Toast::new(&gettext("Opening files arrives in milestone 0.1")),
        );
    }
    0
}

/// Raises the main window, creating it on first use.
pub fn present_window(app: &adw::Application) -> adw::ApplicationWindow {
    let existing = app
        .windows()
        .into_iter()
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
    setup_drop_target(app, &window, &builder);
    window.present();
    window
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

/// Drop a PNG/JPG/WebP onto the window to open it as a new capture (mockup 14).
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
    app.add_action_entries([quit, about, prefs, capture, paste, open]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);
    app.set_accels_for_action("app.paste-image", &["<Shift><Control>v"]);
    app.set_accels_for_action("app.open-image", &["<Control>o"]);
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
