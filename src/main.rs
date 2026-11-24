mod application;
mod capture;
mod cli;
mod config;
mod preferences;
mod recording;
mod schema_check;
mod ui;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain, LocaleCategory};
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    // Translations: failures only mean English UI, never a crash. The locale is set here, not
    // only by GTK later, so the schema check below already speaks the user's language.
    let _ = setlocale(LocaleCategory::LcAll, "");
    let _ = bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR);
    let _ = bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = textdomain(config::GETTEXT_PACKAGE);

    // A stale or missing GSettings schema would abort the process inside GLib at the first
    // `Settings::get`; say what is wrong instead (fix-stale-settings-schema). `--version` and
    // `--help` read no settings and stay available on a broken installation.
    if !schema_check::skips_check(std::env::args_os()) {
        if let Err(message) = schema_check::check_process(config::APP_ID) {
            eprint!("{message}");
            return glib::ExitCode::FAILURE;
        }
    }

    let resource_path = format!("{}/resources.gresource", config::PKGDATADIR);
    match gio::Resource::load(&resource_path) {
        Ok(resources) => gio::resources_register(&resources),
        Err(err) => {
            eprintln!("annota: cannot load {resource_path}: {err}");
            return glib::ExitCode::FAILURE;
        }
    }

    glib::set_application_name("Annota");
    application::run()
}
