//! Quick styles in the properties sidebar (add-style-system task 2.1/2.2; mockups 01–05, 07):
//! the "<Tool> | Favourites" toggle, the theme's eight generated styles of the tool, "My
//! Styles", "Modified", Save as Quick Style, the style context menu, the theme switcher and the
//! Favourites view.
//!
//! What a tile does goes to the editor as a [`QuickEvent`]: applying a style is the canvas's
//! ordinary property path (`Canvas::apply_style`: the selection as one undo step, else the tool
//! defaults). Changes to the library (save, rename, move, star, update, delete) go to
//! [`Styles`] and are announced with a toast and Undo where they lose something (save,
//! delete) — they are application state, not steps of a document's history.
//!
//! Which tile is "selected" is derived, not remembered: the style whose properties the current
//! values contain (preferring the one applied or saved last, when several match). With none,
//! the values are "Modified" and Save as Quick Style is enabled.
//!
//! Keyboard (redline): the arrow keys move in a grid (GtkFlowBox), Enter applies, F2 renames,
//! Ctrl+←/→ moves, Delete deletes, Menu or Shift+F10 opens the context menu. The editor window
//! does not take these keys for the canvas while a grid has the focus (`Editor::routes_to_canvas`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use annota::document::annotation::Property;
use annota::editing::tool::Tool;
use annota::styles::library::StyleRef;
use annota::styles::tile;
use annota::styles::value::{self, TOOLS};

use super::properties::{Mode, PanelInfo};
use super::styles::{self as words, Styles};
use crate::ui::labels;

/// The tile size (redline: 63 × 48).
pub const TILE: (i32, i32) = (63, 48);
/// How long a newly saved style keeps its halo (redline: 2 s).
const HALO_MS: u64 = 2000;

/// What the user asked for.
#[derive(Clone)]
pub enum QuickEvent {
    /// Apply a style where the properties go.
    Apply(Vec<Property>),
    /// A favourite: switch to its tool and apply it (mockup 04 → 05).
    Favourite(Tool, Vec<Property>),
    UseTheme(String),
    ManageThemes,
    /// A toast; the closure is its Undo.
    Toast(String, Option<Rc<dyn Fn()>>),
}

impl std::fmt::Debug for QuickEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuickEvent::Apply(v) => write!(f, "Apply({} values)", v.len()),
            QuickEvent::Favourite(t, _) => write!(f, "Favourite({t:?})"),
            QuickEvent::UseTheme(id) => write!(f, "UseTheme({id})"),
            QuickEvent::ManageThemes => write!(f, "ManageThemes"),
            QuickEvent::Toast(t, undo) => write!(f, "Toast({t}, undo: {})", undo.is_some()),
        }
    }
}

/// One tile.
#[derive(Clone)]
struct Tile {
    style: StyleRef,
    tool: Tool,
    values: Vec<Property>,
    child: gtk::FlowBoxChild,
    star: gtk::Image,
}

/// What the quick styles show.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    tool: Tool,
    /// The style part of the current values.
    current: Vec<Property>,
}

/// Where the open context menu or rename popover is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Tool,
    Favourites,
}

type Handler = Box<dyn Fn(QuickEvent)>;

pub struct QuickStyles {
    styles: Rc<Styles>,
    switch: gtk::Box,
    tool_toggle: gtk::ToggleButton,
    favourites_toggle: gtk::ToggleButton,
    favourites_view: gtk::Box,
    favourites_groups: gtk::Box,
    stack: gtk::Stack,
    section: gtk::Box,
    theme_chips: gtk::DrawingArea,
    theme_name: gtk::Label,
    theme_switcher: gtk::Popover,
    theme_list: gtk::ListBox,
    theme_grid: gtk::FlowBox,
    user_caption: gtk::Label,
    user_grid: gtk::FlowBox,
    heading: gtk::Label,
    caption: gtk::Label,
    modified: gtk::Box,
    save_button: gtk::MenuButton,
    save_popover: gtk::Popover,
    save_preview: gtk::DrawingArea,
    save_summary: gtk::Label,
    save_where: gtk::Label,
    save_name: gtk::Entry,
    save_favourite: gtk::CheckButton,
    save_confirm: gtk::Button,
    rename_popover: gtk::Popover,
    rename_entry: gtk::Entry,
    menu: gtk::PopoverMenu,
    full_actions: gio::SimpleActionGroup,
    limited_actions: gio::SimpleActionGroup,
    // state
    shown: RefCell<Option<Shown>>,
    /// (tool, styles generation) the tool grids were built for.
    built: RefCell<Option<(Tool, u64)>>,
    favourites_built: Cell<Option<u64>>,
    tiles: RefCell<Vec<Tile>>,
    favourite_tiles: RefCell<Vec<Tile>>,
    last_applied: RefCell<Option<StyleRef>>,
    menu_target: RefCell<Option<(StyleRef, Place)>>,
    rename_target: RefCell<Option<String>>,
    /// A style just saved: its tile gets the halo when it is built.
    new_style: RefCell<Option<String>>,
    handler: RefCell<Option<Handler>>,
}

