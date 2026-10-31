//! "Restore Unsaved Captures?" (mockup 14, spec document-format: Autosave and recovery) and
//! "Newer Annota Version Needed" (mockup 15, spec document-format: Format versions).

use std::f64::consts::{FRAC_PI_2, PI};
use std::path::Path;
use std::rc::Rc;
use std::time::UNIX_EPOCH;

use adw::prelude::*;
use gtk::{gio, glib};

use annota::document::archive::FormatVersion;
use annota::document::autosave::RecoveryEntry;
use annota::document::bitmap::Bitmap;

use super::text;
use crate::config;

const UI_RESTORE: &str = "/io/github/trosinde/Annota/ui/restore-dialog.ui";
const UI_NEWER: &str = "/io/github/trosinde/Annota/ui/newer-version-dialog.ui";
/// Redline: 64 × 40 row thumbnails.
const ROW_THUMB: (i32, i32) = (64, 40);

/// Looks up a builder object, or logs and returns (never panics; see `preferences.rs`).
macro_rules! obj {
    ($builder:expr, $ui:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: {} is missing object '{}'", $ui, $name);
                return;
            }
        }
    };
}

/// Name shown for an autosave (the title, else the file it belongs to, else "Untitled").
pub fn entry_name(entry: &RecoveryEntry) -> String {
    entry.name.clone().unwrap_or_else(text::untitled)
}

/// The entries whose row is checked.
pub fn checked(entries: &[RecoveryEntry], flags: &[bool]) -> Vec<RecoveryEntry> {
    entries
        .iter()
        .zip(flags)
        .filter(|(_, on)| **on)
        .map(|(e, _)| e.clone())
        .collect()
}

fn local_time(time: std::time::SystemTime) -> Option<glib::DateTime> {
    let seconds = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    glib::DateTime::from_unix_local(i64::try_from(seconds).ok()?).ok()
}

/// Shows "Restore Unsaved Captures?" over `parent`. Every row starts checked (redline). Restore
/// hands the checked entries to `restore`, Discard to `discard`; unchecked entries, and all of
/// them if the dialog is closed otherwise, stay on disk and are offered again next time.
pub fn present_restore(
    parent: &impl IsA<gtk::Widget>,
    entries: Vec<RecoveryEntry>,
    restore: impl Fn(Vec<RecoveryEntry>) + 'static,
    discard: impl Fn(Vec<RecoveryEntry>) + 'static,
) {
    let builder = gtk::Builder::from_resource(UI_RESTORE);
    let dialog: adw::Dialog = obj!(builder, "restore-dialog.ui", "restore_dialog");
    let list: gtk::ListBox = obj!(builder, "restore-dialog.ui", "entries_list");
    let restore_button: gtk::Button = obj!(builder, "restore-dialog.ui", "restore_button");
    let discard_button: gtk::Button = obj!(builder, "restore-dialog.ui", "discard_button");

    let now = glib::DateTime::now_local().ok();
    let mut checks = Vec::with_capacity(entries.len());
    for entry in &entries {
        let check = gtk::CheckButton::builder()
            .active(true)
            .valign(gtk::Align::Center)
            .build();
        let bitmap = entry
            .thumbnail_png
            .clone()
            .and_then(|png| Bitmap::from_png(png).ok());
        let thumbnail = thumbnail_area(bitmap, ROW_THUMB, 4.0);
        thumbnail.add_css_class("annota-row-thumb");
        let subtitle = match (local_time(entry.saved_at), &now) {
            (Some(saved), Some(now)) => text::autosaved(&saved, now, entry.object_count),
            _ => text::object_count(entry.object_count),
        };
        let row = adw::ActionRow::builder()
            .title(entry_name(entry))
            .subtitle(subtitle)
            .use_markup(false)
            .activatable_widget(&check)
            .build();
        // Prefixes are packed from the start: check, then thumbnail (mockup 14).
        row.add_prefix(&thumbnail);
        row.add_prefix(&check);
        list.append(&row);
        checks.push(check);
    }

    let entries = Rc::new(entries);
    let flags = {
        let checks = checks.clone();
        move || checks.iter().map(|c| c.is_active()).collect::<Vec<bool>>()
    };
    // Nothing to do without a checked row.
    let update = {
        let checks = checks.clone();
        let restore_button = restore_button.clone();
        let discard_button = discard_button.clone();
        move || {
            let any = checks.iter().any(|c| c.is_active());
            restore_button.set_sensitive(any);
            discard_button.set_sensitive(any);
        }
    };
    for check in &checks {
        let update = update.clone();
        check.connect_toggled(move |_| update());
    }

    restore_button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        entries,
        #[strong]
        flags,
        move |_| {
            dialog.close();
            restore(checked(&entries, &flags()));
        }
    ));
    discard_button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[strong]
        entries,
        move |_| {
            dialog.close();
            discard(checked(&entries, &flags()));
        }
    ));
    dialog.set_default_widget(Some(&restore_button));
    dialog.present(Some(parent));
    // Enter restores (the suggested response), it does not toggle the first row.
    dialog.set_focus(Some(&restore_button));
}

