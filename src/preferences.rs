//! Preferences dialog (capability `app-shell`), persisted via GSettings
//! schema `io.github.trosinde.Annota`.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::gio;
use gtk::glib;

use crate::capture::shortcuts::GlobalShortcut;
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
    let open_settings: gtk::Button = obj!(builder, "open_gnome_settings_button");
    macro_rules! row {
        ($mode:expr, $name:literal) => {
            ShortcutRow {
                mode: $mode,
                row: obj!(builder, concat!("shortcut_", $name, "_row")),
                keys: obj!(builder, concat!("shortcut_", $name, "_label")),
                text: obj!(builder, concat!("shortcut_", $name, "_text")),
                warning: obj!(builder, concat!("shortcut_", $name, "_warning")),
                chevron: obj!(builder, concat!("shortcut_", $name, "_chevron")),
            }
        };
    }
    let rows = [
        row!(GlobalShortcut::Capture(CaptureMode::Region), "region"),
        row!(GlobalShortcut::Capture(CaptureMode::Window), "window"),
        row!(GlobalShortcut::Capture(CaptureMode::Screen), "screen"),
        // add-screen-recording mockup 14: "Record video" after "Screen capture".
        row!(GlobalShortcut::Record, "record"),
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
        open_settings,
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
    view.open_settings.connect_clicked(open_gnome_settings);
    // D2: Preferences › Capture reads the portal's triggers again whenever it is shown.
    page.connect_map(|_| shortcuts::refresh_bound());
    shortcuts::connect_bound_changed(glib::clone!(
        #[weak]
        view,
        #[upgrade_or]
        glib::ControlFlow::Break,
        move || {
            view.refresh();
            glib::ControlFlow::Continue
        }
    ));
    for shortcut in &view.rows {
        let mode = shortcut.mode;
        shortcut.row.connect_activated(glib::clone!(
            #[strong]
            view,
            move |_| {
                // Portal path: GNOME owns the keys, so there is nothing to edit here (D4).
                if view.portal_path() {
                    return;
                }
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

/// One row of the Global Shortcuts group.
struct ShortcutRow {
    mode: GlobalShortcut,
    row: adw::ActionRow,
    keys: gtk::ShortcutLabel,
    /// "Not set" or a trigger the portal described in words (fix-portal-shortcut-bindings D3).
    text: gtk::Label,
    /// A trigger GNOME's own keybinding holds (D7, mockup 06a).
    warning: gtk::Image,
    chevron: gtk::Image,
}

/// The Global Shortcuts group (add-screen-capture mockups 09, 10; fix-portal-shortcut-bindings
/// 02, 03, 06a).
struct ShortcutsView {
    settings: gio::Settings,
    dialog: glib::WeakRef<adw::PreferencesDialog>,
    banner: adw::Banner,
    banner_group: adw::PreferencesGroup,
    /// libadwaita < 1.7: the banner lives in `banner_group`.
    banner_in_group: bool,
    group: adw::PreferencesGroup,
    open_settings: gtk::Button,
    rows: [ShortcutRow; 4],
    /// Whether the GlobalShortcuts portal exists; `None` until known.
    portal: Cell<Option<bool>>,
}

impl ShortcutsView {
    /// The GlobalShortcuts portal binds the keys, so GNOME owns them. Also before the portal has
    /// been asked: until it answers, no configured accelerator is shown.
    fn portal_path(&self) -> bool {
        !self.settings.boolean("gnome-shortcuts-created") && self.portal.get() != Some(false)
    }

    fn refresh(&self) {
        let created = self.settings.boolean("gnome-shortcuts-created");
        let portal = self.portal.get();
        let fallback = !created && portal == Some(false);
        self.banner.set_revealed(fallback);
        self.banner_group
            .set_visible(fallback && self.banner_in_group);
        let portal_path = self.portal_path();
        let description = if fallback {
            gettext("This system has no global-shortcuts portal. Annota adds GNOME keyboard shortcuts instead.")
        } else if created {
            gettext("Work in every app, even when Annota is closed")
        } else if portal == Some(true) {
            let managed = gettext("GNOME manages these shortcuts. Change them in GNOME Settings.");
            if shortcuts::unreported() {
                // D8: one more line in the description's own style, naming no cause.
                format!(
                    "{managed}\n{}",
                    gettext("GNOME has not reported these shortcuts.")
                )
            } else {
                managed
            }
        } else {
            String::new()
        };
        self.group.set_description(
            Some(&description)
                .filter(|d| !d.is_empty())
                .map(String::as_str),
        );
        self.open_settings
            .set_visible(portal == Some(true) && !created && gnome_settings_installed());

        // One read of GNOME's keybindings for all rows (reviewer N8).
        let shown = portal_path.then(shortcuts::shown_triggers);
        for shortcut in &self.rows {
            match shown
                .as_ref()
                .and_then(|shown| shown.iter().find(|(mode, _)| *mode == shortcut.mode))
            {
                Some((_, shown)) => self.show_portal_row(shortcut, shown),
                None => self.show_fallback_row(shortcut, created, portal),
            }
        }
    }

    /// Mockups 02, 03a, 03b, 06a: read-only, the trigger GNOME reports.
    fn show_portal_row(&self, shortcut: &ShortcutRow, shown: &shortcuts::Shown) {
        shortcut.row.set_activatable(false);
        shortcut.chevron.set_visible(false);
        let (accel, text) = match &shown.trigger {
            shortcuts::ShownTrigger::Accel(accel) => (accel.clone(), None),
            shortcuts::ShownTrigger::Text(text) => (String::new(), Some(text.clone())),
            shortcuts::ShownTrigger::NotSet => (String::new(), Some(gettext("Not set"))),
            shortcuts::ShownTrigger::Pending => (String::new(), None),
        };
        shortcut.keys.set_accelerator(&accel);
        shortcut.keys.set_visible(!accel.is_empty());
        shortcut.text.set_text(text.as_deref().unwrap_or_default());
        shortcut.text.set_visible(text.is_some());
        let taken = shown.taken_by.as_ref().map(shortcuts::TakenBy::text);
        shortcut.warning.set_visible(taken.is_some());
        shortcut.row.set_subtitle(&glib::markup_escape_text(
            taken.as_deref().unwrap_or_default(),
        ));
        // Read with the row: "Screen capture, Shift+Print, Used by GNOME: Take a screenshot".
        let trigger = match &shown.trigger {
            shortcuts::ShownTrigger::Accel(accel) => crate::application::accel_label(accel),
            _ => text.unwrap_or_default(),
        };
        let spoken = row_description(&trigger, taken.as_deref());
        if spoken.is_empty() {
            shortcut
                .row
                .reset_property(gtk::AccessibleProperty::Description);
        } else {
            shortcut
                .row
                .update_property(&[gtk::accessible::Property::Description(&spoken)]);
        }
    }

    /// GNOME custom keybindings (mockups 09, 10): the configured accelerators, editable.
    fn show_fallback_row(&self, shortcut: &ShortcutRow, created: bool, portal: Option<bool>) {
        let accel = self.settings.string(shortcuts::settings_key(shortcut.mode));
        shortcut.row.set_activatable(true);
        shortcut.chevron.set_visible(true);
        shortcut.keys.set_accelerator(&accel);
        shortcut.keys.set_visible(true);
        shortcut.text.set_visible(false);
        shortcut.warning.set_visible(false);
        shortcut
            .row
            .reset_property(gtk::AccessibleProperty::Description);
        let subtitle = if accel.is_empty() {
            glib::markup_escape_text(&gettext("Disabled")).to_string()
        } else if created {
            let command = glib::markup_escape_text(&shortcuts::keybinding_command(shortcut.mode));
            glib::markup_escape_text(&gettext("GNOME shortcut · runs {command}"))
                .replace("{command}", &format!("<tt>{command}</tt>"))
        } else if portal == Some(true) {
            glib::markup_escape_text(&gettext("System shortcut")).to_string()
        } else {
            glib::markup_escape_text(&gettext("Not active yet")).to_string()
        };
        shortcut.row.set_subtitle(&subtitle);
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

/// A portal-path row's accessible description: the trigger as read aloud, then the "Used by
/// GNOME" warning when GNOME holds it (reviewer N5), so the warning is not only visible.
pub(crate) fn row_description(trigger: &str, taken: Option<&str>) -> String {
    [Some(trigger), taken]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// GNOME Settings' desktop file; "Open GNOME Settings" is hidden without it (design.md D4).
const GNOME_SETTINGS_APP: &str = "org.gnome.Settings.desktop";

fn gnome_settings_installed() -> bool {
    gio::DesktopAppInfo::new(GNOME_SETTINGS_APP).is_some()
}

/// The `org.gnome.Settings` `launch-panel` call that opens Apps › Annota, where GNOME lists
/// Annota's global shortcuts (design.md D4, measured with gnome-control-center 50.3).
pub(crate) fn launch_panel_parameters(activation_token: Option<&str>) -> glib::Variant {
    let target = ("applications", vec![config::APP_ID.to_variant()]).to_variant();
    let platform_data = glib::VariantDict::new(None);
    if let Some(token) = activation_token {
        platform_data.insert("activation-token", token);
        platform_data.insert("desktop-startup-id", token);
    }
    // `(sava{sv})`: a tuple of `Variant`s would box each one as `v` again.
    glib::Variant::tuple_from_iter([
        "launch-panel".to_variant(),
        vec![target].to_variant(),
        platform_data.end(),
    ])
}

/// "Open GNOME Settings": GNOME Settings at Apps › Annota. A D-Bus call on this computer; no
/// network.
fn open_gnome_settings(button: &gtk::Button) {
    let token = gio::DesktopAppInfo::new(GNOME_SETTINGS_APP).and_then(|info| {
        button
            .display()
            .app_launch_context()
            .startup_notify_id(Some(&info), &[])
            .map(|t| t.to_string())
    });
    let parameters = launch_panel_parameters(token.as_deref());
    glib::spawn_future_local(async move {
        let result = match gio::bus_get_future(gio::BusType::Session).await {
            Ok(connection) => connection
                .call_future(
                    Some("org.gnome.Settings"),
                    "/org/gnome/Settings",
                    "org.freedesktop.Application",
                    "ActivateAction",
                    Some(&parameters),
                    None,
                    gio::DBusCallFlags::NONE,
                    10_000,
                )
                .await
                .map(|_| ()),
            Err(err) => Err(err),
        };
        if let Err(err) = result {
            eprintln!("annota: cannot open GNOME Settings: {err}");
        }
    });
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

    // spec: capture / Global capture hotkey / Trigger taken by GNOME
    #[test]
    fn a_taken_rows_description_names_the_gnome_action() {
        assert_eq!(
            row_description("Shift+Print", Some("Used by GNOME: Take a screenshot")),
            "Shift+Print, Used by GNOME: Take a screenshot"
        );
        assert_eq!(row_description("Shift+Ctrl+P", None), "Shift+Ctrl+P");
        assert_eq!(row_description("Not set", None), "Not set");
        assert_eq!(row_description("", None), "");
    }

    // spec: capture / Global capture hotkey / Open GNOME Settings
    #[test]
    fn open_gnome_settings_targets_annotas_apps_page() {
        // `gdbus call ... ActivateAction` form of `gnome-control-center applications <app-id>`.
        assert_eq!(
            launch_panel_parameters(None).print(true).as_str(),
            "('launch-panel', [<('applications', [<'io.github.trosinde.Annota'>])>], @a{sv} {})"
        );
        let with_token = launch_panel_parameters(Some("t-1"));
        assert_eq!(with_token.type_().as_str(), "(sava{sv})");
        assert!(with_token
            .print(false)
            .contains("'activation-token': <'t-1'>"));
    }
}
