//! Set Shortcut dialog (mockup 11): press a new key combination for one capture mode.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use super::shortcuts::{self, Conflict, ShortcutError};
use crate::cli::CaptureMode;

const UI_SHORTCUT_DIALOG: &str = "/io/github/trosinde/Annota/ui/shortcut-dialog.ui";

/// What a key press in the dialog means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Press {
    /// Esc: close without changes.
    Cancel,
    /// Backspace: disable the shortcut.
    Disable,
    /// Enter: confirm the pressed shortcut (same as the Set button).
    Confirm,
    /// Tab: keyboard navigation between the dialog's buttons.
    Navigate,
    /// A modifier on its own, nothing to record yet.
    Ignore,
    /// A complete accelerator in GTK syntax.
    Accel(String),
}

fn is_modifier(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Hyper_L
            | gdk::Key::Hyper_R
            | gdk::Key::ISO_Level3_Shift
            | gdk::Key::Caps_Lock
            | gdk::Key::Num_Lock
    )
}

fn interpret(key: gdk::Key, state: gdk::ModifierType) -> Press {
    let mods = state & gtk::accelerator_get_default_mod_mask();
    if mods.is_empty() {
        match key {
            gdk::Key::Escape => return Press::Cancel,
            gdk::Key::BackSpace => return Press::Disable,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => return Press::Confirm,
            gdk::Key::Tab | gdk::Key::ISO_Left_Tab => return Press::Navigate,
            _ => {}
        }
    }
    if is_modifier(key) {
        return Press::Ignore;
    }
    // Alt+Print arrives as SysRq on many keyboards; GNOME calls it Alt+Print.
    let key = if key == gdk::Key::Sys_Req {
        gdk::Key::Print
    } else {
        key.to_lower()
    };
    Press::Accel(gtk::accelerator_name(key, mods).to_string())
}

/// Opens the dialog over `parent`. `done` is called after a change was applied.
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    settings: &gio::Settings,
    mode: CaptureMode,
    done: impl Fn(Result<(), ShortcutError>) + 'static,
) {
    let builder = gtk::Builder::from_resource(UI_SHORTCUT_DIALOG);
    let (
        Some(dialog),
        Some(cancel),
        Some(set),
        Some(prompt),
        Some(keys),
        Some(status_box),
        Some(status_icon),
        Some(status_label),
    ) = (
        builder.object::<adw::Dialog>("shortcut_dialog"),
        builder.object::<gtk::Button>("cancel_button"),
        builder.object::<gtk::Button>("set_button"),
        builder.object::<gtk::Label>("prompt_label"),
        builder.object::<gtk::ShortcutLabel>("live_keys"),
        builder.object::<gtk::Box>("status_box"),
        builder.object::<gtk::Image>("status_icon"),
        builder.object::<gtk::Label>("status_label"),
    )
    else {
        eprintln!("annota: shortcut-dialog.ui is incomplete");
        return;
    };

    let title = glib::markup_escape_text(&shortcuts::mode_title(mode));
    prompt.set_markup(
        &glib::markup_escape_text(&gettext("Press the new shortcut for {name}"))
            .replace("{name}", &format!("<b>{title}</b>")),
    );
    keys.set_accelerator(&settings.string(shortcuts::settings_key(mode)));

    let pending: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let done = Rc::new(done);
    let settings = settings.clone();

    let apply = Rc::new(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        settings,
        #[strong]
        done,
        move |accel: &str| {
            let result = shortcuts::change_shortcut(&settings, mode, accel);
            dialog.close();
            done(result);
        }
    ));

    let show_status = glib::clone!(
        #[weak]
        status_box,
        #[weak]
        status_icon,
        #[weak]
        status_label,
        move |conflict: Option<&Conflict>| {
            for class in ["success", "warning", "error"] {
                status_box.remove_css_class(class);
            }
            let (icon, text, class) = match conflict {
                None => (
                    "object-select-symbolic",
                    gettext("Not used by GNOME or another app"),
                    "success",
                ),
                Some(c) if c.takeover => (
                    "dialog-information-symbolic",
                    gettext("Taken over from GNOME: {name}").replace("{name}", &c.name),
                    "warning",
                ),
                Some(c) => (
                    "dialog-warning-symbolic",
                    gettext("Used by {name}").replace("{name}", &c.name),
                    "error",
                ),
            };
            status_icon.set_icon_name(Some(icon));
            status_label.set_text(&text);
            status_box.add_css_class(class);
            status_box.set_visible(true);
        }
    );

    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    controller.connect_key_pressed(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        keys,
        #[weak]
        set,
        #[strong]
        pending,
        #[strong]
        apply,
        #[strong]
        settings,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, state| {
            match interpret(key, state) {
                Press::Cancel => {
                    dialog.close();
                }
                Press::Disable => apply(""),
                Press::Confirm => {
                    let accel = pending.borrow().clone();
                    if let Some(accel) = accel.filter(|_| set.is_sensitive()) {
                        apply(&accel);
                    }
                }
                Press::Ignore | Press::Navigate => return glib::Propagation::Proceed,
                Press::Accel(accel) => {
                    let conflict = shortcuts::find_conflict(&settings, mode, &accel);
                    keys.set_accelerator(&accel);
                    set.set_sensitive(conflict.as_ref().is_none_or(|c| c.takeover));
                    show_status(conflict.as_ref());
                    pending.replace(Some(accel));
                }
            }
            glib::Propagation::Stop
        }
    ));
    dialog.add_controller(controller);

    cancel.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    set.connect_clicked(glib::clone!(
        #[strong]
        pending,
        #[strong]
        apply,
        move |_| {
            if let Some(accel) = pending.borrow().clone() {
                apply(&accel);
            }
        }
    ));

    dialog.present(Some(parent));
}