/// A `size` thumbnail of `bitmap`, scaled to cover it, with corners rounded by `radius`. (A
/// `GtkPicture` would ask for the bitmap's own size where nothing limits its height.)
pub(super) fn thumbnail_area(
    bitmap: Option<Bitmap>,
    size: (i32, i32),
    radius: f64,
) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size.0)
        .content_height(size.1)
        .valign(gtk::Align::Center)
        .build();
    area.set_draw_func(move |_, cr, w, h| {
        let (w, h) = (f64::from(w), f64::from(h));
        cr.new_sub_path();
        cr.arc(w - radius, radius, radius, -FRAC_PI_2, 0.0);
        cr.arc(w - radius, h - radius, radius, 0.0, FRAC_PI_2);
        cr.arc(radius, h - radius, radius, FRAC_PI_2, PI);
        cr.arc(radius, radius, radius, PI, 3.0 * FRAC_PI_2);
        cr.close_path();
        cr.clip();
        let Some(bitmap) = &bitmap else {
            return;
        };
        let (bw, bh) = (f64::from(bitmap.width()), f64::from(bitmap.height()));
        let scale = (w / bw).max(h / bh);
        cr.translate((w - bw * scale) / 2.0, (h - bh * scale) / 2.0);
        cr.scale(scale, scale);
        if cr.set_source_surface(bitmap.surface(), 0.0, 0.0).is_ok() {
            cr.source().set_filter(gtk::cairo::Filter::Good);
            let _ = cr.paint();
        }
    });
    area
}

/// Shows "Newer Annota Version Needed" for `path` over `parent`.
pub fn present_newer_version(
    parent: &impl IsA<gtk::Widget>,
    path: &Path,
    found: FormatVersion,
    requires: Option<&str>,
) {
    let builder = gtk::Builder::from_resource(UI_NEWER);
    let dialog: adw::Dialog = obj!(builder, "newer-version-dialog.ui", "newer_version_dialog");
    let body: gtk::Label = obj!(builder, "newer-version-dialog.ui", "body_label");
    let show: gtk::Button = obj!(builder, "newer-version-dialog.ui", "show_button");
    let ok: gtk::Button = obj!(builder, "newer-version-dialog.ui", "ok_button");
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    body.set_text(&text::newer_version_body(
        &file_name,
        found,
        requires,
        config::VERSION,
    ));
    let file = gio::File::for_path(path);
    show.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            let window = dialog.root().and_downcast::<gtk::Window>();
            gtk::FileLauncher::new(Some(&file)).open_containing_folder(
                window.as_ref(),
                gio::Cancellable::NONE,
                |result| {
                    if let Err(err) = result {
                        eprintln!("annota: cannot show the file: {err}");
                    }
                },
            );
            dialog.close();
        }
    ));
    ok.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    dialog.set_default_widget(Some(&ok));
    dialog.present(Some(parent));
    dialog.set_focus(Some(&ok));
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::autosave::AutosaveId;
    use std::path::PathBuf;

    fn entry(name: Option<&str>) -> RecoveryEntry {
        RecoveryEntry {
            id: AutosaveId::generate(),
            path: PathBuf::from("/tmp/x.annota"),
            name: name.map(str::to_string),
            saved_at: UNIX_EPOCH,
            object_count: 3,
            thumbnail_png: None,
            source: None,
        }
    }

    #[test]
    fn rows_name_untitled_captures() {
        assert_eq!(entry_name(&entry(Some("Checkout flow"))), "Checkout flow");
        assert_eq!(entry_name(&entry(None)), "Untitled Capture");
    }

    #[test]
    fn only_checked_entries_are_restored_or_discarded() {
        let entries = vec![entry(Some("a")), entry(Some("b")), entry(Some("c"))];
        let picked = checked(&entries, &[true, false, true]);
        assert_eq!(
            picked.iter().map(entry_name).collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert!(checked(&entries, &[false; 3]).is_empty());
    }
}
