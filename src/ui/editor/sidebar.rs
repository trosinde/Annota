//! The properties sidebar (redline "Sidebar contents"; mockups 01, 06, 16): the document when
//! nothing is selected (Image, Canvas, Objects; a per-type summary above 40 objects), the
//! selection otherwise (type heading, group members and Ungroup, Arrange, Select All <Type>).
//!
//! What it shows is computed by [`state`] from the document and the selection, without
//! widgets. Type-specific property panels (colour, width, line style …) belong to
//! add-core-annotation-tools and slot in between the heading and Arrange.

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use gettextrs::gettext;
use gtk::glib;

use annota::document::color::{palette, Rgba};
use annota::document::geometry::{Arrange, Rect};
use annota::document::scene::ObjectId;
use annota::document::{Background, Document};
use annota::editing::selection::is_selectable;
use annota::tools::group::Group;
use annota::tools::image::Image;

use super::text;
use crate::application::accel_label;
use crate::ui::labels::object_name;

/// Redline "Many objects": the summary appears above this many objects.
pub const SUMMARY_THRESHOLD: usize = 40;

/// The background swatches of the Canvas group (redline: 7 palette colours, Transparent,
/// Paper, Custom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swatch {
    Coral,
    Amber,
    Lime,
    Teal,
    Azure,
    Violet,
    Ink,
    Transparent,
    Paper,
    Custom,
}

impl Swatch {
    const PALETTE: [(Swatch, Rgba); 8] = [
        (Swatch::Coral, palette::CORAL),
        (Swatch::Amber, palette::AMBER),
        (Swatch::Lime, palette::LIME),
        (Swatch::Teal, palette::TEAL),
        (Swatch::Azure, palette::AZURE),
        (Swatch::Violet, palette::VIOLET),
        (Swatch::Ink, palette::INK),
        (Swatch::Paper, palette::PAPER),
    ];

    /// The swatch that shows `background` as current, if any.
    pub fn current(background: Background) -> Option<Swatch> {
        match background {
            Background::Transparent => Some(Swatch::Transparent),
            Background::Color { color } => Some(
                Self::PALETTE
                    .iter()
                    .find(|(_, c)| *c == color)
                    .map_or(Swatch::Custom, |(s, _)| *s),
            ),
            // A background type of a newer Annota: none of these.
            Background::Unknown => None,
        }
    }

    fn widget_name(self) -> &'static str {
        match self {
            Swatch::Coral => "swatch_coral",
            Swatch::Amber => "swatch_amber",
            Swatch::Lime => "swatch_lime",
            Swatch::Teal => "swatch_teal",
            Swatch::Azure => "swatch_azure",
            Swatch::Violet => "swatch_violet",
            Swatch::Ink => "swatch_ink",
            Swatch::Transparent => "swatch_transparent",
            Swatch::Paper => "swatch_paper",
            Swatch::Custom => "swatch_custom",
        }
    }

    const ALL: [Swatch; 10] = [
        Swatch::Coral,
        Swatch::Amber,
        Swatch::Lime,
        Swatch::Teal,
        Swatch::Azure,
        Swatch::Violet,
        Swatch::Ink,
        Swatch::Transparent,
        Swatch::Paper,
        Swatch::Custom,
    ];
}

/// Nothing selected.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentInfo {
    pub size: (i32, i32),
    pub corner_radius: f64,
    pub background: Background,
    /// Editable (top-level) objects.
    pub objects: usize,
    /// Selectable objects per type, most first (the summary of mockup 16).
    pub by_type: Vec<(&'static str, usize)>,
}

/// Something selected.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionInfo {
    pub count: usize,
    /// The type all selected objects share.
    pub common: Option<&'static str>,
    /// Member types of a single selected group (the member list of mockup 06).
    pub members: Vec<&'static str>,
    /// The box the Arrange entries show: the object's own for one, the union for several.
    pub frame: Arrange,
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Empty,
    Document(DocumentInfo),
    Selection(SelectionInfo),
}