impl QuickStyles {
    pub fn new(builder: &gtk::Builder, styles: Rc<Styles>) -> Option<Rc<Self>> {
        let get = |name: &str| -> Option<glib::Object> {
            let object = builder.object(name);
            if object.is_none() {
                eprintln!("annota: editor-window.ui is missing object '{name}'");
            }
            object
        };
        macro_rules! obj {
            ($name:expr) => {
                get($name)?.downcast().ok()?
            };
        }
        let menu_model: gio::MenuModel = obj!("style_menu");
        let menu = gtk::PopoverMenu::builder()
            .menu_model(&menu_model)
            .has_arrow(true)
            .build();
        menu.add_css_class("annota-style-menu");
        let quick = Rc::new(QuickStyles {
            styles,
            switch: obj!("quick_switch"),
            tool_toggle: obj!("quick_tool_toggle"),
            favourites_toggle: obj!("quick_favourites_toggle"),
            favourites_view: obj!("favourites_view"),
            favourites_groups: obj!("favourites_groups"),
            stack: obj!("properties_stack"),
            section: obj!("quick_styles"),
            theme_chips: obj!("theme_chips"),
            theme_name: obj!("theme_name"),
            theme_switcher: obj!("theme_switcher"),
            theme_list: obj!("theme_list"),
            theme_grid: obj!("theme_grid"),
            user_caption: obj!("my_styles_caption"),
            user_grid: obj!("user_grid"),
            heading: obj!("selection_heading"),
            caption: obj!("selection_caption"),
            modified: obj!("style_modified"),
            save_button: obj!("save_style_button"),
            save_popover: obj!("save_style_popover"),
            save_preview: obj!("save_style_preview"),
            save_summary: obj!("save_style_summary"),
            save_where: obj!("save_style_where"),
            save_name: obj!("save_style_name"),
            save_favourite: obj!("save_style_favourite"),
            save_confirm: obj!("save_style_confirm"),
            rename_popover: obj!("style_rename_popover"),
            rename_entry: obj!("style_rename_entry"),
            menu,
            full_actions: gio::SimpleActionGroup::new(),
            limited_actions: gio::SimpleActionGroup::new(),
            shown: RefCell::new(None),
            built: RefCell::new(None),
            favourites_built: Cell::new(None),
            tiles: RefCell::new(Vec::new()),
            favourite_tiles: RefCell::new(Vec::new()),
            last_applied: RefCell::new(None),
            menu_target: RefCell::new(None),
            rename_target: RefCell::new(None),
            new_style: RefCell::new(None),
            handler: RefCell::new(None),
        });
        let save_cancel: gtk::Button = obj!("save_style_cancel");
        let rename_cancel: gtk::Button = obj!("style_rename_cancel");
        let rename_confirm: gtk::Button = obj!("style_rename_confirm");
        let manage: gtk::Button = obj!("manage_themes_button");
        quick.setup(&save_cancel, &rename_cancel, &rename_confirm, &manage);
        Some(quick)
    }

    pub fn connect_event(&self, f: impl Fn(QuickEvent) + 'static) {
        self.handler.replace(Some(Box::new(f)));
    }

    fn emit(&self, event: QuickEvent) {
        if let Some(f) = self.handler.borrow().as_ref() {
            f(event);
        }
    }

    // ---- setup ----

    fn setup(
        self: &Rc<Self>,
        save_cancel: &gtk::Button,
        rename_cancel: &gtk::Button,
        rename_confirm: &gtk::Button,
        manage: &gtk::Button,
    ) {
        let weak = Rc::downgrade(self);
        self.favourites_toggle.connect_toggled(move |_| {
            if let Some(q) = weak.upgrade() {
                q.show_page();
            }
        });
        for (grid, place) in [
            (self.theme_grid.clone(), Place::Tool),
            (self.user_grid.clone(), Place::Tool),
        ] {
            self.setup_grid(&grid, place);
        }
        // Theme switcher.
        let weak = Rc::downgrade(self);
        self.theme_chips.set_draw_func(move |_, cr, _, _| {
            if let Some(q) = weak.upgrade() {
                let theme = q.styles.current_theme();
                let _ = words::draw_chips(cr, &theme.colours, 4, 10.0, 10.0, 3.0);
            }
        });
        let weak = Rc::downgrade(self);
        self.theme_switcher.connect_show(move |_| {
            if let Some(q) = weak.upgrade() {
                q.fill_theme_list();
            }
        });
        let weak = Rc::downgrade(self);
        self.theme_list.connect_row_activated(move |_, row| {
            let Some(q) = weak.upgrade() else {
                return;
            };
            let id = row.widget_name().to_string();
            q.theme_switcher.popdown();
            q.emit(QuickEvent::UseTheme(id));
        });
        let weak = Rc::downgrade(self);
        manage.connect_clicked(move |_| {
            if let Some(q) = weak.upgrade() {
                q.theme_switcher.popdown();
                q.emit(QuickEvent::ManageThemes);
            }
        });
        // Save as Quick Style.
        let weak = Rc::downgrade(self);
        self.save_popover.connect_show(move |_| {
            if let Some(q) = weak.upgrade() {
                q.prepare_save();
            }
        });
        let weak = Rc::downgrade(self);
        self.save_preview.set_draw_func(move |_, cr, w, h| {
            if let Some(shown) = weak.upgrade().and_then(|q| q.shown.borrow().clone()) {
                let _ = tile::draw(cr, shown.tool, &shown.current, f64::from(w), f64::from(h));
            }
        });
        let weak = Rc::downgrade(self);
        self.save_name.connect_changed(move |entry| {
            if let Some(q) = weak.upgrade() {
                q.save_confirm
                    .set_sensitive(!entry.text().trim().is_empty());
            }
        });
        let popover = self.save_popover.downgrade();
        save_cancel.connect_clicked(move |_| {
            if let Some(p) = popover.upgrade() {
                p.popdown();
            }
        });
        let weak = Rc::downgrade(self);
        self.save_confirm.connect_clicked(move |_| {
            if let Some(q) = weak.upgrade() {
                q.save();
            }
        });
        // Rename.
        let popover = self.rename_popover.downgrade();
        rename_cancel.connect_clicked(move |_| {
            if let Some(p) = popover.upgrade() {
                p.popdown();
            }
        });
        let weak = Rc::downgrade(self);
        rename_confirm.connect_clicked(move |_| {
            if let Some(q) = weak.upgrade() {
                q.rename_confirmed();
            }
        });
        let weak = Rc::downgrade(self);
        self.rename_entry.connect_changed(move |entry| {
            if let Some(q) = weak.upgrade() {
                if let Some(button) = q.rename_popover.default_widget() {
                    button.set_sensitive(!entry.text().trim().is_empty());
                }
            }
        });
        self.setup_actions();
    }

