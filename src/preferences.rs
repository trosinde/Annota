//! Preferences dialog (capability `app-shell`), persisted via GSettings
//! schema `io.github.trosinde.Annota`.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::gio;
use gtk::glib;

use crate::capture::{shortcut_dialog, shortcuts};
use crate::cli::CaptureMode;
use crate::config;

const UI_PREFERENCES: &str = "/io/github/trosinde/Annota/ui/preferences.ui";

/// Looks up a builder object, or logs and bails out of the enclosing `()`-returning function.
/// `preferences.ui` is a compiled-in resource built from `data/ui/preferences.blp` in lock-step
/// with this module, so a miss here means the two drifted apart, not a user-facing error —
/// but it must never panic (see CLAUDE.md §7: no `unwrap()` outside tests).
macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(widget) => widget,
            None => {
                eprintln!("annota: preferences.ui is missing object '{}'", $name);
                return;
            }
        }
    };
}

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let builder = gtk::Builder::from_resource(UI_PREFERENCES);
    let dialog: adw::PreferencesDialog = obj!(builder, "preferences_dialog");
    let settings = gio::Settings::new(config::APP_ID);

    bind_saving(&builder, &settings);
    bind_export(&builder, &settings);
    bind_capture(&builder, &settings, &dialog);

    dialog.present(Some(parent));
}

fn bind_saving(builder: &gtk::Builder, settings: &gio::Settings) {
    let save_folder_row: adw::ActionRow = obj!(builder, "save_folder_row");
    let save_folder_button: gtk::Button = obj!(builder, "save_folder_button");
    let filename_pattern_row: adw::EntryRow = obj!(builder, "filename_pattern_row");
    let filename_example_row: adw::ActionRow = obj!(builder, "filename_example_row");

    update_folder_subtitle(
        &save_folder_row,
        &resolved_save_folder(&settings.string("save-folder")),
    );
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
    let default_format_row: adw::ComboRow = obj!(builder, "default_format_row");
    default_format_row.set_selected(settings.enum_("default-export-format") as u32);
    default_format_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            let _ = settings.set_enum("default-export-format", row.selected() as i32);
        }
    ));

    let export_folder_row: adw::ActionRow = obj!(builder, "export_folder_row");
    let export_folder_button: gtk::Button = obj!(builder, "export_folder_button");
    update_folder_subtitle(
        &export_folder_row,
        &resolved_export_folder(&settings.string("export-folder")),
    );
    export_folder_button.connect_clicked(glib::clone!(
        #[weak]
        export_folder_row,
        #[strong]
        settings,
        move |button| pick_folder(button, settings.clone(), "export-folder", export_folder_row)
    ));
}