/// What the sidebar shows for `doc` with `selection` selected.
pub fn state(doc: &Document, selection: &[ObjectId]) -> State {
    let scene = doc.scene();
    let selected: Vec<_> = selection.iter().filter_map(|&id| scene.get(id)).collect();
    if selected.is_empty() {
        let mut by_type: Vec<(&'static str, usize)> = Vec::new();
        for node in scene.nodes() {
            if !is_selectable(node.object.as_ref()) {
                continue;
            }
            let t = node.object.type_name();
            match by_type.iter_mut().find(|(name, _)| *name == t) {
                Some((_, n)) => *n += 1,
                None => by_type.push((t, 1)),
            }
        }
        by_type.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let canvas = doc.canvas();
        return State::Document(DocumentInfo {
            size: (canvas.rect.width, canvas.rect.height),
            corner_radius: doc.image().corner_radius,
            background: canvas.background,
            objects: doc.object_count(),
            by_type,
        });
    }
    let first = selected[0].object.type_name();
    let common = selected
        .iter()
        .all(|n| n.object.type_name() == first)
        .then_some(first);
    let (frame, members) = match selected.as_slice() {
        [one] => (
            one.object.arrange(),
            one.object
                .children()
                .iter()
                .map(|c| c.object.type_name())
                .collect(),
        ),
        many => (
            Arrange::new(
                many.iter()
                    .fold(Rect::NONE, |acc, n| acc.union(&n.object.arrange().bounds())),
                0.0,
            ),
            Vec::new(),
        ),
    };
    State::Selection(SelectionInfo {
        count: selected.len(),
        common,
        members,
        frame,
    })
}

/// Parses an Arrange entry ("580", "12.5"); `None` if it is not a finite number.
pub fn parse_number(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Shows a number the way the Arrange entries do: whole pixels without decimals.
pub fn format_number(v: f64) -> String {
    let rounded = (v * 10.0).round() / 10.0;
    // No "-0".
    let rounded = if rounded == 0.0 { 0.0 } else { rounded };
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

fn type_icon(type_name: &str) -> &'static str {
    match type_name {
        Image::TYPE => "image-x-generic-symbolic",
        Group::TYPE => "edit-select-all-symbolic",
        _ => "image-missing-symbolic",
    }
}

type ArrangeHandler = Box<dyn Fn(Arrange)>;

/// The sidebar's widgets (from `editor-window.ui`).
pub struct Sidebar {
    pub properties_box: gtk::Box,
    pub(super) stack: gtk::Stack,
    pub(super) summary_group: gtk::Box,
    pub(super) summary_caption: gtk::Label,
    pub(super) summary_list: gtk::ListBox,
    pub(super) summary_key: RefCell<Vec<(&'static str, usize)>>,
    pub(super) size_row: adw::ActionRow,
    pub(super) corner_radius_row: adw::SpinRow,
    pub(super) swatches: Vec<(Swatch, gtk::Button)>,
    pub snap_row: adw::SwitchRow,
    pub(super) objects_row: adw::ActionRow,
    pub(super) selection_heading: gtk::Label,
    pub(super) selection_caption: gtk::Label,
    pub(super) members_list: gtk::ListBox,
    pub(super) ungroup_button: gtk::Button,
    pub(super) entries: [gtk::Entry; 4],
    pub(super) rotation: gtk::SpinButton,
    pub(super) select_all_type_button: gtk::Button,
    pub(super) group_note: gtk::Label,
    /// The selection the Arrange entries were filled from.
    pub(super) frame: Cell<Option<(Arrange, bool)>>,
    /// Set while the widgets are filled in, so that does not count as user input.
    pub(super) updating: Cell<bool>,
    pub(super) on_arrange: RefCell<Option<ArrangeHandler>>,
}

impl Sidebar {
    pub fn new(builder: &gtk::Builder) -> Option<std::rc::Rc<Self>> {
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
        let mut swatches = Vec::new();
        for swatch in Swatch::ALL {
            swatches.push((swatch, obj!(swatch.widget_name())));
        }
        let ungroup_accel: gtk::Label = obj!("ungroup_accel");
        ungroup_accel.set_text(&accel_label("<Control><Shift>g"));
        let sidebar = std::rc::Rc::new(Self {
            properties_box: obj!("properties_box"),
            stack: obj!("properties_stack"),
            summary_group: obj!("summary_group"),
            summary_caption: obj!("summary_caption"),
            summary_list: obj!("summary_list"),
            summary_key: RefCell::new(Vec::new()),
            size_row: obj!("size_row"),
            corner_radius_row: obj!("corner_radius_row"),
            swatches,
            snap_row: obj!("snap_row"),
            objects_row: obj!("objects_row"),
            selection_heading: obj!("selection_heading"),
            selection_caption: obj!("selection_caption"),
            members_list: obj!("members_list"),
            ungroup_button: obj!("ungroup_button"),
            entries: [
                obj!("arrange_x"),
                obj!("arrange_y"),
                obj!("arrange_w"),
                obj!("arrange_h"),
            ],
            rotation: obj!("arrange_rotation"),
            select_all_type_button: obj!("select_all_type_button"),
            group_note: obj!("group_note"),
            frame: Cell::new(None),
            updating: Cell::new(false),
            on_arrange: RefCell::new(None),
        });
        sidebar.ungroup_button.set_action_name(Some("win.ungroup"));
        sidebar.corner_radius_row.connect_output(|row| {
            row.set_text(&gettext("{n} px").replace("{n}", &format_number(row.value())));
            true
        });
        sidebar.rotation.connect_output(|spin| {
            spin.set_text(&format!("{}°", format_number(spin.value())));
            glib::Propagation::Stop
        });
        sidebar.rotation.connect_input(|spin| {
            let digits: String = spin
                .text()
                .chars()
                .filter(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ',' | '\u{2212}'))
                .map(|c| if c == '\u{2212}' { '-' } else { c })
                .collect();
            Some(parse_number(&digits).ok_or(()))
        });
        sidebar.connect_inputs();
        Some(sidebar)
    }

    pub fn connect_arrange(&self, f: impl Fn(Arrange) + 'static) {
        self.on_arrange.replace(Some(Box::new(f)));
    }

    /// Enter, leaving an entry, and the rotation spin button apply the Arrange values.
    fn connect_inputs(self: &std::rc::Rc<Self>) {
        let weak = std::rc::Rc::downgrade(self);
        for entry in &self.entries {
            let w = weak.clone();
            entry.connect_activate(move |_| {
                if let Some(sidebar) = w.upgrade() {
                    sidebar.apply();
                }
            });
            let focus = gtk::EventControllerFocus::new();
            let w = weak.clone();
            focus.connect_leave(move |_| {
                if let Some(sidebar) = w.upgrade() {
                    sidebar.apply();
                }
            });
            entry.add_controller(focus);
        }
        self.rotation.connect_value_changed(move |_| {
            if let Some(sidebar) = weak.upgrade() {
                sidebar.apply();
            }
        });
    }

    fn apply(&self) {
        if self.updating.get() {
            return;
        }
        let Some((frame, multi)) = self.frame.get() else {
            return;
        };
        let values: Vec<f64> = self
            .entries
            .iter()
            .zip([
                frame.rect.x,
                frame.rect.y,
                frame.rect.width,
                frame.rect.height,
            ])
            .map(|(entry, current)| parse_number(&entry.text()).unwrap_or(current))
            .collect();
        let rotation = if multi {
            frame.rotation
        } else {
            self.rotation.value()
        };
        let min = annota::document::annotation::MIN_SIZE;
        let target = Arrange::new(
            Rect::new(values[0], values[1], values[2].max(min), values[3].max(min)),
            rotation,
        );
        if let Some(f) = self.on_arrange.borrow().as_ref() {
            f(target);
        }
    }

    pub fn show(&self, state: &State) {
        self.updating.set(true);
        match state {
            State::Empty => {
                self.frame.set(None);
                self.stack.set_visible_child_name("empty");
            }
            State::Document(info) => {
                self.frame.set(None);
                self.show_document(info);
                self.stack.set_visible_child_name("document");
            }
            State::Selection(info) => {
                self.show_selection(info);
                self.stack.set_visible_child_name("selection");
            }
        }
        self.updating.set(false);
    }

    pub fn is_empty(&self) -> bool {
        self.stack.visible_child_name().as_deref() == Some("empty")
    }

    fn show_document(&self, info: &DocumentInfo) {
        self.size_row.set_subtitle(
            &gettext("{size} px").replace("{size}", &text::size(info.size.0, info.size.1)),
        );
        self.corner_radius_row.set_value(info.corner_radius);
        let current = Swatch::current(info.background);
        for (swatch, button) in &self.swatches {
            if Some(*swatch) == current {
                button.add_css_class("annota-swatch-current");
            } else {
                button.remove_css_class("annota-swatch-current");
            }
        }
        self.objects_row
            .set_title(&text::editable_objects(info.objects));

        let large = info.objects > SUMMARY_THRESHOLD;
        self.summary_group.set_visible(large);
        if large && *self.summary_key.borrow() != info.by_type {
            self.summary_key.replace(info.by_type.clone());
            self.summary_caption
                .set_text(&text::object_count(info.objects));
            while let Some(row) = self.summary_list.first_child() {
                self.summary_list.remove(&row);
            }
            for (type_name, n) in &info.by_type {
                let row = adw::ActionRow::builder()
                    .title(text::type_plural(type_name))
                    .use_markup(false)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name(type_icon(type_name)));
                let badge = gtk::Label::builder()
                    .label(n.to_string())
                    .valign(gtk::Align::Center)
                    .css_classes(["annota-count-badge", "numeric"])
                    .build();
                row.add_suffix(&badge);
                let select = gtk::Button::builder()
                    .label(gettext("Select"))
                    .valign(gtk::Align::Center)
                    .action_target(&type_name.to_variant())
                    .action_name("win.select-type")
                    .css_classes(["flat"])
                    .build();
                row.add_suffix(&select);
                self.summary_list.append(&row);
            }
        }
    }

    fn show_selection(&self, info: &SelectionInfo) {
        let (heading, caption) =
            text::selection_heading(info.common, info.count, info.members.len());
        self.selection_heading.set_text(&heading);
        self.selection_caption.set_text(&caption);

        let group = info.count == 1 && info.common == Some(Group::TYPE);
        self.members_list.set_visible(group);
        self.ungroup_button.set_visible(group);
        self.group_note.set_visible(group);
        while let Some(row) = self.members_list.first_child() {
            self.members_list.remove(&row);
        }
        if group {
            for member in &info.members {
                let row = adw::ActionRow::builder()
                    .title(object_name(member))
                    .use_markup(false)
                    .build();
                row.add_prefix(&gtk::Image::from_icon_name(type_icon(member)));
                self.members_list.append(&row);
            }
        }

        let multi = info.count > 1;
        self.frame.set(Some((info.frame, multi)));
        let r = info.frame.rect;
        for (entry, v) in self.entries.iter().zip([r.x, r.y, r.width, r.height]) {
            entry.set_text(&format_number(v));
        }
        // Several objects form one box without an orientation of its own.
        self.rotation
            .set_value(if multi { 0.0 } else { info.frame.rotation });
        self.rotation.set_sensitive(!multi);

        let select_all = info.common.and_then(text::select_all_label);
        self.select_all_type_button
            .set_visible(select_all.is_some());
        if let (Some(label), Some(type_name)) = (select_all, info.common) {
            self.select_all_type_button.set_label(&label);
            self.select_all_type_button
                .set_action_target_value(Some(&type_name.to_variant()));
            self.select_all_type_button
                .set_action_name(Some("win.select-type"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::bitmap::Bitmap;
    use annota::document::command::History;
    use annota::document::geometry::Point;
    use annota::editing::ops;
    use annota::editing::selection::Selection;

    fn doc_with_images(n: usize) -> (Document, History) {
        let mut doc = Document::new(Bitmap::filled(1600, 1000, palette::PAPER).unwrap());
        let mut history = History::new();
        for k in 0..n {
            let pip = Bitmap::filled(100, 50, palette::CORAL).unwrap();
            let at = Point::new(100.0 + 20.0 * k as f64, 100.0);
            let cmd = Image::insert_command(&doc, pip, at);
            history.execute(&mut doc, "Insert", cmd).unwrap();
        }
        (doc, history)
    }

    #[test]
    fn document_state_without_selection() {
        let (doc, _) = doc_with_images(3);
        let State::Document(info) = state(&doc, &[]) else {
            panic!("document page expected");
        };
        assert_eq!(info.size, (1600, 1000));
        assert_eq!(info.objects, 3);
        assert_eq!(info.by_type, vec![(Image::TYPE, 3)]);
        assert_eq!(info.background, Background::Transparent);
        assert_eq!(Swatch::current(info.background), Some(Swatch::Transparent));
    }

    #[test]
    fn selection_state_for_one_several_and_a_group() {
        let (mut doc, mut history) = doc_with_images(3);
        let ids = doc.scene().ids();
        let State::Selection(one) = state(&doc, &ids[..1]) else {
            panic!("selection page expected");
        };
        assert_eq!((one.count, one.common), (1, Some(Image::TYPE)));
        assert_eq!(one.frame.rect, Rect::new(50.0, 75.0, 100.0, 50.0));
        let State::Selection(two) = state(&doc, &ids[..2]) else {
            panic!("selection page expected");
        };
        assert_eq!(two.frame.rect, Rect::new(50.0, 75.0, 120.0, 50.0));
        assert_eq!(two.frame.rotation, 0.0);

        let mut sel = Selection::new();
        sel.set(doc.scene(), ids[..2].to_vec());
        let edit = ops::group(&doc, &sel).unwrap();
        history.execute(&mut doc, "Group", edit.command).unwrap();
        let group = doc
            .scene()
            .nodes()
            .iter()
            .find(|n| n.object.type_name() == Group::TYPE)
            .unwrap()
            .id;
        let State::Selection(g) = state(&doc, &[group]) else {
            panic!("selection page expected");
        };
        assert_eq!(g.common, Some(Group::TYPE));
        assert_eq!(g.members, vec![Image::TYPE, Image::TYPE]);
        let State::Selection(mixed) = state(&doc, &doc.scene().ids()) else {
            panic!("selection page expected");
        };
        assert_eq!(mixed.common, None);
        // The summary counts groups and images apart, most first.
        let State::Document(info) = state(&doc, &[]) else {
            panic!("document page expected");
        };
        assert_eq!(info.by_type, vec![(Group::TYPE, 1), (Image::TYPE, 1)]);
    }

    #[test]
    fn swatch_for_each_background() {
        let color = |c| Background::Color { color: c };
        assert_eq!(Swatch::current(color(palette::CORAL)), Some(Swatch::Coral));
        assert_eq!(Swatch::current(color(palette::PAPER)), Some(Swatch::Paper));
        assert_eq!(
            Swatch::current(color(Rgba::opaque(1, 2, 3))),
            Some(Swatch::Custom)
        );
        assert_eq!(Swatch::current(Background::Unknown), None);
    }

    #[test]
    fn arrange_numbers() {
        assert_eq!(parse_number(" 580 "), Some(580.0));
        assert_eq!(parse_number("12,5"), Some(12.5));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(format_number(580.0), "580");
        assert_eq!(format_number(12.46), "12.5");
        assert_eq!(format_number(-0.04), "0");
    }
}
