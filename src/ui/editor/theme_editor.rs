//! The theme editor (add-style-system task 2.3/2.4; mockups 06, 08, 09, 10): the themes —
//! built in, installed by an administrator (read-only, with a lock), the user's own — and the
//! selected theme's name, palette (2–8 colours: click for the colour chooser, drag or Ctrl+←/→
//! to reorder), heading and body font, and a live preview of the styles it generates.
//!
//! Every change to one of the user's themes is saved at once (there is no Save button, as in
//! GNOME's own preferences) through [`Styles`], which tells the sidebar: an edit of the theme in
//! use changes the quick styles, the palette swatches and the tool defaults right away.
//! Import and export use the file chooser portal (`gtk::FileDialog`), filtered to
//! `*.annota-theme`.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use annota::document::color::{palette, Rgba};
use annota::styles::generate::{self, PREVIEW};
use annota::styles::store::StylePaths;
use annota::styles::theme::{self, Origin, Theme, DEFAULT_ID, MAX_COLOURS, MIN_COLOURS};
use annota::styles::tile;

use super::colour;
use super::quick_styles::TILE;
use super::styles::{self as words, Styles};

const UI: &str = "/io/github/trosinde/Annota/ui/theme-editor.ui";

thread_local! {
    /// The open theme editor (one at a time).
    static OPEN: RefCell<Option<Rc<ThemeEditor>>> = const { RefCell::new(None) };
}

pub struct ThemeEditor {
    styles: Rc<Styles>,
    dialog: adw::Dialog,
    toasts: adw::ToastOverlay,
    rows: gtk::ListBox,
    use_button: gtk::Button,
    in_use: gtk::Label,
    title: gtk::Label,
    more: gtk::MenuButton,
    export_button: gtk::Button,
    banner: gtk::Box,
    name_row: adw::EntryRow,
    name_lock: gtk::Image,
    palette_box: gtk::Box,
    heading_font: gtk::FontDialogButton,
    body_font: gtk::FontDialogButton,
    preview_tiles: gtk::Box,
    preview_heading: gtk::Label,
    preview_body: gtk::Label,
    preview_fonts: gtk::Label,
    swatch_popover: gtk::Popover,
    swatch_hex: gtk::Entry,
    swatch_remove: gtk::Button,
    swatch_left: gtk::Button,
    swatch_right: gtk::Button,
    actions: gio::SimpleActionGroup,
    /// The theme shown.
    selected: RefCell<String>,
    /// The swatch the colour chooser edits.
    swatch: Cell<Option<usize>>,
    swatch_buttons: RefCell<Vec<gtk::Button>>,
    updating: Cell<bool>,
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: theme-editor.ui is missing object '{}'", $name);
                return None;
            }
        }
    };
}

/// Shows the theme editor over `parent`, the current theme selected.
pub fn present(parent: &impl IsA<gtk::Widget>, styles: Rc<Styles>) -> Option<Rc<ThemeEditor>> {
    if let Some(open) = OPEN.with(|o| o.borrow().clone()) {
        open.dialog.present(Some(parent));
        return Some(open);
    }
    let editor = ThemeEditor::new(styles)?;
    OPEN.with(|o| o.replace(Some(editor.clone())));
    editor.dialog.connect_closed(|_| {
        OPEN.with(|o| o.replace(None));
    });
    editor.dialog.present(Some(parent));
    Some(editor)
}

