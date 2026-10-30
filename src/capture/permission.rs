//! "Screen Access Denied" dialog (spec: Portal permission handling; mockup 08).

use adw::prelude::*;
use gtk::{gio, glib};

use crate::{application, config};

const UI_PERMISSION: &str = "/io/github/trosinde/Annota/ui/permission-dialog.ui";

/// The GNOME Settings page where screenshot access is granted to Annota.
pub fn settings_command() -> String {
    format!("gnome-control-center applications {}", config::APP_ID)
}

pub fn present(app: &adw::Application) {
    let window = application::present_window(app);
    let builder = gtk::Builder::from_resource(UI_PERMISSION);
    let (Some(dialog), Some(close), Some(open)) = (
        builder.object::<adw::Dialog>("permission_dialog"),
        builder.object::<gtk::Button>("close_button"),
        builder.object::<gtk::Button>("open_settings_button"),
    ) else {
        eprintln!("annota: permission-dialog.ui is incomplete");
        return;
    };
    close.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    open.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            open_settings();
            dialog.close();
        }
    ));
    dialog.set_default_widget(Some(&open));
    dialog.set_focus(Some(&open));
    dialog.present(Some(&window));
}

fn open_settings() {
    let launched = gio::AppInfo::create_from_commandline(
        settings_command(),
        None,
        gio::AppInfoCreateFlags::NONE,
    )
    .and_then(|info| info.launch(&[], gio::AppLaunchContext::NONE));
    if let Err(err) = launched {
        eprintln!("annota: cannot open GNOME Settings: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_annota_app_settings() {
        assert_eq!(
            settings_command(),
            "gnome-control-center applications io.github.trosinde.Annota"
        );
    }
}
