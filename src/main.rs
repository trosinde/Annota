mod application;
mod cli;
mod config;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};
use gtk::{gio, glib};

fn main() -> glib::ExitCode {
    // Translations: failures only mean English UI, never a crash.
    let _ = bindtextdomain(config::GETTEXT_PACKAGE, config::LOCALEDIR);
    let _ = bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8");
    let _ = textdomain(config::GETTEXT_PACKAGE);

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