impl ThemeEditor {
    fn new(styles: Rc<Styles>) -> Option<Rc<Self>> {
        let b = gtk::Builder::from_resource(UI);
        let editor = Rc::new(ThemeEditor {
            styles,
            dialog: obj!(b, "theme_editor"),
            toasts: obj!(b, "theme_toasts"),
            rows: obj!(b, "theme_rows"),
            use_button: obj!(b, "use_theme_button"),
            in_use: obj!(b, "in_use_label"),
            title: obj!(b, "detail_title"),
            more: obj!(b, "theme_more_button"),
            export_button: obj!(b, "export_theme_button"),
            banner: obj!(b, "managed_banner"),
            name_row: obj!(b, "theme_name_row"),
            name_lock: obj!(b, "name_lock"),
            palette_box: obj!(b, "palette_box"),
            heading_font: obj!(b, "heading_font_button"),
            body_font: obj!(b, "body_font_button"),
            preview_tiles: obj!(b, "preview_tiles"),
            preview_heading: obj!(b, "preview_heading"),
            preview_body: obj!(b, "preview_body"),
            preview_fonts: obj!(b, "preview_fonts"),
            swatch_popover: obj!(b, "swatch_popover"),
            swatch_hex: obj!(b, "swatch_hex"),
            swatch_remove: obj!(b, "swatch_remove"),
            swatch_left: obj!(b, "swatch_move_left"),
            swatch_right: obj!(b, "swatch_move_right"),
            actions: gio::SimpleActionGroup::new(),
            selected: RefCell::new(String::new()),
            swatch: Cell::new(None),
            swatch_buttons: RefCell::new(Vec::new()),
            updating: Cell::new(false),
        });
        let new_button: gtk::Button = obj!(b, "new_theme_button");
        let import_button: gtk::Button = obj!(b, "import_theme_button");
        let duplicate: gtk::Button = obj!(b, "banner_duplicate_button");
        let pick: gtk::Button = obj!(b, "swatch_pick");
        let custom: gtk::Button = obj!(b, "swatch_custom");
        editor.setup(&new_button, &import_button, &duplicate, &pick, &custom);
        let current = editor.styles.current_theme().id;
        editor.selected.replace(current);
        editor.fill_list();
        editor.show_selected();
        Some(editor)
    }