    fn setup_grid(self: &Rc<Self>, grid: &gtk::FlowBox, place: Place) {
        let weak = Rc::downgrade(self);
        grid.connect_child_activated(move |_, child| {
            if let Some(q) = weak.upgrade() {
                if let Some(t) = q.tile_of(child, place) {
                    q.apply(&t, place);
                }
            }
        });
        // Right-click.
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = Rc::downgrade(self);
        let g = grid.downgrade();
        click.connect_pressed(move |gesture, _, x, y| {
            let (Some(q), Some(grid)) = (weak.upgrade(), g.upgrade()) else {
                return;
            };
            if let Some(child) = grid.child_at_pos(x as i32, y as i32) {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                child.grab_focus();
                q.open_menu(&child, place);
            }
        });
        grid.add_controller(click);
        // Long press (touch).
        let press = gtk::GestureLongPress::new();
        let weak = Rc::downgrade(self);
        let g = grid.downgrade();
        press.connect_pressed(move |_, x, y| {
            let (Some(q), Some(grid)) = (weak.upgrade(), g.upgrade()) else {
                return;
            };
            if let Some(child) = grid.child_at_pos(x as i32, y as i32) {
                q.open_menu(&child, place);
            }
        });
        grid.add_controller(press);
        // Keys (capture: Ctrl+←/→ before the grid moves the focus).
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        let g = grid.downgrade();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let (Some(q), Some(grid)) = (weak.upgrade(), g.upgrade()) else {
                return glib::Propagation::Proceed;
            };
            if q.grid_key(&grid, key, modifiers, place) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        grid.add_controller(keys);
    }

    fn setup_actions(self: &Rc<Self>) {
        let add =
            |group: &gio::SimpleActionGroup, name: &str, f: fn(&Rc<Self>, &StyleRef, Place)| {
                let action = gio::SimpleAction::new(name, None);
                let weak = Rc::downgrade(self);
                action.connect_activate(move |_, _| {
                    let Some(q) = weak.upgrade() else {
                        return;
                    };
                    let target = q.menu_target.borrow().clone();
                    if let Some((style, place)) = target {
                        f(&q, &style, place);
                    }
                });
                group.add_action(&action);
            };
        for group in [&self.full_actions, &self.limited_actions] {
            add(group, "apply", |q, style, place| {
                if let Some(t) = q.find_tile(style, place) {
                    q.apply(&t, place);
                }
            });
            add(group, "star", |q, style, _| q.set_favourite(style, true));
            add(group, "unstar", |q, style, _| q.set_favourite(style, false));
        }
        let full = &self.full_actions;
        add(full, "rename", |q, style, place| {
            q.start_rename(style, place)
        });
        add(full, "move-left", |q, style, _| q.move_style(style, -1));
        add(full, "move-right", |q, style, _| q.move_style(style, 1));
        add(full, "update", |q, style, _| q.update_style(style));
        add(full, "delete", |q, style, _| q.delete_style(style));
        self.menu
            .insert_action_group("style", Some(&self.full_actions));
    }

    // ---- showing ----