fn bind_capture(builder: &gtk::Builder, settings: &gio::Settings, dialog: &adw::PreferencesDialog) {
    let include_pointer_row: adw::SwitchRow = obj!(builder, "include_pointer_row");
    settings
        .bind("include-pointer", &include_pointer_row, "active")
        .build();

    let capture_delay_row: adw::SpinRow = obj!(builder, "capture_delay_row");
    settings
        .bind("capture-delay", &capture_delay_row, "value")
        .build();
    capture_delay_row.connect_output(|row| {
        row.set_text(&seconds_text(row.value() as i32));
        true
    });

    let all_monitors_row: adw::SwitchRow = obj!(builder, "capture_all_monitors_row");
    settings
        .bind("capture-all-monitors", &all_monitors_row, "active")
        .build();

    let after_capture_row: adw::ComboRow = obj!(builder, "after_capture_row");
    after_capture_row.set_selected(settings.enum_("after-capture").max(0) as u32);
    after_capture_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        move |row| {
            let _ = settings.set_enum("after-capture", row.selected() as i32);
        }
    ));

    let page: adw::PreferencesPage = obj!(builder, "capture_page");
    let banner_group: adw::PreferencesGroup = obj!(builder, "banner_group");
    let banner: adw::Banner = obj!(builder, "shortcuts_banner");
    let group: adw::PreferencesGroup = obj!(builder, "shortcuts_group");
    let rows = [
        (
            CaptureMode::Region,
            obj!(builder, "shortcut_region_row"),
            obj!(builder, "shortcut_region_label"),
        ),
        (
            CaptureMode::Window,
            obj!(builder, "shortcut_window_row"),
            obj!(builder, "shortcut_window_label"),
        ),
        (
            CaptureMode::Screen,
            obj!(builder, "shortcut_screen_row"),
            obj!(builder, "shortcut_screen_label"),
        ),
    ];
    // AdwPreferencesPage:banner exists from libadwaita 1.7 (Ubuntu 26.04); older versions show
    // the banner at the top of the page's content instead.
    let banner_in_group = page.find_property("banner").is_none();
    if banner_in_group {
        banner_group.add(&banner);
    } else {
        page.set_property("banner", &banner);
    }

    let view = Rc::new(ShortcutsView {
        settings: settings.clone(),
        dialog: dialog.downgrade(),
        banner,
        banner_group,
        banner_in_group,
        group,
        rows,
        portal: Cell::new(shortcuts::portal_active().then_some(true)),
    });
    view.refresh();
    if view.portal.get().is_none() {
        let view = view.clone();
        glib::spawn_future_local(async move {
            let available = shortcuts::portal_available().await;
            view.portal.set(Some(available));
            view.refresh();
        });
    }

    view.banner.connect_button_clicked(glib::clone!(
        #[strong]
        view,
        move |_| view.create_gnome_shortcuts()
    ));
    for (mode, row, _) in &view.rows {
        let mode = *mode;
        row.connect_activated(glib::clone!(
            #[strong]
            view,
            move |_| {
                let Some(dialog) = view.dialog.upgrade() else {
                    return;
                };
                let refresh_view = view.clone();
                shortcut_dialog::present(
                    &dialog,
                    &view.settings,
                    mode,
                    move |result| match result {
                        Ok(()) => refresh_view.refresh(),
                        Err(err) => refresh_view.toast(
                            &gettext("Could not change the shortcut: {error}")
                                .replace("{error}", &err.to_string()),
                        ),
                    },
                );
            }
        ));
    }
    settings.connect_changed(
        None,
        glib::clone!(
            #[weak]
            view,
            move |_, key| {
                if key.starts_with("shortcut-") || key == "gnome-shortcuts-created" {
                    view.refresh();
                }
            }
        ),
    );
}

/// "5 s" for spin rows.
fn seconds_text(seconds: i32) -> String {
    gettext("{seconds} s").replace("{seconds}", &seconds.to_string())
}

/// The Global Shortcuts group (mockups 09, 10).
struct ShortcutsView {
    settings: gio::Settings,
    dialog: glib::WeakRef<adw::PreferencesDialog>,
    banner: adw::Banner,
    banner_group: adw::PreferencesGroup,
    /// libadwaita < 1.7: the banner lives in `banner_group`.
    banner_in_group: bool,
    group: adw::PreferencesGroup,
    rows: [(CaptureMode, adw::ActionRow, gtk::ShortcutLabel); 3],
    /// Whether the GlobalShortcuts portal exists; `None` until known.
    portal: Cell<Option<bool>>,
}

impl ShortcutsView {
    fn refresh(&self) {
        let created = self.settings.boolean("gnome-shortcuts-created");
        let portal = self.portal.get();
        let fallback = !created && portal == Some(false);
        self.banner.set_revealed(fallback);
        self.banner_group
            .set_visible(fallback && self.banner_in_group);
        let description = if fallback {
            gettext("This system has no global-shortcuts portal. Annota adds GNOME keyboard shortcuts instead.")
        } else if created {
            gettext("Work in every app, even when Annota is closed")
        } else if portal == Some(true) {
            gettext("Work in every app while Annota is running")
        } else {
            String::new()
        };
        self.group.set_description(
            Some(&description)
                .filter(|d| !d.is_empty())
                .map(String::as_str),
        );

        for (mode, row, label) in &self.rows {
            let accel = self.settings.string(shortcuts::settings_key(*mode));
            label.set_accelerator(&accel);
            let subtitle = if accel.is_empty() {
                glib::markup_escape_text(&gettext("Disabled")).to_string()
            } else if created {
                let command = glib::markup_escape_text(&shortcuts::keybinding_command(*mode));
                glib::markup_escape_text(&gettext("GNOME shortcut · runs {command}"))
                    .replace("{command}", &format!("<tt>{command}</tt>"))
            } else if portal == Some(true) {
                glib::markup_escape_text(&gettext("System shortcut")).to_string()
            } else {
                glib::markup_escape_text(&gettext("Not active yet")).to_string()
            };
            row.set_subtitle(&subtitle);
        }
    }