    fn setup(
        self: &Rc<Self>,
        new_button: &gtk::Button,
        import_button: &gtk::Button,
        duplicate: &gtk::Button,
        pick: &gtk::Button,
        custom: &gtk::Button,
    ) {
        // The swatch popover lives in the palette box (a GtkBox unparents it when disposed).
        self.swatch_popover.set_parent(&self.palette_box);
        self.rows.set_header_func(|row, before| {
            // "system/…", "user/…", else built in.
            let section = |r: &gtk::ListBoxRow| -> Option<String> {
                let name = r.widget_name();
                Some(match name.split_once('/') {
                    Some((prefix, _)) => prefix.to_string(),
                    None => "builtin".to_string(),
                })
            };
            let mine = section(row);
            if before.and_then(section) == mine {
                row.set_header(None::<&gtk::Widget>);
                return;
            }
            let text = match mine.as_deref() {
                Some("system") => gettext("Installed by administrator"),
                Some("user") => gettext("My Themes"),
                _ => gettext("Built-in"),
            };
            let label = gtk::Label::builder()
                .label(text)
                .xalign(0.0)
                .margin_top(12)
                .margin_bottom(6)
                .margin_start(12)
                .css_classes(["heading", "dim-label"])
                .build();
            row.set_header(Some(&label));
        });
        let weak = Rc::downgrade(self);
        self.rows.connect_row_selected(move |_, row| {
            let (Some(editor), Some(row)) = (weak.upgrade(), row) else {
                return;
            };
            if editor.updating.get() {
                return;
            }
            editor.selected.replace(row.widget_name().to_string());
            editor.show_selected();
        });
        let weak = Rc::downgrade(self);
        self.styles.connect_changed(move || {
            if let Some(editor) = weak.upgrade() {
                editor.fill_list();
                editor.show_selected();
            }
        });
        let weak = Rc::downgrade(self);
        self.use_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                let id = editor.selected.borrow().clone();
                editor.styles.set_theme(&id);
            }
        });
        let weak = Rc::downgrade(self);
        new_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.new_theme();
            }
        });
        let weak = Rc::downgrade(self);
        import_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.choose_import();
            }
        });
        let weak = Rc::downgrade(self);
        self.export_button.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.choose_export();
            }
        });
        let weak = Rc::downgrade(self);
        duplicate.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.duplicate();
            }
        });
        // Name.
        let weak = Rc::downgrade(self);
        self.name_row.connect_apply(move |row| {
            if let Some(editor) = weak.upgrade() {
                let name = row.text().trim().to_string();
                if !name.is_empty() {
                    editor.edit(|t| t.name = name);
                }
            }
        });
        // Fonts.
        for (button, heading) in [(&self.heading_font, true), (&self.body_font, false)] {
            let weak = Rc::downgrade(self);
            button.connect_font_desc_notify(move |button| {
                let Some(editor) = weak.upgrade() else {
                    return;
                };
                if editor.updating.get() {
                    return;
                }
                let Some(family) = button.font_desc().and_then(|d| d.family()) else {
                    return;
                };
                let family = family.to_string();
                editor.edit(|t| {
                    if heading {
                        t.heading_font = family;
                    } else {
                        t.body_font = family;
                    }
                });
            });
        }
        // The swatch popover.
        let weak = Rc::downgrade(self);
        self.swatch_hex.connect_activate(move |entry| {
            let Some(editor) = weak.upgrade() else {
                return;
            };
            if let Some(c) = colour::parse_hex(&entry.text()) {
                editor.set_swatch(c);
            }
        });
        let weak = Rc::downgrade(self);
        self.swatch_remove.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.remove_swatch();
            }
        });
        for (button, delta) in [(&self.swatch_left, -1isize), (&self.swatch_right, 1)] {
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(editor) = weak.upgrade() {
                    if let Some(i) = editor.swatch.get() {
                        editor.move_swatch(i, delta);
                    }
                }
            });
        }
        let weak = Rc::downgrade(self);
        pick.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.pick_from_screen();
            }
        });
        let weak = Rc::downgrade(self);
        custom.connect_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.choose_custom();
            }
        });
        let weak = Rc::downgrade(self);
        self.swatch_popover.connect_closed(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.swatch.set(None);
            }
        });
        // More menu.
        let add = |name: &str, f: fn(&Rc<Self>)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(editor) = weak.upgrade() {
                    f(&editor);
                }
            });
            self.actions.add_action(&action);
        };
        add("export", |e| e.choose_export());
        add("duplicate", |e| e.duplicate());
        add("delete", |e| e.confirm_delete());
        self.dialog
            .insert_action_group("theme", Some(&self.actions));
        // Preview tiles.
        for (tool, slot) in PREVIEW {
            let area = gtk::DrawingArea::builder()
                .content_width(TILE.0)
                .content_height(TILE.1)
                .css_classes(["annota-style-tile"])
                .overflow(gtk::Overflow::Hidden)
                .build();
            let weak = Rc::downgrade(self);
            area.set_draw_func(move |_, cr, w, h| {
                let Some(theme) = weak.upgrade().and_then(|e| e.theme()) else {
                    return;
                };
                let values = generate::style(&theme, tool, slot);
                let _ = tile::draw(cr, tool, &values, f64::from(w), f64::from(h));
            });
            self.preview_tiles.append(&area);
        }
    }

    /// The theme shown.
    fn theme(&self) -> Option<Theme> {
        self.styles.theme(&self.selected.borrow())
    }

    fn fill_list(self: &Rc<Self>) {
        let was = self.updating.replace(true);
        while let Some(child) = self.rows.first_child() {
            self.rows.remove(&child);
        }
        let themes = self.styles.themes().clone();
        let selected = self.selected.borrow().clone();
        for theme in &themes {
            let row = gtk::ListBoxRow::new();
            row.set_widget_name(&theme.id);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.set_margin_top(8);
            line.set_margin_bottom(8);
            line.set_margin_start(6);
            line.set_margin_end(6);
            let chips = gtk::DrawingArea::builder()
                .content_width(5 * 8 + 4 * 2)
                .content_height(16)
                .valign(gtk::Align::Center)
                .build();
            let colours = theme.colours.clone();
            chips.set_draw_func(move |_, cr, _, _| {
                let _ = words::draw_chips(cr, &colours, 5, 8.0, 16.0, 2.0);
            });
            line.append(&chips);
            line.append(
                &gtk::Label::builder()
                    .label(&theme.name)
                    .xalign(0.0)
                    .hexpand(true)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build(),
            );
            if theme.id == DEFAULT_ID {
                line.append(
                    &gtk::Label::builder()
                        .label(gettext("Default"))
                        .css_classes(["dim-label", "caption"])
                        .build(),
                );
            }
            if theme.origin == Origin::Managed {
                let lock = gtk::Image::from_icon_name("changes-prevent-symbolic");
                lock.add_css_class("dim-label");
                lock.set_tooltip_text(Some(&gettext("Managed by your administrator")));
                line.append(&lock);
            }
            row.set_child(Some(&line));
            row.update_property(&[gtk::accessible::Property::Label(&theme.name)]);
            self.rows.append(&row);
            if theme.id == selected {
                self.rows.select_row(Some(&row));
            }
        }
        // The shown theme is gone (deleted): the current one.
        if !themes.iter().any(|t| t.id == selected) {
            let current = self.styles.current_theme().id;
            self.selected.replace(current.clone());
            let mut i = 0;
            while let Some(row) = self.rows.row_at_index(i) {
                if row.widget_name() == current {
                    self.rows.select_row(Some(&row));
                }
                i += 1;
            }
        }
        self.updating.set(was);
    }

    fn show_selected(self: &Rc<Self>) {
        let Some(theme) = self.theme() else {
            return;
        };
        let was = self.updating.replace(true);
        let current = self.styles.current_theme().id == theme.id;
        let editable = theme.editable();
        let managed = theme.origin == Origin::Managed;
        self.title.set_text(&theme.name);
        self.use_button.set_visible(!current);
        self.in_use.set_visible(current);
        // Mockup 08: managed themes have the banner and no More menu.
        self.banner.set_visible(managed);
        self.more.set_visible(!managed);
        if let Some(delete) = self
            .actions
            .lookup_action("delete")
            .and_downcast::<gio::SimpleAction>()
        {
            delete.set_enabled(editable);
        }
        if self.name_row.text() != theme.name {
            self.name_row.set_text(&theme.name);
        }
        self.name_row.set_editable(editable);
        if editable {
            self.name_row.remove_css_class("annota-readonly");
        } else {
            self.name_row.add_css_class("annota-readonly");
        }
        self.name_row.set_show_apply_button(editable);
        self.name_lock.set_visible(!editable);
        for (button, family) in [
            (&self.heading_font, &theme.heading_font),
            (&self.body_font, &theme.body_font),
        ] {
            let shown = button
                .font_desc()
                .and_then(|d| d.family())
                .map(|f| f.to_string());
            if shown.as_deref() != Some(family.as_str()) {
                let mut desc = gtk::pango::FontDescription::new();
                desc.set_family(family);
                button.set_font_desc(&desc);
            }
            button.set_sensitive(editable);
        }
        self.show_palette(&theme);
        self.show_preview(&theme);
        self.updating.set(was);
    }

    fn show_palette(self: &Rc<Self>, theme: &Theme) {
        // Everything but the colour chooser popover, which lives here too.
        let mut child = self.palette_box.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if !c.is::<gtk::Popover>() {
                self.palette_box.remove(&c);
            }
        }
        let editable = theme.editable();
        let mut buttons = Vec::new();
        for (i, c) in theme.colours.iter().copied().enumerate() {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let button = gtk::Button::builder()
                .css_classes(["annota-theme-swatch"])
                .tooltip_text(theme::hex(c))
                .build();
            button.set_size_request(40, 40);
            let area = gtk::DrawingArea::builder()
                .content_width(40)
                .content_height(40)
                .can_target(false)
                .build();
            area.set_draw_func(move |_, cr, w, h| {
                rounded(cr, f64::from(w), f64::from(h), 9.0);
                c.set_source(cr);
                let _ = cr.fill();
            });
            button.set_child(Some(&area));
            button.update_property(&[gtk::accessible::Property::Label(
                &gettext("Colour {n}: {hex}")
                    .replace("{n}", &(i + 1).to_string())
                    .replace("{hex}", &theme::hex(c)),
            )]);
            if editable {
                let weak = Rc::downgrade(self);
                button.connect_clicked(move |b| {
                    if let Some(editor) = weak.upgrade() {
                        editor.open_swatch(i, b);
                    }
                });
                self.reorderable(&button, i);
            } else {
                button.set_can_target(false);
                button.set_focusable(false);
            }
            column.append(&button);
            column.append(
                &gtk::Label::builder()
                    .label(theme::hex(c))
                    .css_classes(["annota-hex-caption", "dim-label"])
                    .build(),
            );
            self.palette_box.append(&column);
            buttons.push(button);
        }
        // The dashed Add tile (hidden at 8 colours and for read-only themes).
        if editable && theme.colours.len() < MAX_COLOURS {
            let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let add = gtk::Button::builder()
                .icon_name("list-add-symbolic")
                .tooltip_text(gettext("Add Colour"))
                .css_classes(["annota-theme-add"])
                .build();
            add.set_size_request(40, 40);
            add.update_property(&[gtk::accessible::Property::Label(&gettext("Add Colour"))]);
            let weak = Rc::downgrade(self);
            add.connect_clicked(move |_| {
                if let Some(editor) = weak.upgrade() {
                    editor.add_swatch();
                }
            });
            column.append(&add);
            column.append(
                &gtk::Label::builder()
                    .label(gettext("Add"))
                    .css_classes(["annota-hex-caption", "dim-label"])
                    .build(),
            );
            self.palette_box.append(&column);
        }
        self.swatch_buttons.replace(buttons);
    }

    /// Drag a swatch onto another to move it there; Ctrl+←/→ on a focused swatch.
    fn reorderable(self: &Rc<Self>, button: &gtk::Button, index: usize) {
        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::MOVE);
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&(index as u32).to_value()))
        });
        button.add_controller(source);
        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let weak = Rc::downgrade(self);
        target.connect_drop(move |_, value, _, _| {
            let (Some(editor), Ok(from)) = (weak.upgrade(), value.get::<u32>()) else {
                return false;
            };
            editor.reorder(from as usize, index)
        });
        button.add_controller(target);
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(editor) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            let delta = match key {
                gdk::Key::Left | gdk::Key::KP_Left => -1,
                gdk::Key::Right | gdk::Key::KP_Right => 1,
                _ => return glib::Propagation::Proceed,
            };
            editor.move_swatch(index, delta);
            glib::Propagation::Stop
        });
        button.add_controller(keys);
    }

    fn show_preview(&self, theme: &Theme) {
        let mut child = self.preview_tiles.first_child();
        while let Some(c) = child {
            c.queue_draw();
            child = c.next_sibling();
        }
        let attrs = |family: &str, colour: Option<Rgba>, bold: bool| {
            let list = gtk::pango::AttrList::new();
            list.insert(gtk::pango::AttrString::new_family(family));
            if let Some(c) = colour {
                let scale = |v: u8| u16::from(v) * 257;
                list.insert(gtk::pango::AttrColor::new_foreground(
                    scale(c.r),
                    scale(c.g),
                    scale(c.b),
                ));
            }
            if bold {
                list.insert(gtk::pango::AttrInt::new_weight(gtk::pango::Weight::Bold));
                list.insert(gtk::pango::AttrSize::new(15 * gtk::pango::SCALE));
            }
            list
        };
        self.preview_heading.set_attributes(Some(&attrs(
            &theme.heading_font,
            theme.colour(0),
            true,
        )));
        self.preview_body
            .set_attributes(Some(&attrs(&theme.body_font, None, false)));
        self.preview_fonts
            .set_text(&format!("{} / {}", theme.heading_font, theme.body_font));
    }

    // ---- editing ----

    /// Changes the shown theme (one of the user's) and saves it.
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Theme)) {
        let Some(mut theme) = self.theme().filter(Theme::editable) else {
            return;
        };
        let before = theme.clone();
        change(&mut theme);
        if theme == before {
            return;
        }
        if let Err(err) = self.styles.save_theme(&mut theme) {
            eprintln!("annota: cannot save theme: {err}");
            self.toast(&gettext("The theme could not be saved"), None);
        }
    }

    fn open_swatch(self: &Rc<Self>, index: usize, anchor: &gtk::Button) {
        let Some(theme) = self.theme() else {
            return;
        };
        let Some(c) = theme.colour(index) else {
            return;
        };
        self.swatch.set(Some(index));
        self.swatch_hex.set_text(&theme::hex(c));
        self.swatch_remove
            .set_sensitive(theme.colours.len() > MIN_COLOURS);
        self.swatch_left.set_sensitive(index > 0);
        self.swatch_right
            .set_sensitive(index + 1 < theme.colours.len());
        if let Some(b) = anchor.compute_bounds(&self.palette_box) {
            self.swatch_popover
                .set_pointing_to(Some(&gdk::Rectangle::new(
                    b.x() as i32,
                    b.y() as i32,
                    b.width() as i32,
                    b.height() as i32,
                )));
        }
        self.swatch_popover.popup();
        self.swatch_hex.grab_focus();
    }

    fn set_swatch(self: &Rc<Self>, c: Rgba) {
        let Some(i) = self.swatch.get() else {
            return;
        };
        self.edit(|t| {
            if let Some(slot) = t.colours.get_mut(i) {
                *slot = c;
            }
        });
        self.swatch_popover.popdown();
    }

    fn remove_swatch(self: &Rc<Self>) {
        let Some(i) = self.swatch.get() else {
            return;
        };
        self.swatch_popover.popdown();
        self.edit(|t| {
            if t.colours.len() > MIN_COLOURS && i < t.colours.len() {
                t.colours.remove(i);
            }
        });
    }

    /// Adds a colour (one of Annota's palette the theme does not have yet) and opens its
    /// chooser.
    fn add_swatch(self: &Rc<Self>) {
        let Some(theme) = self.theme() else {
            return;
        };
        let next = colour::PALETTE
            .iter()
            .copied()
            .find(|c| !theme.colours.contains(c))
            .unwrap_or(palette::INK);
        let index = theme.colours.len();
        self.edit(|t| {
            if t.colours.len() < MAX_COLOURS {
                t.colours.push(next);
            }
        });
        let button = self.swatch_buttons.borrow().get(index).cloned();
        if let Some(button) = button {
            self.open_swatch(index, &button);
        }
    }

    fn move_swatch(self: &Rc<Self>, index: usize, delta: isize) {
        if let Some(to) = index.checked_add_signed(delta) {
            if self.reorder(index, to) {
                self.swatch_popover.popdown();
                let button = self.swatch_buttons.borrow().get(to).cloned();
                if let Some(button) = button {
                    button.grab_focus();
                }
            }
        }
    }

    /// Moves colour `from` to position `to`.
    fn reorder(self: &Rc<Self>, from: usize, to: usize) -> bool {
        let Some(theme) = self.theme() else {
            return false;
        };
        let n = theme.colours.len();
        if from == to || from >= n || to >= n || !theme.editable() {
            return false;
        }
        self.edit(|t| {
            let c = t.colours.remove(from);
            t.colours.insert(to, c);
        });
        true
    }

    fn pick_from_screen(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.swatch_popover.popdown();
        let index = self.swatch.get();
        glib::spawn_future_local(async move {
            let result = crate::capture::portal::pick_colour().await;
            let Some(editor) = weak.upgrade() else {
                return;
            };
            match result {
                Ok((r, g, b)) => {
                    editor.swatch.set(index);
                    editor.set_swatch(colour::from_portal(r, g, b));
                }
                Err(crate::capture::portal::CaptureError::Cancelled) => {}
                Err(err) => {
                    eprintln!("annota: eyedropper: {err}");
                    editor.toast(&gettext("The colour could not be picked"), None);
                }
            }
        });
    }

    /// GTK's colour chooser for any colour.
    fn choose_custom(self: &Rc<Self>) {
        let Some(c) = self
            .swatch
            .get()
            .and_then(|i| self.theme().and_then(|t| t.colour(i)))
        else {
            return;
        };
        let index = self.swatch.get();
        self.swatch_popover.popdown();
        let dialog = gtk::ColorDialog::builder()
            .title(gettext("Custom Colour"))
            .with_alpha(false)
            .modal(true)
            .build();
        let (r, g, b, _) = c.to_f64();
        let initial = gdk::RGBA::new(r as f32, g as f32, b as f32, 1.0);
        let window = self.dialog.root().and_downcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(rgba) = dialog
                .choose_rgba_future(window.as_ref(), Some(&initial))
                .await
            else {
                return;
            };
            if let Some(editor) = weak.upgrade() {
                let channel = |v: f32| (f64::from(v).clamp(0.0, 1.0) * 255.0).round() as u8;
                editor.swatch.set(index);
                editor.set_swatch(Rgba::opaque(
                    channel(rgba.red()),
                    channel(rgba.green()),
                    channel(rgba.blue()),
                ));
            }
        });
    }

    // ---- themes ----

    fn select(self: &Rc<Self>, id: &str) {
        self.selected.replace(id.to_string());
        self.fill_list();
        self.show_selected();
    }

    /// "+": a new theme of the user's, starting from the shown theme's colours and fonts.
    fn new_theme(self: &Rc<Self>) {
        let base = self.theme().unwrap_or_else(Theme::annota);
        let name = unique_theme_name(&self.styles.themes(), &gettext("New Theme"));
        self.save_new(Theme { name, ..base });
    }

    fn duplicate(self: &Rc<Self>) {
        let Some(theme) = self.theme() else {
            return;
        };
        let name = unique_theme_name(
            &self.styles.themes(),
            &gettext("{name} Copy").replace("{name}", &theme.name),
        );
        self.save_new(Theme { name, ..theme });
    }

    /// Saves `theme` as a new theme of the user's and shows it.
    fn save_new(self: &Rc<Self>, mut theme: Theme) {
        theme.origin = Origin::BuiltIn;
        theme.id = String::new();
        match self.styles.save_theme(&mut theme) {
            Ok(()) => self.select(&theme.id),
            Err(err) => {
                eprintln!("annota: cannot save theme: {err}");
                self.toast(&gettext("The theme could not be saved"), None);
            }
        }
    }

    fn confirm_delete(self: &Rc<Self>) {
        let Some(theme) = self.theme().filter(Theme::editable) else {
            return;
        };
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete Theme “{name}”?").replace("{name}", &theme.name)),
            Some(&gettext(
                "Its generated quick styles disappear. Your own quick styles stay.",
            )),
        );
        alert.add_responses(&[
            ("cancel", &gettext("Cancel")),
            ("delete", &gettext("Delete")),
        ]);
        alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        alert.connect_response(None, move |_, response| {
            if response != "delete" {
                return;
            }
            if let Some(editor) = weak.upgrade() {
                if let Err(err) = editor.styles.delete_theme(&theme) {
                    eprintln!("annota: cannot delete theme: {err}");
                }
            }
        });
        alert.present(Some(&self.dialog));
    }

    fn filters() -> (gio::ListStore, gtk::FileFilter) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Annota Themes")));
        filter.add_suffix(theme::EXTENSION);
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        (filters, filter)
    }

    /// Import Theme… (folder button): the file chooser portal, `*.annota-theme`.
    fn choose_import(self: &Rc<Self>) {
        let (filters, filter) = Self::filters();
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Import Theme"))
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .build();
        let window = self.dialog.root().and_downcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.open_future(window.as_ref()).await else {
                return;
            };
            let (Some(editor), Some(path)) = (weak.upgrade(), file.path()) else {
                return;
            };
            editor.import(&path);
        });
    }

    /// Imports `path`: "Theme “X” imported" with Apply, or the alert saying why not.
    pub fn import(self: &Rc<Self>, path: &Path) {
        match self.styles.import(path) {
            Ok(theme) => {
                self.select(&theme.id);
                let title = gettext("Theme “{name}” imported").replace("{name}", &theme.name);
                let weak = Rc::downgrade(self);
                let id = theme.id.clone();
                let apply: Rc<dyn Fn()> = Rc::new(move || {
                    if let Some(editor) = weak.upgrade() {
                        editor.styles.set_theme(&id);
                    }
                });
                self.toast(&title, Some((gettext("Apply"), apply)));
            }
            Err(err) => {
                let file = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Can’t Import Theme")),
                    Some(&words::import_error(&file, &err)),
                );
                alert.add_response("ok", &gettext("OK"));
                alert.set_response_appearance("ok", adw::ResponseAppearance::Suggested);
                alert.set_default_response(Some("ok"));
                alert.set_close_response("ok");
                alert.present(Some(&self.dialog));
            }
        }
    }

    /// Export Theme… (header button, More menu).
    fn choose_export(self: &Rc<Self>) {
        let Some(theme) = self.theme() else {
            return;
        };
        let (filters, filter) = Self::filters();
        let dialog = gtk::FileDialog::builder()
            .title(gettext("Export Theme"))
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .initial_name(format!("{}.{}", file_name(&theme.name), theme::EXTENSION))
            .build();
        let window = self.dialog.root().and_downcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(file) = dialog.save_future(window.as_ref()).await else {
                return;
            };
            let (Some(editor), Some(path)) = (weak.upgrade(), file.path()) else {
                return;
            };
            editor.export(&theme, &path);
        });
    }

    pub fn export(&self, theme: &Theme, path: &Path) {
        match StylePaths::export(theme, path) {
            Ok(()) => self.toast(
                &gettext("Theme “{name}” exported").replace("{name}", &theme.name),
                None,
            ),
            Err(err) => {
                eprintln!("annota: cannot export theme: {err}");
                self.toast(&gettext("The theme could not be exported"), None);
            }
        }
    }

    fn toast(&self, title: &str, button: Option<(String, Rc<dyn Fn()>)>) {
        let toast = adw::Toast::new(title);
        toast.set_use_markup(false);
        if let Some((label, action)) = button {
            // Its button is for what was just done: shown at once.
            toast.set_priority(adw::ToastPriority::High);
            toast.set_button_label(Some(&label));
            toast.connect_button_clicked(move |_| action());
        }
        self.toasts.add_toast(toast);
    }

    // ---- for tests ----

    #[cfg(test)]
    pub fn close(&self) {
        self.dialog.force_close();
    }

    /// (id, row shows a lock) of the list, top to bottom.
    #[cfg(test)]
    pub fn list(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(row) = self.rows.row_at_index(i) {
            out.push(row.widget_name().to_string());
            i += 1;
        }
        out
    }

    #[cfg(test)]
    pub fn show_theme(self: &Rc<Self>, id: &str) {
        self.select(id);
    }

    /// (banner, name editable, fonts sensitive, Add tile, More menu, Use Theme) of the detail.
    #[cfg(test)]
    pub fn detail(&self) -> (bool, bool, bool, bool, bool, bool) {
        let add = {
            let mut found = false;
            let mut c = self.palette_box.first_child();
            while let Some(w) = c {
                if w.first_child()
                    .is_some_and(|b| b.has_css_class("annota-theme-add"))
                {
                    found = true;
                }
                c = w.next_sibling();
            }
            found
        };
        (
            self.banner.is_visible(),
            self.name_row.is_editable(),
            self.heading_font.is_sensitive(),
            add,
            self.more.is_visible(),
            self.use_button.is_visible(),
        )
    }

    #[cfg(test)]
    pub fn use_theme(&self) {
        self.use_button.emit_clicked();
    }

    #[cfg(test)]
    pub fn rename(&self, name: &str) {
        self.name_row.set_text(name);
        self.name_row.emit_by_name::<()>("apply", &[]);
    }

    #[cfg(test)]
    pub fn edit_colour(self: &Rc<Self>, index: usize, c: Rgba) {
        self.swatch.set(Some(index));
        self.set_swatch(c);
    }

    #[cfg(test)]
    pub fn add_colour(self: &Rc<Self>) {
        self.add_swatch();
        self.swatch_popover.popdown();
    }

    #[cfg(test)]
    pub fn remove_colour(self: &Rc<Self>, index: usize) {
        self.swatch.set(Some(index));
        self.remove_swatch();
    }

    #[cfg(test)]
    pub fn move_colour(self: &Rc<Self>, index: usize, delta: isize) {
        self.move_swatch(index, delta);
    }

    #[cfg(test)]
    pub fn new_theme_for_test(self: &Rc<Self>) {
        self.new_theme();
    }

    #[cfg(test)]
    pub fn duplicate_for_test(self: &Rc<Self>) {
        self.duplicate();
    }

    #[cfg(test)]
    pub fn selected(&self) -> String {
        self.selected.borrow().clone()
    }

    #[cfg(test)]
    pub fn widget(&self) -> adw::Dialog {
        self.dialog.clone()
    }
}

/// `base`, else `base 2` … — a theme name not in use.
fn unique_theme_name(themes: &[Theme], base: &str) -> String {
    let taken = |name: &str| themes.iter().any(|t| t.name == name);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

/// A file name for a theme name (no path separators).
fn file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '\0') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() {
        gettext("Theme")
    } else {
        cleaned
    }
}

fn rounded(cr: &gtk::cairo::Context, w: f64, h: f64, r: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    cr.new_sub_path();
    cr.arc(w - r, r, r, -FRAC_PI_2, 0.0);
    cr.arc(w - r, h - r, r, 0.0, FRAC_PI_2);
    cr.arc(r, h - r, r, FRAC_PI_2, PI);
    cr.arc(r, r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_names_and_file_names() {
        let themes = vec![Theme::annota(), Theme::monochrome()];
        assert_eq!(unique_theme_name(&themes, "Annota"), "Annota 2");
        assert_eq!(unique_theme_name(&themes, "Brand"), "Brand");
        assert_eq!(file_name("Northwind/Ferries"), "Northwind-Ferries");
        assert_eq!(file_name(" .hidden"), "hidden");
        assert_eq!(file_name("  "), "Theme");
    }
}
