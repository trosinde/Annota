//! Application lifecycle: single instance, command line, main window, app actions.

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

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
    app.connect_startup(setup_actions);

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
    let window = present_window(app);

    let request = if options.contains("capture") {
        Some(gettext("Capturing arrives in milestone 0.1"))
    } else if options.contains("open") {
        Some(gettext("Opening files arrives in milestone 0.1"))
    } else {
        None
    };
    if let Some(text) = request {
        show_toast(&window, &text);
    }
    0
}

fn present_window(app: &adw::Application) -> adw::ApplicationWindow {
    if let Some(window) = app.active_window() {
        if let Ok(window) = window.downcast::<adw::ApplicationWindow>() {
            window.present();
            return window;
        }
    }
    let builder = gtk::Builder::from_resource(UI_WINDOW);
    let window: adw::ApplicationWindow = builder
        .object("window")
        .unwrap_or_else(|| adw::ApplicationWindow::new(app));
    window.set_application(Some(app));
    window.present();
    window
}

fn show_toast(window: &adw::ApplicationWindow, text: &str) {
    if let Some(overlay) = window.content().and_then(|c| find_toast_overlay(&c)) {
        overlay.add_toast(adw::Toast::new(text));
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
    app.add_action_entries([quit, about, prefs]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);
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
        .license_type(gtk::License::Gpl30)
        .copyright("© 2026 The Annota Contributors")
        .comments(gettext(
            "Works entirely on this computer. No account, no tracking.",
        ))
        .build();
    about.present(app.active_window().as_ref());
}