    fn toast(&self, text: &str) {
        if let Some(dialog) = self.dialog.upgrade() {
            let toast = adw::Toast::new(text);
            toast.set_use_markup(false);
            dialog.add_toast(toast);
        }
    }

    /// Banner button "Create GNOME Shortcuts" (mockup 09 → 10).
    fn create_gnome_shortcuts(&self) {
        match shortcuts::create_gnome_shortcuts(&self.settings) {
            Ok(count) => {
                self.refresh();
                let Some(dialog) = self.dialog.upgrade() else {
                    return;
                };
                let text = ngettext(
                    "{count} GNOME shortcut created",
                    "{count} GNOME shortcuts created",
                    count as u32,
                )
                .replace("{count}", &count.to_string());
                let toast = adw::Toast::builder()
                    .title(text)
                    .button_label(gettext("Open Keyboard Settings"))
                    .build();
                toast.connect_button_clicked(|_| open_keyboard_settings());
                dialog.add_toast(toast);
            }
            Err(err) => self.toast(
                &gettext("Could not create GNOME shortcuts: {error}")
                    .replace("{error}", &err.to_string()),
            ),
        }
    }
}

fn open_keyboard_settings() {
    let launched = gio::AppInfo::create_from_commandline(
        "gnome-control-center keyboard",
        None,
        gio::AppInfoCreateFlags::NONE,
    )
    .and_then(|info| info.launch(&[], gio::AppLaunchContext::NONE));
    if let Err(err) = launched {
        eprintln!("annota: cannot open GNOME keyboard settings: {err}");
    }
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
                row.set_subtitle(&display_path(&path));
            }
        }
    });
}

/// The save folder setting with its default applied: empty means `~/Pictures/Annota`.
pub(crate) fn resolved_save_folder(value: &str) -> PathBuf {
    if value.is_empty() {
        let mut path =
            glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(glib::home_dir);
        path.push("Annota");
        path
    } else {
        PathBuf::from(value)
    }
}

/// The export folder setting with its default applied: empty means the save folder's
/// `Exports` subfolder.
pub(crate) fn resolved_export_folder(value: &str) -> PathBuf {
    if value.is_empty() {
        let mut path = resolved_save_folder("");
        path.push("Exports");
        path
    } else {
        PathBuf::from(value)
    }
}

/// `path` as shown in folder rows: `~/…` inside the home folder.
pub(crate) fn display_path(path: &Path) -> String {
    let home = glib::home_dir();
    match path.strip_prefix(&home) {
        Ok(rest) if rest != Path::new("") => format!("~/{}", rest.display()),
        Ok(_) => "~".to_string(),
        Err(_) => path.display().to_string(),
    }
}

fn update_folder_subtitle(row: &adw::ActionRow, resolved: &Path) {
    row.set_subtitle(&display_path(resolved));
}

/// Expands the `filename-pattern` setting (strftime-style) for `now`; an invalid pattern is
/// used literally.
pub(crate) fn expand_filename_pattern(pattern: &str, now: &glib::DateTime) -> String {
    now.format(pattern)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| pattern.to_string())
}

fn update_filename_example(row: &adw::ActionRow, pattern: &str) {
    let Ok(now) = glib::DateTime::now_local() else {
        return;
    };
    let stem = expand_filename_pattern(pattern, &now);
    row.set_subtitle(&format!("{stem}.png"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_pattern_expands() {
        let time = glib::DateTime::from_utc(2026, 10, 30, 14, 22, 1.0).unwrap();
        assert_eq!(
            expand_filename_pattern("Annota %Y-%m-%d %H-%M-%S", &time),
            "Annota 2026-10-30 14-22-01"
        );
    }

    #[test]
    fn empty_save_folder_defaults_to_pictures_annota() {
        assert!(resolved_save_folder("").ends_with("Annota"));
        assert_eq!(
            resolved_save_folder("/tmp/shots"),
            PathBuf::from("/tmp/shots")
        );
    }
}