    /// Shows the quick styles for `info` (the type properties), or hides them.
    pub fn show(self: &Rc<Self>, info: Option<&PanelInfo>) {
        let shown = info
            .filter(|i| {
                matches!(
                    i.mode,
                    Mode::Selected(_) | Mode::ToolDefaults | Mode::Editing
                )
            })
            .and_then(|i| {
                let tool = value::tool_of_type(i.type_name)?;
                Some(Shown {
                    tool,
                    current: value::style_of(tool, &i.values),
                })
            });
        let Some(shown) = shown else {
            self.shown.replace(None);
            self.switch.set_visible(false);
            self.section.set_visible(false);
            self.save_button.set_visible(false);
            self.modified.set_visible(false);
            self.caption.set_visible(true);
            self.show_page();
            return;
        };
        self.shown.replace(Some(shown.clone()));
        self.switch.set_visible(true);
        self.section.set_visible(true);
        self.save_button.set_visible(true);
        self.tool_toggle.set_label(&labels::tool_name(shown.tool));
        self.theme_name.set_text(&self.styles.current_theme().name);
        self.theme_chips.queue_draw();
        let generation = self.styles.generation();
        if *self.built.borrow() != Some((shown.tool, generation)) {
            self.build_grids(shown.tool);
            self.built.replace(Some((shown.tool, generation)));
        }
        let selected = self.selected_style();
        for t in self.tiles.borrow().iter() {
            mark(
                &t.child,
                "annota-style-selected",
                Some(&t.style) == selected.as_ref(),
            );
        }
        let modified = selected.is_none();
        // Redline: "Properties" + "Modified" (or the mode, as add-core-annotation-tools had it).
        self.heading.set_text(&gettext("Properties"));
        self.modified.set_visible(modified);
        self.caption.set_visible(!modified);
        self.save_button.set_sensitive(modified);
        if !modified && self.save_popover.is_visible() {
            self.save_popover.popdown();
        }
        self.show_page();
    }

    /// The Favourites page or the tool's page.
    fn show_page(self: &Rc<Self>) {
        let favourites = self.switch.is_visible() && self.favourites_toggle.is_active();
        self.favourites_view.set_visible(favourites);
        self.stack.set_visible(!favourites);
        if favourites {
            let generation = self.styles.generation();
            if self.favourites_built.get() != Some(generation) {
                self.build_favourites();
                self.favourites_built.set(Some(generation));
            }
        }
    }

    /// The style the current values are: the one applied last if it still matches, else the
    /// first matching tile.
    fn selected_style(&self) -> Option<StyleRef> {
        let shown = self.shown.borrow();
        let shown = shown.as_ref()?;
        let tiles = self.tiles.borrow();
        let matching: Vec<&Tile> = tiles
            .iter()
            .filter(|t| value::matches(&t.values, &shown.current))
            .collect();
        let last = self.last_applied.borrow();
        matching
            .iter()
            .find(|t| Some(&t.style) == last.as_ref())
            .or(matching.first())
            .map(|t| t.style.clone())
    }

    fn build_grids(self: &Rc<Self>, tool: Tool) {
        clear(&self.theme_grid);
        clear(&self.user_grid);
        let mut tiles = Vec::new();
        let theme = self.styles.current_theme();
        let generated = self.styles.generated(&theme, tool);
        for (slot, values) in generated.iter().enumerate() {
            let style = StyleRef::Theme {
                theme: theme.id.clone(),
                tool,
                slot,
            };
            let tooltip = words::description(tool, values);
            let t = self.tile(style, tool, values.clone(), &tooltip);
            self.theme_grid.append(&t.child);
            tiles.push(t);
        }
        let users: Vec<(String, String, Vec<Property>)> = self
            .styles
            .library()
            .styles(tool)
            .iter()
            .map(|s| (s.id.clone(), s.name.clone(), s.values.clone()))
            .collect();
        self.user_caption.set_visible(!users.is_empty());
        self.user_grid.set_visible(!users.is_empty());
        let new_style = self.new_style.take();
        for (id, name, values) in users {
            let tooltip = format!("{name}\n{}", words::description(tool, &values));
            let t = self.tile(StyleRef::User(id.clone()), tool, values, &tooltip);
            t.child
                .update_property(&[gtk::accessible::Property::Label(&name)]);
            self.user_grid.append(&t.child);
            if new_style.as_deref() == Some(id.as_str()) {
                halo(&t.child);
            }
            tiles.push(t);
        }
        self.tiles.replace(tiles);
    }

    fn build_favourites(self: &Rc<Self>) {
        while let Some(child) = self.favourites_groups.first_child() {
            self.favourites_groups.remove(&child);
        }
        let favourites: Vec<StyleRef> = self
            .styles
            .library()
            .favourites()
            .into_iter()
            .cloned()
            .collect();
        let resolved: Vec<(StyleRef, words::Resolved)> = favourites
            .into_iter()
            .filter_map(|r| self.styles.resolve(&r).map(|res| (r, res)))
            .collect();
        let mut tiles = Vec::new();
        for tool in TOOLS {
            let mine: Vec<&(StyleRef, words::Resolved)> =
                resolved.iter().filter(|(_, r)| r.tool == tool).collect();
            if mine.is_empty() {
                continue;
            }
            let group = gtk::Box::new(gtk::Orientation::Vertical, 8);
            let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let icon = gtk::Image::from_icon_name(super::tool_icon(tool));
            icon.set_pixel_size(14);
            icon.add_css_class("dim-label");
            heading.append(&icon);
            let label = gtk::Label::new(Some(&labels::tool_name(tool)));
            label.add_css_class("dim-label");
            label.add_css_class("annota-favourites-caption");
            heading.append(&label);
            group.append(&heading);
            let grid = gtk::FlowBox::builder()
                .halign(gtk::Align::Start)
                .min_children_per_line(4)
                .max_children_per_line(4)
                .column_spacing(8)
                .row_spacing(8)
                .homogeneous(true)
                .selection_mode(gtk::SelectionMode::None)
                .activate_on_single_click(true)
                .css_classes(["annota-style-grid"])
                .build();
            grid.update_property(&[gtk::accessible::Property::Label(&labels::tool_name(tool))]);
            self.setup_grid(&grid, Place::Favourites);
            for (style, res) in mine {
                let mut tooltip = words::favourite_description(tool, &res.values);
                if let Some(name) = &res.name {
                    tooltip = format!("{name}\n{tooltip}");
                }
                let t = self.tile(style.clone(), tool, res.values.clone(), &tooltip);
                grid.append(&t.child);
                tiles.push(t);
            }
            group.append(&grid);
            self.favourites_groups.append(&group);
        }
        self.favourites_groups.set_visible(!tiles.is_empty());
        self.favourite_tiles.replace(tiles);
    }

