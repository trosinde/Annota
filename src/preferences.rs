//! Preferences dialog (capability `app-shell`), persisted via GSettings
//! schema `io.github.trosinde.Annota`.

use std::path::{Path, PathBuf};

use adw::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use crate::config;

const UI_PREFERENCES: &str = "/io/github/trosinde/Annota/ui/preferences.ui";

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let builder = gtk::Builder::from_resource(UI_PREFERENCES);
    let dialog: adw::PreferencesDialog = builder.object("preferences_dialog").unwrap();
    let settings = gio::Settings::new(config::APP_ID);

    bind_saving(&builder, &settings);
    bind_export(&builder, &settings);
    bind_capture(&builder, &settings);

    dialog.present(Some(parent));
}

fn bind_saving(builder: &gtk::Builder, settings: &gio::Settings) {
    let save_folder_row: adw::ActionRow = builder.object("save_folder_row").unwrap();
    let save_folder_button: gtk::Button = builder.object("save_folder_button").unwrap();
    let filename_pattern_row: adw::EntryRow = builder.object("filename_pattern_row").unwrap();
    let filename_example_row: adw::ActionRow = builder.object("filename_example_row").unwrap();

    update_folder_subtitle(&save_folder_row, &settings.string("save-folder"));
    settings
        .bind("filename-pattern", &filename_pattern_row, "text")
        .build();

    update_filename_example(&filename_example_row, &filename_pattern_row.text());
    filename_pattern_row.connect_notify_local(
        Some("text"),
        glib::clone!(
            #[weak]
            filename_example_row,
            move |entry, _| update_filename_example(&filename_example_row, &entry.text())
        ),
    );

    save_folder_button.connect_clicked(glib::clone!(
        #[weak]
        save_folder_row,
        #[strong]
        settings,
        move |button| pick_folder(button, settings.clone(), "save-folder", save_folder_row)
    ));
}

fn bind_export(builder: &gtk::Builder, settings: &gio::Settings) {
    let default_format_row: adw::ComboRow = builder.object("default_format_row").unwrap();
    default_format_row.set_selected(settings.enum_("default-export-format") as u32);
    default_format_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            let _ = settings.set_enum("default-export-format", row.selected() as i32);
        }
    ));

    let export_folder_row: adw::ActionRow = builder.object("export_folder_row").unwrap();
    let export_folder_button: gtk::Button = builder.object("export_folder_button").unwrap();
    update_folder_subtitle(&export_folder_row, &settings.string("export-folder"));
    export_folder_button.connect_clicked(glib::clone!(
        #[weak]
        export_folder_row,
        #[strong]
        settings,
        move |button| pick_folder(button, settings.clone(), "export-folder", export_folder_row)
    ));
}

fn bind_capture(builder: &gtk::Builder, settings: &gio::Settings) {
    let after_capture_editor: gtk::CheckButton = builder.object("after_capture_editor").unwrap();
    let after_capture_clipboard: gtk::CheckButton =
        builder.object("after_capture_clipboard").unwrap();
    let after_capture_both: gtk::CheckButton = builder.object("after_capture_both").unwrap();

    match settings.enum_("after-capture") {
        1 => after_capture_clipboard.set_active(true),
        2 => after_capture_both.set_active(true),
        _ => after_capture_editor.set_active(true),
    }
    for (radio, value) in [
        (&after_capture_editor, 0),
        (&after_capture_clipboard, 1),
        (&after_capture_both, 2),
    ] {
        radio.connect_toggled(glib::clone!(
            #[strong]
            settings,
            move |radio| {
                if radio.is_active() {
                    let _ = settings.set_enum("after-capture", value);
                }
            }
        ));
    }

    let capture_mode_row: adw::ComboRow = builder.object("capture_mode_row").unwrap();
    capture_mode_row.set_selected(settings.enum_("capture-mode") as u32);
    capture_mode_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            let _ = settings.set_enum("capture-mode", row.selected() as i32);
        }
    ));

    let capture_delay_row: adw::SpinRow = builder.object("capture_delay_row").unwrap();
    settings
        .bind("capture-delay", &capture_delay_row, "value")
        .build();

    let include_pointer_row: adw::SwitchRow = builder.object("include_pointer_row").unwrap();
    settings
        .bind("include-pointer", &include_pointer_row, "active")
        .build();
}

fn pick_folder(
    button: &gtk::Button,
    settings: gio::Settings,
    key: &'static str,
    row: adw::ActionRow,
) {
    let parent = button.root().and_downcast::<gtk::Window>();
    glib::spawn_future_local(async move {
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Choose Folder"))
            .modal(true)
            .build();
        if let Ok(folder) = dialog.select_folder_future(parent.as_ref()).await {
            if let Some(path) = folder.path() {
                let _ = settings.set_string(key, &path.to_string_lossy());
                update_folder_subtitle(&row, &path.to_string_lossy());
            }
        }
    });
}

fn resolved_folder(value: &str) -> PathBuf {
    if value.is_empty() {
        let mut path =
            glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(glib::home_dir);
        path.push("Annota");
        path
    } else {
        PathBuf::from(value)
    }
}

fn display_path(path: &Path) -> String {
    let home = glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if rest != Path::new("") => format!("~/{}", rest.display()),
        Ok(_) => "~".to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn update_folder_subtitle(row: &adw::ActionRow, raw: &str) {
    row.set_subtitle(&display_path(&resolved_folder(raw)));
}

fn update_filename_example(row: &adw::ActionRow, pattern: &str) {
    let now = glib::DateTime::now_local().expect("local time is always available");
    let stem = now
        .format(pattern)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| pattern.to_string());
    row.set_subtitle(&format!("{stem}.png"));
}