    fn tile(&self, style: StyleRef, tool: Tool, values: Vec<Property>, tooltip: &str) -> Tile {
        // 2 px inside the tile's ring (style.css).
        let area = gtk::DrawingArea::builder()
            .content_width(TILE.0 - 4)
            .content_height(TILE.1 - 4)
            .margin_top(2)
            .margin_bottom(2)
            .margin_start(2)
            .margin_end(2)
            .can_target(false)
            .build();
        let drawn = values.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let _ = tile::draw(cr, tool, &drawn, f64::from(w), f64::from(h));
        });
        let star = gtk::Image::builder()
            .icon_name("starred-symbolic")
            .pixel_size(12)
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .margin_top(2)
            .margin_end(2)
            .can_target(false)
            .css_classes(["annota-style-star"])
            .visible(self.styles.library().is_favourite(&style))
            .build();
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&area));
        overlay.add_overlay(&star);
        let child = gtk::FlowBoxChild::builder()
            .child(&overlay)
            .tooltip_text(tooltip)
            .css_classes(["annota-style-tile"])
            .overflow(gtk::Overflow::Hidden)
            .build();
        child.update_property(&[gtk::accessible::Property::Label(
            tooltip.lines().next().unwrap_or_default(),
        )]);
        Tile {
            style,
            tool,
            values,
            child,
            star,
        }
    }

    fn tile_of(&self, child: &gtk::FlowBoxChild, place: Place) -> Option<Tile> {
        let tiles = match place {
            Place::Tool => self.tiles.borrow(),
            Place::Favourites => self.favourite_tiles.borrow(),
        };
        tiles.iter().find(|t| &t.child == child).cloned()
    }

    fn find_tile(&self, style: &StyleRef, place: Place) -> Option<Tile> {
        let tiles = match place {
            Place::Tool => self.tiles.borrow(),
            Place::Favourites => self.favourite_tiles.borrow(),
        };
        tiles.iter().find(|t| &t.style == style).cloned()
    }

    fn fill_theme_list(self: &Rc<Self>) {
        clear_list(&self.theme_list);
        let current = self.styles.current_theme().id;
        let themes = self.styles.themes().clone();
        for theme in themes {
            let row = gtk::ListBoxRow::new();
            row.set_widget_name(&theme.id);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.set_margin_top(6);
            line.set_margin_bottom(6);
            line.set_margin_start(6);
            line.set_margin_end(6);
            let chips = gtk::DrawingArea::builder()
                .content_width(46)
                .content_height(10)
                .valign(gtk::Align::Center)
                .build();
            let colours = theme.colours.clone();
            chips.set_draw_func(move |_, cr, _, _| {
                let _ = words::draw_chips(cr, &colours, 4, 10.0, 10.0, 3.0);
            });
            line.append(&chips);
            let name = gtk::Label::builder()
                .label(&theme.name)
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            line.append(&name);
            if theme.origin == annota::styles::theme::Origin::Managed {
                let lock = gtk::Image::from_icon_name("changes-prevent-symbolic");
                lock.set_tooltip_text(Some(&gettext("Managed by your administrator")));
                line.append(&lock);
            }
            if theme.id == current {
                line.append(&gtk::Image::from_icon_name("object-select-symbolic"));
            }
            row.set_child(Some(&line));
            row.update_property(&[gtk::accessible::Property::Label(&theme.name)]);
            self.theme_list.append(&row);
        }
    }

    // ---- doing ----

    fn apply(self: &Rc<Self>, t: &Tile, place: Place) {
        self.last_applied.replace(Some(t.style.clone()));
        match place {
            Place::Tool => self.emit(QuickEvent::Apply(t.values.clone())),
            Place::Favourites => {
                // Mockup 05: back to the tool's page, its tool active.
                self.tool_toggle.set_active(true);
                self.emit(QuickEvent::Favourite(t.tool, t.values.clone()));
            }
        }
    }

    fn set_favourite(self: &Rc<Self>, style: &StyleRef, on: bool) {
        self.styles.change(|l| l.set_favourite(style, on));
        self.refresh_stars();
    }

    fn refresh_stars(&self) {
        let library = self.styles.library();
        for t in self
            .tiles
            .borrow()
            .iter()
            .chain(self.favourite_tiles.borrow().iter())
        {
            t.star.set_visible(library.is_favourite(&t.style));
        }
    }

    fn move_style(self: &Rc<Self>, style: &StyleRef, delta: isize) {
        let StyleRef::User(id) = style else {
            return;
        };
        if self.styles.change(|l| l.move_by(id, delta)) {
            self.refresh_now();
            self.focus_tile(style);
        }
    }

    fn update_style(self: &Rc<Self>, style: &StyleRef) {
        let StyleRef::User(id) = style else {
            return;
        };
        let Some(current) = self.shown.borrow().as_ref().map(|s| s.current.clone()) else {
            return;
        };
        self.last_applied.replace(Some(style.clone()));
        self.styles.change(|l| l.update(id, &current));
        self.refresh_now();
    }

    fn delete_style(self: &Rc<Self>, style: &StyleRef) {
        let StyleRef::User(id) = style else {
            return;
        };
        // Keyboard: the focus goes to a neighbour.
        let neighbour = {
            let tiles = self.tiles.borrow();
            let users: Vec<&Tile> = tiles
                .iter()
                .filter(|t| matches!(t.style, StyleRef::User(_)))
                .collect();
            let at = users.iter().position(|t| &t.style == style);
            at.and_then(|i| {
                users
                    .get(i + 1)
                    .or_else(|| i.checked_sub(1).and_then(|j| users.get(j)))
            })
            .map(|t| t.style.clone())
        };
        let Some(removed) = self.styles.change(|l| l.remove(id)) else {
            return;
        };
        self.refresh_now();
        match neighbour {
            Some(n) => self.focus_tile(&n),
            None => {
                if let Some(first) = self.theme_grid.child_at_index(0) {
                    first.grab_focus();
                }
            }
        }
        let title = gettext("Quick style “{name}” deleted").replace("{name}", &removed.style.name);
        let styles = Rc::downgrade(&self.styles);
        let undo: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(styles) = styles.upgrade() {
                let removed = removed.clone();
                styles.change(|l| l.restore(removed));
            }
        });
        self.emit(QuickEvent::Toast(title, Some(undo)));
    }

    fn start_rename(self: &Rc<Self>, style: &StyleRef, place: Place) {
        let StyleRef::User(id) = style else {
            return;
        };
        let Some(name) = self.styles.library().get(id).map(|s| s.name.clone()) else {
            return;
        };
        let Some(t) = self.find_tile(style, place) else {
            return;
        };
        self.rename_target.replace(Some(id.clone()));
        self.point_at(self.rename_popover.upcast_ref(), &t.child);
        self.rename_entry.set_text(&name);
        self.rename_popover.popup();
        self.rename_entry.grab_focus();
        self.rename_entry.select_region(0, -1);
    }

    fn rename_confirmed(self: &Rc<Self>) {
        let Some(id) = self.rename_target.take() else {
            return;
        };
        let name = self.rename_entry.text().to_string();
        self.rename_popover.popdown();
        if self.styles.change(|l| l.rename(&id, &name)) {
            self.refresh_now();
        }
        self.focus_tile(&StyleRef::User(id));
    }

    fn prepare_save(self: &Rc<Self>) {
        let Some(shown) = self.shown.borrow().clone() else {
            return;
        };
        let theme = self.styles.current_theme();
        let base = self
            .styles
            .generated(&theme, shown.tool)
            .first()
            .cloned()
            .unwrap_or_default();
        let suggested = words::suggested_name(shown.tool, &shown.current, &base);
        let name = self.styles.library().unique_name(shown.tool, &suggested);
        self.save_summary
            .set_text(&words::description(shown.tool, &shown.current));
        self.save_where.set_text(
            &gettext("Added to the end of {tool}")
                .replace("{tool}", &labels::tool_name(shown.tool)),
        );
        self.save_name.set_text(&name);
        self.save_confirm.set_sensitive(!name.trim().is_empty());
        self.save_preview.queue_draw();
        self.save_name.grab_focus();
        self.save_name.select_region(0, -1);
    }

    /// Saves the current values as a new style at the end of the tool's (scenario "Save
    /// style"): selected, with a halo for 2 s and a toast with Undo.
    pub fn save(self: &Rc<Self>) {
        let Some(shown) = self.shown.borrow().clone() else {
            return;
        };
        let name = self.save_name.text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let favourite = self.save_favourite.is_active();
        let id = self.styles.change(|l| {
            let id = l.add(shown.tool, &name, shown.current.clone());
            if favourite {
                l.set_favourite(&StyleRef::User(id.clone()), true);
            }
            id
        });
        self.save_popover.popdown();
        self.last_applied.replace(Some(StyleRef::User(id.clone())));
        self.new_style.replace(Some(id.clone()));
        self.refresh_now();
        let title = gettext("Quick style “{name}” saved").replace("{name}", &name);
        let styles = Rc::downgrade(&self.styles);
        let undo: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(styles) = styles.upgrade() {
                styles.change(|l| l.remove(&id));
            }
        });
        self.emit(QuickEvent::Toast(title, Some(undo)));
    }

    /// Rebuilds now (the library changed under what is shown).
    fn refresh_now(self: &Rc<Self>) {
        let shown = self.shown.borrow().clone();
        if let Some(shown) = shown {
            self.built.replace(None);
            let generation = self.styles.generation();
            self.build_grids(shown.tool);
            self.built.replace(Some((shown.tool, generation)));
            let selected = self.selected_style();
            for t in self.tiles.borrow().iter() {
                mark(
                    &t.child,
                    "annota-style-selected",
                    Some(&t.style) == selected.as_ref(),
                );
            }
            let modified = selected.is_none();
            self.modified.set_visible(modified);
            self.caption.set_visible(!modified);
            self.save_button.set_sensitive(modified);
        }
        if self.favourites_view.is_visible() {
            self.build_favourites();
            self.favourites_built.set(Some(self.styles.generation()));
        }
    }

    fn focus_tile(&self, style: &StyleRef) {
        if let Some(t) = self.find_tile(style, Place::Tool) {
            t.child.grab_focus();
        }
    }

    /// Parents `popover` to the quick styles section (or the Favourites view) and points it
    /// at `child`.
    fn point_at(&self, popover: &gtk::Popover, child: &gtk::FlowBoxChild) {
        let host: gtk::Widget = if child.is_ancestor(&self.favourites_view) {
            self.favourites_view.clone().upcast()
        } else {
            self.section.clone().upcast()
        };
        if popover.parent().as_ref() != Some(&host) {
            if popover.parent().is_some() {
                popover.unparent();
            }
            popover.set_parent(&host);
        }
        if let Some(bounds) = child.compute_bounds(&host) {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            )));
        }
    }

    /// The context menu of a tile (mockup 03): the full one for the user's own styles in the
    /// tool's grid, Apply and the favourite item for theme styles and in Favourites.
    fn open_menu(self: &Rc<Self>, child: &gtk::FlowBoxChild, place: Place) {
        let Some(t) = self.tile_of(child, place) else {
            return;
        };
        let favourite = self.styles.library().is_favourite(&t.style);
        let full = matches!(t.style, StyleRef::User(_)) && place == Place::Tool;
        let group = if full {
            &self.full_actions
        } else {
            &self.limited_actions
        };
        let enable = |name: &str, on: bool| {
            if let Some(action) = group
                .lookup_action(name)
                .and_downcast::<gio::SimpleAction>()
            {
                action.set_enabled(on);
            }
        };
        enable("star", !favourite);
        enable("unstar", favourite);
        if let StyleRef::User(id) = &t.style {
            let library = self.styles.library();
            enable("move-left", library.can_move(id, -1));
            enable("move-right", library.can_move(id, 1));
        }
        self.menu_target.replace(Some((t.style.clone(), place)));
        self.menu.insert_action_group("style", Some(group));
        self.point_at(self.menu.upcast_ref(), child);
        self.menu.popup();
    }

    /// The grid's keys (redline "Keyboard"). Returns whether the key was used.
    fn grid_key(
        self: &Rc<Self>,
        grid: &gtk::FlowBox,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
        place: Place,
    ) -> bool {
        let Some(child) = grid.focus_child().and_downcast::<gtk::FlowBoxChild>() else {
            return false;
        };
        let Some(t) = self.tile_of(&child, place) else {
            return false;
        };
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let user = matches!(t.style, StyleRef::User(_)) && place == Place::Tool;
        match key {
            gdk::Key::Menu => {
                self.open_menu(&child, place);
                true
            }
            gdk::Key::F10 if shift => {
                self.open_menu(&child, place);
                true
            }
            gdk::Key::F2 if user => {
                self.start_rename(&t.style, place);
                true
            }
            gdk::Key::Delete | gdk::Key::KP_Delete if user => {
                self.delete_style(&t.style);
                true
            }
            gdk::Key::Left | gdk::Key::KP_Left if ctrl && user => {
                self.move_style(&t.style, -1);
                true
            }
            gdk::Key::Right | gdk::Key::KP_Right if ctrl && user => {
                self.move_style(&t.style, 1);
                true
            }
            _ => false,
        }
    }

    // ---- for tests ----

    /// (tooltip, selected, starred) of the tool grids' tiles.
    #[cfg(test)]
    pub fn tiles(&self) -> Vec<(String, bool, bool)> {
        self.tiles
            .borrow()
            .iter()
            .map(|t| {
                (
                    t.child
                        .tooltip_text()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    t.child.has_css_class("annota-style-selected"),
                    t.star.is_visible(),
                )
            })
            .collect()
    }

    /// Activates tile `index` of the tool grids (theme tiles first) as a click would.
    #[cfg(test)]
    pub fn activate(self: &Rc<Self>, index: usize) {
        let tile = self.tiles.borrow().get(index).cloned();
        if let Some(t) = tile {
            self.apply(&t, Place::Tool);
        }
    }

    /// Opens the context menu of tile `index` and runs `action` from it.
    #[cfg(test)]
    pub fn menu_action(self: &Rc<Self>, index: usize, action: &str) -> bool {
        let tile = self.tiles.borrow().get(index).cloned();
        let Some(t) = tile else {
            return false;
        };
        self.open_menu(&t.child, Place::Tool);
        self.menu.popdown();
        let full = matches!(t.style, StyleRef::User(_));
        let group = if full {
            &self.full_actions
        } else {
            &self.limited_actions
        };
        match group.lookup_action(action) {
            Some(a) if a.is_enabled() => {
                a.activate(None);
                true
            }
            _ => false,
        }
    }

    /// Sends `key` to the grid of tile `index` as if that tile had the focus.
    #[cfg(test)]
    pub fn press_on(
        self: &Rc<Self>,
        index: usize,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> bool {
        let tile = self.tiles.borrow().get(index).cloned();
        let Some(t) = tile else {
            return false;
        };
        let Some(grid) = t.child.parent().and_downcast::<gtk::FlowBox>() else {
            return false;
        };
        t.child.grab_focus();
        grid.set_focus_child(Some(&t.child));
        self.grid_key(&grid, key, modifiers, Place::Tool)
    }

    #[cfg(test)]
    pub fn show_favourites(self: &Rc<Self>, on: bool) {
        if on {
            self.favourites_toggle.set_active(true);
        } else {
            self.tool_toggle.set_active(true);
        }
    }

    /// (tool caption, tile count) per group of the Favourites view, and whether it shows.
    #[cfg(test)]
    pub fn favourites(&self) -> (bool, Vec<(String, usize)>) {
        let mut groups = Vec::new();
        let mut child = self.favourites_groups.first_child();
        while let Some(group) = child {
            let caption = group
                .first_child()
                .and_then(|h| h.last_child())
                .and_downcast::<gtk::Label>()
                .map(|l| l.text().to_string())
                .unwrap_or_default();
            let n = group
                .last_child()
                .and_downcast::<gtk::FlowBox>()
                .map(|g| {
                    let mut n = 0;
                    while g.child_at_index(n).is_some() {
                        n += 1;
                    }
                    n as usize
                })
                .unwrap_or(0);
            groups.push((caption, n));
            child = group.next_sibling();
        }
        (self.favourites_view.is_visible(), groups)
    }

    /// Activates favourite `index` (in view order).
    #[cfg(test)]
    pub fn activate_favourite(self: &Rc<Self>, index: usize) {
        let tile = self.favourite_tiles.borrow().get(index).cloned();
        if let Some(t) = tile {
            self.apply(&t, Place::Favourites);
        }
    }

    /// The save popover's fields, for tests: opens it and returns (name, summary, where).
    #[cfg(test)]
    pub fn open_save(self: &Rc<Self>) -> (String, String, String) {
        self.save_button.popup();
        self.prepare_save();
        (
            self.save_name.text().to_string(),
            self.save_summary.text().to_string(),
            self.save_where.text().to_string(),
        )
    }

    /// The text colour of the context menu item `label` of tile `index` (redline: Delete in
    /// the destructive colour).
    #[cfg(test)]
    pub fn menu_item_colour(self: &Rc<Self>, index: usize, label: &str) -> Option<gdk::RGBA> {
        fn find(w: &gtk::Widget, label: &str) -> Option<gtk::Widget> {
            if w.type_().name() == "GtkModelButton"
                && w.property::<Option<String>>("text").as_deref() == Some(label)
            {
                return Some(w.clone());
            }
            let mut c = w.first_child();
            while let Some(x) = c {
                if let Some(found) = find(&x, label) {
                    return Some(found);
                }
                c = x.next_sibling();
            }
            None
        }
        let tile = self.tiles.borrow().get(index).cloned()?;
        self.open_menu(&tile.child, Place::Tool);
        let colour = find(self.menu.upcast_ref(), label).map(|w| w.color());
        self.menu.popdown();
        colour
    }

    /// Whether the context menu is open; closes it.
    #[cfg(test)]
    pub fn menu_shown(&self) -> bool {
        let shown = self.menu.is_visible();
        self.menu.popdown();
        shown
    }

    #[cfg(test)]
    pub fn save_enabled(&self) -> bool {
        self.save_button.is_sensitive() && self.save_button.is_visible()
    }

    #[cfg(test)]
    pub fn modified_shown(&self) -> bool {
        self.modified.is_visible()
    }

    #[cfg(test)]
    pub fn has_halo(&self, index: usize) -> bool {
        self.tiles
            .borrow()
            .get(index)
            .is_some_and(|t| t.child.has_css_class("annota-style-new"))
    }

    #[cfg(test)]
    pub fn rename_to(self: &Rc<Self>, name: &str) {
        self.rename_entry.set_text(name);
        self.rename_confirmed();
    }

    #[cfg(test)]
    pub fn theme_rows(self: &Rc<Self>) -> Vec<String> {
        self.fill_theme_list();
        let mut out = Vec::new();
        let mut i = 0;
        while let Some(row) = self.theme_list.row_at_index(i) {
            out.push(row.widget_name().to_string());
            i += 1;
        }
        out
    }

    #[cfg(test)]
    pub fn pick_theme(self: &Rc<Self>, index: i32) {
        self.fill_theme_list();
        if let Some(row) = self.theme_list.row_at_index(index) {
            row.activate();
        }
    }

    #[cfg(test)]
    pub fn tool_label(&self) -> String {
        self.tool_toggle
            .label()
            .map(|s| s.to_string())
            .unwrap_or_default()
    }
}

fn clear(grid: &gtk::FlowBox) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
}

fn clear_list(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn mark(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
}

/// The newly saved tile's halo (redline: 4 px accent 25 %, fades after 2 s).
fn halo(child: &gtk::FlowBoxChild) {
    child.add_css_class("annota-style-new");
    let weak = child.downgrade();
    glib::timeout_add_local_once(std::time::Duration::from_millis(HALO_MS), move || {
        if let Some(child) = weak.upgrade() {
            child.remove_css_class("annota-style-new");
        }
    });
}
