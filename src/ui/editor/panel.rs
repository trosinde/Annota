//! The type properties' widgets (add-core-annotation-tools tasks 3.1–3.3): the rows of
//! `editor-window.blp`'s `type_panel`, shown and ordered by [`properties::rows`], the colour
//! picker popover (mockup 16) and the shadow editor (mockup 18).
//!
//! Every control edits one property through `properties`/`set_property`; what it sends is a
//! [`PanelEvent`] for the editor, which turns it into one `Command` on the selection, a change
//! of the text being edited, or a change of the tool's defaults. Like the Arrange and Canvas
//! entries (the phantom-edit fixes of add-annotation-editor), a control never sends the value
//! the panel already shows — not when it is filled in, not when a spin button re-reads its
//! rounded text on leaving it — so walking through the panel never adds an undo step.
//!
//! Sliders, the shadow's angle dial and the colour picker preview their value on the canvas
//! while they are dragged and commit once, when they are let go (or the popover closes): one
//! undo step per committed change.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gettextrs::{gettext, ngettext, pgettext};
use gtk::{gdk, glib};

use annota::document::annotation::PropertyValue;
use annota::document::color::{palette, Rgba};
use annota::document::style::{LineStyle, Shadow};
use annota::tools::arrow::{self, EndStyle};
use annota::tools::callout::{self, Callout};
use annota::tools::pen;
use annota::tools::redaction::{Redaction, RedactionKind};
use annota::tools::shape;
use annota::tools::step::{self, Counting, Step};
use annota::tools::text;

use super::colour::{self, Hsva, Swatch, SwatchSet};
use super::glyphs::{self, Glyph};
use super::properties::{self, Mode, PanelInfo, Row};
use super::sidebar::{format_number, parse_number};
use crate::ui::labels::object_name;

/// What the user asked for in the type properties.
#[derive(Clone, Debug, PartialEq)]
pub enum PanelEvent {
    /// Set a property (one undo step on the selection, or a tool default).
    Property(&'static str, PropertyValue),
    /// Show a property's value on the canvas while a slider, the dial or the colour picker is
    /// dragged; `Property` or `EndPreview` follows.
    Preview(&'static str, PropertyValue),
    EndPreview,
    /// The eyedropper: pick the property's colour anywhere on the screen.
    PickScreen(&'static str),
    RestartSequence,
    /// "Next value" of the step tool.
    NextStep(u32),
}

type Handler = Box<dyn Fn(PanelEvent)>;
/// Commits a slider's value.
type Release = Rc<dyn Fn(f64)>;
/// A toggle group: the choice each button stands for.
type Choices = Vec<(&'static str, gtk::ToggleButton)>;

/// One colour row: its property, its swatches and where they go.
pub(super) struct ColourRow {
    pub(super) key: &'static str,
    pub(super) caption: gtk::Label,
    pub(super) container: gtk::Box,
    /// The set, the recent colours and the buttons it was built with.
    pub(super) built: RefCell<Option<(SwatchSet, Vec<Rgba>)>>,
    pub(super) buttons: RefCell<Vec<(Swatch, gtk::Button)>>,
}

/// A slider with its value label: `factor` maps the stored value to the slider's.
pub(super) struct Slider {
    pub(super) scale: gtk::Scale,
    pub(super) label: gtk::Label,
    /// Pressed with the pointer: changes preview until it is released.
    pub(super) sliding: Rc<Cell<bool>>,
    /// Commits the value where the pointer let go.
    pub(super) release: RefCell<Option<Release>>,
}

impl Slider {
    /// The pointer let go: the value is committed (one undo step) — what the legacy
    /// controller of `bind_slider` does on a button release.
    #[cfg(test)]
    fn released(&self) {
        if self.sliding.replace(false) {
            let release = self.release.borrow().clone();
            if let Some(release) = release {
                release(self.scale.value());
            }
        }
    }
}

/// Hue, alpha or saturation/value: what a drag in the colour popover moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PickerPart {
    Area,
    Hue,
    Alpha,
}

/// The colour being picked in the popover.
#[derive(Clone, Copy, Debug)]
pub(super) struct Picking {
    pub(super) key: &'static str,
    pub(super) colour: Hsva,
    pub(super) initial: Rgba,
}

pub(super) struct ColourPopover {
    pub(super) popover: gtk::Popover,
    pub(super) area: gtk::DrawingArea,
    pub(super) hue: gtk::DrawingArea,
    pub(super) alpha: gtk::DrawingArea,
    pub(super) hex: gtk::Entry,
    pub(super) channels: [gtk::Entry; 3],
    pub(super) pick_screen: gtk::Button,
    pub(super) recent: gtk::Box,
    pub(super) state: Cell<Option<Picking>>,
}

pub(super) struct ShadowEditor {
    pub(super) row: adw::ExpanderRow,
    pub(super) swatches: gtk::Box,
    pub(super) swatch_buttons: RefCell<Vec<(Rgba, gtk::Button)>>,
    pub(super) dial: gtk::DrawingArea,
    pub(super) angle: gtk::SpinButton,
    pub(super) distance: Slider,
    pub(super) blur: Slider,
    pub(super) opacity: Slider,
    pub(super) reset: gtk::Button,
    /// The dial is being dragged.
    pub(super) turning: Cell<bool>,
}

pub struct TypePanel {
    pub root: gtk::Box,
    pub(super) rows: HashMap<&'static str, gtk::Widget>,
    pub(super) sequence: gtk::DropDown,
    pub(super) shape_kind: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) callout_shape: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) step_shape: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) redact_type: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) redact_shape: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) line_style: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) alignment: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) flags: Vec<(&'static str, gtk::ToggleButton)>,
    pub(super) font_family: gtk::DropDown,
    pub(super) fonts: gtk::StringList,
    pub(super) colour_rows: Vec<ColourRow>,
    pub(super) fill_opacity: Slider,
    pub(super) opacity: Slider,
    pub(super) smoothing: Slider,
    pub(super) intensity: Slider,
    /// Spin buttons that edit a number property: (key, spin, stored → shown factor).
    pub(super) spins: Vec<(&'static str, gtk::SpinButton, f64)>,
    pub(super) start_glyph: gtk::DrawingArea,
    pub(super) end_glyph: gtk::DrawingArea,
    pub(super) start_tiles: Vec<(EndStyle, gtk::ToggleButton)>,
    pub(super) end_tiles: Vec<(EndStyle, gtk::ToggleButton)>,
    pub(super) start_popover: gtk::Popover,
    pub(super) end_popover: gtk::Popover,
    pub(super) shown_ends: Cell<(EndStyle, EndStyle)>,
    pub(super) next_value: gtk::SpinButton,
    pub(super) value_entry: gtk::Entry,
    pub(super) restart: gtk::Button,
    pub(super) shadow: ShadowEditor,
    pub(super) picker: ColourPopover,
    /// What is shown.
    pub(super) info: RefCell<Option<PanelInfo>>,
    /// Custom colours committed recently (newest first).
    pub(super) recent: RefCell<Vec<Rgba>>,
    /// The property whose colour the screen eyedropper is picking.
    pub(super) picking: Cell<Option<&'static str>>,
    pub(super) updating: Cell<bool>,
    pub(super) handler: RefCell<Option<Handler>>,
}

/// The Blueprint name of `row`'s widget.
fn row_name(row: Row) -> &'static str {
    match row {
        Row::Sequence => "row_sequence",
        Row::ShapeKind => "row_shape_kind",
        Row::CalloutShape => "row_callout_shape",
        Row::StepShape => "row_step_shape",
        Row::RedactType => "row_redact_type",
        Row::Font => "row_font",
        Row::TextStyle => "row_text_style",
        Row::Colour { key: "fill", .. } => "row_fill",
        Row::Colour {
            key: "outline_color",
            ..
        } => "row_outline",
        Row::Colour { .. } => "row_color",
        Row::FillOpacity => "row_fill_opacity",
        Row::Width => "row_width",
        Row::OutlineWidth => "row_outline_width",
        Row::OutlineAndRadius => "row_outline_radius",
        Row::OutlineAndPadding => "row_outline_padding",
        Row::Ends => "row_ends",
        Row::LineStyle => "row_line_style",
        Row::Smoothing => "row_smoothing",
        Row::Intensity => "row_intensity",
        Row::RedactShape => "row_redact_shape",
        Row::Size => "row_size",
        Row::Value => "row_value",
        Row::NextValue => "row_next_value",
        Row::RestartSequence => "row_restart",
        Row::Opacity => "row_opacity",
        Row::Shadow => "row_shadow",
        Row::HelpPen => "help_pen",
        Row::HelpPolygon => "help_polygon",
        Row::HelpHighlighter => "help_highlighter",
        Row::HelpCallout => "help_callout",
        Row::WarningRedact => "warning_redact",
    }
}

const ROW_NAMES: [&str; 31] = [
    "row_sequence",
    "row_shape_kind",
    "row_callout_shape",
    "row_step_shape",
    "row_redact_type",
    "row_font",
    "row_text_style",
    "row_color",
    "row_fill",
    "row_outline",
    "row_fill_opacity",
    "row_width",
    "row_outline_width",
    "row_outline_radius",
    "row_outline_padding",
    "row_ends",
    "row_line_style",
    "row_smoothing",
    "row_intensity",
    "row_redact_shape",
    "row_size",
    "row_value",
    "row_next_value",
    "row_restart",
    "row_opacity",
    "row_shadow",
    "help_pen",
    "help_polygon",
    "help_highlighter",
    "help_callout",
    "warning_redact",
];

/// Heading and caption of the type properties (redline: "1 selected", "Tool defaults",
/// "Editing", "Drawing polygon"; mockup 09: "1 selected · 2 tails").
pub fn heading(info: &PanelInfo) -> (String, String) {
    let name = object_name(info.type_name);
    match info.mode {
        Mode::Selected(1) => match info.tails {
            Some(n) => (
                name,
                ngettext("1 selected · {n} tail", "1 selected · {n} tails", n as u32)
                    .replace("{n}", &n.to_string()),
            ),
            None => (name, gettext("1 selected")),
        },
        Mode::Selected(n) => super::text::selection_heading(Some(info.type_name), n, 0),
        Mode::ToolDefaults => (name, gettext("Tool defaults")),
        Mode::Editing => (name, gettext("Editing")),
        Mode::DrawingPolygon => (name, gettext("Drawing polygon")),
    }
}

/// The caption of a colour row for `type_name` (redline: "Colour", "Fill", "Outline"; the
/// redaction's "Solid colour"; a callout's text colour).
fn colour_caption(type_name: &str, key: &str) -> String {
    match (type_name, key) {
        (_, "fill") => gettext("Fill"),
        (_, "outline_color") => gettext("Outline"),
        (Redaction::TYPE, _) => gettext("Solid colour"),
        (Callout::TYPE, _) => gettext("Text colour"),
        _ => gettext("Colour"),
    }
}

/// Whether two numbers look the same in a control showing `digits` decimals.
fn same_shown(a: f64, b: f64) -> bool {
    format_number(a) == format_number(b)
}

/// Whether `commit` should treat `value` as a no-op given what the panel currently shows for
/// that property and the selection `mode`.
///
/// Always `false` (never suppressed) for a multi-object selection: `PanelInfo::values` only
/// reflects the *first* selected object (there is no "mixed" representation), so comparing
/// against it would silently drop a real edit on every other selected object that already
/// happened to share the first one's value — e.g. clicking the colour the first of several
/// differently-coloured selected arrows already has would otherwise look like "no change" and
/// never reach the others. `ops::set_property` already no-ops per object when nothing about
/// *that* object actually changes, so this guard is only ever redundant there, never
/// load-bearing.
fn unchanged(mode: Mode, shown: Option<&PropertyValue>, value: &PropertyValue) -> bool {
    let multi_selection = matches!(mode, Mode::Selected(n) if n > 1);
    !multi_selection && shown == Some(value)
}

/// A spin button's value as text: "8 px", "100 %", "45°".
fn spin_text(value: f64, unit: &str) -> String {
    match unit {
        "" => format_number(value),
        "°" => format!("{}°", format_number(value)),
        unit => gettext("{n} {unit}")
            .replace("{n}", &format_number(value))
            .replace("{unit}", unit),
    }
}

fn set_spin_unit(spin: &gtk::SpinButton, unit: &'static str) {
    spin.connect_output(move |spin| {
        spin.set_text(&spin_text(spin.value(), unit));
        glib::Propagation::Stop
    });
    spin.connect_input(|spin| {
        let digits: String = spin
            .text()
            .chars()
            .filter(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ','))
            .collect();
        Some(parse_number(&digits).ok_or(()))
    });
}

/// The foreground colour of `widget` for cairo.
fn fg(widget: &impl IsA<gtk::Widget>) -> (f64, f64, f64, f64) {
    let c = widget.as_ref().color();
    (
        f64::from(c.red()),
        f64::from(c.green()),
        f64::from(c.blue()),
        f64::from(c.alpha()),
    )
}

fn set_glyph(button: &gtk::ToggleButton, glyph: Glyph) {
    if let Some(area) = button.child().and_downcast::<gtk::DrawingArea>() {
        area.set_draw_func(move |area, cr, w, h| {
            let _ = glyphs::draw(cr, glyph, f64::from(w), f64::from(h), fg(area));
        });
    }
}

/// The DrawingArea inside an end-style tile.
fn tile_area(button: &gtk::ToggleButton) -> Option<gtk::DrawingArea> {
    button
        .child()
        .and_then(|b| b.first_child())
        .and_downcast::<gtk::DrawingArea>()
}

fn label_of(scale: &gtk::Scale, label: &gtk::Label, unit: &str) {
    label.set_text(&spin_text(scale.value(), unit));
}

impl TypePanel {
    pub fn new(builder: &gtk::Builder) -> Option<Rc<Self>> {
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
        let toggles =
            |pairs: &[(&'static str, &str)]| -> Option<Vec<(&'static str, gtk::ToggleButton)>> {
                let mut out = Vec::new();
                for (key, name) in pairs {
                    out.push((*key, get(name)?.downcast().ok()?));
                }
                Some(out)
            };
        let slider = |scale: &str, label: &str| -> Option<Slider> {
            Some(Slider {
                scale: get(scale)?.downcast().ok()?,
                label: get(label)?.downcast().ok()?,
                sliding: Rc::new(Cell::new(false)),
                release: RefCell::new(None),
            })
        };
        let mut rows = HashMap::new();
        for name in ROW_NAMES {
            rows.insert(name, get(name)?.downcast::<gtk::Widget>().ok()?);
        }
        let colour_row = |key: &'static str, caption: &str, container: &str| -> Option<ColourRow> {
            Some(ColourRow {
                key,
                caption: get(caption)?.downcast().ok()?,
                container: get(container)?.downcast().ok()?,
                built: RefCell::new(None),
                buttons: RefCell::new(Vec::new()),
            })
        };
        let ends = |prefix: &str| -> Option<Vec<(EndStyle, gtk::ToggleButton)>> {
            let mut out = Vec::new();
            for &style in EndStyle::ALL {
                let button: gtk::ToggleButton =
                    get(&format!("{prefix}_{}", style.key()))?.downcast().ok()?;
                out.push((style, button));
            }
            Some(out)
        };
        let fonts = gtk::StringList::new(&[]);
        let panel = Rc::new(Self {
            root: obj!("type_panel"),
            rows,
            sequence: obj!("step_sequence"),
            shape_kind: toggles(&[
                ("rectangle", "shape_rectangle"),
                ("rounded_rectangle", "shape_rounded_rectangle"),
                ("ellipse", "shape_ellipse"),
                ("polygon", "shape_polygon"),
            ])?,
            callout_shape: toggles(&[
                ("rectangle", "callout_rectangle"),
                ("rounded_rectangle", "callout_rounded_rectangle"),
                ("ellipse", "callout_ellipse"),
                ("cloud", "callout_cloud"),
            ])?,
            step_shape: toggles(&[
                ("circle", "step_circle"),
                ("square", "step_square"),
                ("rounded_square", "step_rounded_square"),
                ("pin", "step_pin"),
            ])?,
            redact_type: toggles(&[
                ("blur", "redact_blur"),
                ("pixelate", "redact_pixelate"),
                ("solid", "redact_solid"),
            ])?,
            redact_shape: toggles(&[
                ("rectangle", "redact_rectangle"),
                ("ellipse", "redact_ellipse"),
            ])?,
            line_style: toggles(&[
                ("solid", "line_solid"),
                ("dashed", "line_dashed"),
                ("dotted", "line_dotted"),
            ])?,
            alignment: toggles(&[
                ("left", "align_left"),
                ("center", "align_center"),
                ("right", "align_right"),
            ])?,
            flags: toggles(&[
                ("bold", "text_bold"),
                ("italic", "text_italic"),
                ("underline", "text_underline"),
                ("strikethrough", "text_strikethrough"),
            ])?,
            font_family: obj!("font_family"),
            fonts,
            colour_rows: vec![
                colour_row("color", "color_caption", "color_swatches")?,
                colour_row("fill", "fill_caption", "fill_swatches")?,
                colour_row("outline_color", "outline_caption", "outline_swatches")?,
            ],
            fill_opacity: slider("fill_opacity_scale", "fill_opacity_value")?,
            opacity: slider("opacity_scale", "opacity_value")?,
            smoothing: slider("smoothing_scale", "smoothing_value")?,
            intensity: slider("intensity_scale", "intensity_value")?,
            spins: vec![
                ("width", obj!("width_spin"), 1.0),
                ("outline_width", obj!("outline_width_spin"), 1.0),
                ("outline_width", obj!("radius_outline_spin"), 1.0),
                ("corner_radius", obj!("corner_radius_spin"), 1.0),
                ("outline_width", obj!("padding_outline_spin"), 1.0),
                ("padding", obj!("padding_spin"), 1.0),
                ("radius", obj!("size_spin"), 1.0),
                ("font_size", obj!("font_size"), 1.0),
                ("start_size", obj!("start_size_spin"), 100.0),
                ("end_size", obj!("end_size_spin"), 100.0),
            ],
            start_glyph: obj!("start_style_glyph"),
            end_glyph: obj!("end_style_glyph"),
            start_tiles: ends("start")?,
            end_tiles: ends("end")?,
            start_popover: obj!("start_style_popover"),
            end_popover: obj!("end_style_popover"),
            shown_ends: Cell::new((EndStyle::None, EndStyle::None)),
            next_value: obj!("next_value_spin"),
            value_entry: obj!("value_entry"),
            restart: obj!("row_restart"),
            shadow: ShadowEditor {
                row: obj!("shadow_row"),
                swatches: obj!("shadow_swatches"),
                swatch_buttons: RefCell::new(Vec::new()),
                dial: obj!("shadow_dial"),
                angle: obj!("shadow_angle"),
                distance: slider("shadow_distance", "shadow_distance_value")?,
                blur: slider("shadow_blur", "shadow_blur_value")?,
                opacity: slider("shadow_opacity", "shadow_opacity_value")?,
                reset: obj!("shadow_reset"),
                turning: Cell::new(false),
            },
            picker: ColourPopover {
                popover: obj!("colour_popover"),
                area: obj!("colour_area"),
                hue: obj!("colour_hue"),
                alpha: obj!("colour_alpha"),
                hex: obj!("colour_hex"),
                channels: [obj!("colour_r"), obj!("colour_g"), obj!("colour_b")],
                pick_screen: obj!("colour_pick_screen"),
                recent: obj!("colour_recent"),
                state: Cell::new(None),
            },
            info: RefCell::new(None),
            recent: RefCell::new(Vec::new()),
            picking: Cell::new(None),
            updating: Cell::new(false),
            handler: RefCell::new(None),
        });
        panel.setup();
        Some(panel)
    }

    pub fn connect_event(&self, f: impl Fn(PanelEvent) + 'static) {
        self.handler.replace(Some(Box::new(f)));
    }

    fn emit(&self, event: PanelEvent) {
        if self.updating.get() {
            return;
        }
        if let Some(f) = self.handler.borrow().as_ref() {
            f(event);
        }
    }

    /// Sends `value` for `key` unless it is what the panel shows (the phantom-edit guard).
    fn commit(&self, key: &'static str, value: PropertyValue) {
        if self.updating.get() {
            return;
        }
        let info = self.info.borrow();
        let mode = info.as_ref().map(|i| i.mode).unwrap_or(Mode::ToolDefaults);
        let shown = info.as_ref().and_then(|i| i.value(key).cloned());
        drop(info);
        if unchanged(mode, shown.as_ref(), &value) {
            // A preview may still be up (a slider let go where it started).
            self.emit(PanelEvent::EndPreview);
            return;
        }
        self.emit(PanelEvent::Property(key, value));
    }

    fn preview(&self, key: &'static str, value: PropertyValue) {
        self.emit(PanelEvent::Preview(key, value));
    }

    /// The custom colours committed recently (newest first), for tests.
    #[cfg(test)]
    pub fn recent(&self) -> Vec<Rgba> {
        self.recent.borrow().clone()
    }

    /// Drags the slider `name` through `values` with the pointer and lets go (tests: GTK
    /// cannot synthesise the pointer; this is the same path the pointer takes).
    #[cfg(test)]
    pub fn slide(&self, name: &str, values: &[f64]) {
        let slider = match name {
            "opacity" => &self.opacity,
            "fill_opacity" => &self.fill_opacity,
            "smoothing" => &self.smoothing,
            "intensity" => &self.intensity,
            "shadow_distance" => &self.shadow.distance,
            "shadow_blur" => &self.shadow.blur,
            _ => &self.shadow.opacity,
        };
        slider.sliding.set(true);
        for v in values {
            slider.scale.set_value(*v);
        }
        slider.released();
    }

    /// The Blueprint names of the rows shown, top to bottom (tests).
    #[cfg(test)]
    pub fn shown_rows(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let mut child = self.root.first_child();
        while let Some(widget) = child {
            if widget.is_visible() {
                if let Some((name, _)) = self.rows.iter().find(|(_, w)| **w == widget) {
                    out.push(*name);
                }
            }
            child = widget.next_sibling();
        }
        out
    }

    /// Remembers a committed custom colour (the picker, the eyedropper).
    pub fn remember(self: &Rc<Self>, colour: Rgba) {
        if colour::remember(&mut self.recent.borrow_mut(), colour) {
            if let Some(info) = self.info.borrow().clone() {
                self.show_colours(&info);
            }
        }
    }

    /// The screen eyedropper is (or is no longer) picking `key`'s colour: its swatch is filled
    /// with the accent colour (mockup 17).
    pub fn set_picking(&self, key: Option<&'static str>) {
        self.picking.set(key);
        for row in &self.colour_rows {
            for (swatch, button) in row.buttons.borrow().iter() {
                if *swatch == Swatch::Eyedropper {
                    if key == Some(row.key) {
                        button.add_css_class("annota-swatch-picking");
                    } else {
                        button.remove_css_class("annota-swatch-picking");
                    }
                }
            }
        }
    }

    // ---- setup ----

    fn setup(self: &Rc<Self>) {
        self.setup_fonts();
        self.setup_choices();
        self.setup_spins();
        self.setup_sliders();
        self.setup_ends();
        self.setup_step();
        self.setup_shadow();
        self.setup_picker();
    }

    fn setup_fonts(self: &Rc<Self>) {
        use gtk::pango::prelude::*;
        let map = pangocairo::FontMap::default();
        let mut names: Vec<String> = map
            .list_families()
            .iter()
            .map(|f| f.name().to_string())
            .collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        for name in &names {
            self.fonts.append(name);
        }
        self.font_family.set_model(Some(&self.fonts));
        let weak = Rc::downgrade(self);
        self.font_family.connect_selected_notify(move |dropdown| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let Some(name) = dropdown.selected_item().and_downcast::<gtk::StringObject>() else {
                return;
            };
            panel.commit(
                "font_family",
                PropertyValue::Text(name.string().to_string()),
            );
        });
    }

    fn setup_choices(self: &Rc<Self>) {
        let groups: [(&'static str, &Vec<(&'static str, gtk::ToggleButton)>); 7] = [
            ("shape", &self.shape_kind),
            ("shape", &self.callout_shape),
            ("shape", &self.step_shape),
            ("redaction", &self.redact_type),
            ("shape", &self.redact_shape),
            ("line_style", &self.line_style),
            ("alignment", &self.alignment),
        ];
        for (key, buttons) in groups {
            for (choice, button) in buttons {
                let choice = *choice;
                let weak = Rc::downgrade(self);
                button.connect_toggled(move |button| {
                    if let (Some(panel), true) = (weak.upgrade(), button.is_active()) {
                        panel.commit(key, PropertyValue::Choice(choice.into()));
                    }
                });
            }
        }
        for (key, button) in &self.flags {
            let key = *key;
            let weak = Rc::downgrade(self);
            button.connect_toggled(move |button| {
                if let Some(panel) = weak.upgrade() {
                    panel.commit(key, PropertyValue::Bool(button.is_active()));
                }
            });
        }
        let glyph_sets: [(&Choices, &[Glyph]); 5] = [
            (
                &self.shape_kind,
                &[
                    Glyph::Rectangle,
                    Glyph::RoundedRectangle,
                    Glyph::Ellipse,
                    Glyph::Polygon,
                ],
            ),
            (
                &self.callout_shape,
                &[
                    Glyph::Rectangle,
                    Glyph::RoundedRectangle,
                    Glyph::Ellipse,
                    Glyph::Cloud,
                ],
            ),
            (
                &self.step_shape,
                &[
                    Glyph::StepCircle,
                    Glyph::StepSquare,
                    Glyph::StepRoundedSquare,
                    Glyph::StepPin,
                ],
            ),
            (&self.redact_shape, &[Glyph::Rectangle, Glyph::Ellipse]),
            (
                &self.line_style,
                &[
                    Glyph::Line(LineStyle::Solid),
                    Glyph::Line(LineStyle::Dashed),
                    Glyph::Line(LineStyle::Dotted),
                ],
            ),
        ];
        for (buttons, glyphs) in glyph_sets {
            for ((_, button), glyph) in buttons.iter().zip(glyphs) {
                set_glyph(button, *glyph);
            }
        }
        // The line style glyphs are wider than an icon.
        for (_, button) in &self.line_style {
            if let Some(area) = button.child().and_downcast::<gtk::DrawingArea>() {
                area.set_content_width(28);
            }
        }
        let weak = Rc::downgrade(self);
        self.sequence.connect_selected_notify(move |dropdown| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            if let Some(counting) = Counting::ALL.get(dropdown.selected() as usize) {
                panel.commit("counting", PropertyValue::Choice(counting.key().into()));
            }
        });
    }

    fn setup_spins(self: &Rc<Self>) {
        for (key, spin, factor) in &self.spins {
            let (key, factor) = (*key, *factor);
            let unit = match key {
                "start_size" | "end_size" => "%",
                "font_size" => "",
                _ => "px",
            };
            set_spin_unit(spin, unit);
            let weak = Rc::downgrade(self);
            spin.connect_value_changed(move |spin| {
                let Some(panel) = weak.upgrade() else {
                    return;
                };
                let shown = panel
                    .info
                    .borrow()
                    .as_ref()
                    .and_then(|i| i.number(key))
                    .map(|v| v * factor);
                // Leaving a spin re-reads its rounded text: that is not an edit.
                if shown.is_some_and(|s| same_shown(s, spin.value())) {
                    return;
                }
                panel.commit(key, PropertyValue::Number(spin.value() / factor));
            });
        }
        set_spin_unit(&self.next_value, "");
        let weak = Rc::downgrade(self);
        self.next_value.connect_value_changed(move |spin| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let next = spin.value().round().max(1.0) as u32;
            let shown = panel.info.borrow().as_ref().and_then(|i| i.next_step);
            if shown != Some(next) {
                panel.emit(PanelEvent::NextStep(next));
            }
        });
    }

    /// A slider's value: while it is pressed a preview, on release (and from the keyboard) a
    /// commit.
    fn bind_slider(
        self: &Rc<Self>,
        slider: &Slider,
        unit: &'static str,
        value: impl Fn(&Self, f64) -> Option<(&'static str, PropertyValue)> + 'static,
    ) {
        let value = Rc::new(value);
        let weak = Rc::downgrade(self);
        let release = value.clone();
        slider.release.replace(Some(Rc::new(move |at: f64| {
            if let Some(panel) = weak.upgrade() {
                if let Some((key, v)) = release(&panel, at) {
                    panel.commit(key, v);
                }
            }
        })));
        // The pointer: pressed, the slider previews; released, it commits. (A legacy
        // controller sees the release even when the range's own drag gesture claimed it.)
        let legacy = gtk::EventControllerLegacy::new();
        legacy.set_propagation_phase(gtk::PropagationPhase::Capture);
        let sliding = slider.sliding.clone();
        let release = slider.release.borrow().clone();
        let scale = slider.scale.clone();
        legacy.connect_event(move |_, event| {
            match event.event_type() {
                gdk::EventType::ButtonPress | gdk::EventType::TouchBegin => sliding.set(true),
                gdk::EventType::ButtonRelease | gdk::EventType::TouchEnd
                    if sliding.replace(false) =>
                {
                    if let Some(release) = &release {
                        release(scale.value());
                    }
                }
                _ => {}
            }
            glib::Propagation::Proceed
        });
        slider.scale.add_controller(legacy);
        let sliding = slider.sliding.clone();
        let label = slider.label.clone();
        let weak = Rc::downgrade(self);
        slider.scale.connect_value_changed(move |scale| {
            label_of(scale, &label, unit);
            let Some(panel) = weak.upgrade() else {
                return;
            };
            if panel.updating.get() {
                return;
            }
            if let Some((key, v)) = value(&panel, scale.value()) {
                if sliding.get() {
                    panel.preview(key, v);
                } else {
                    panel.commit(key, v);
                }
            }
        });
    }

    fn setup_sliders(self: &Rc<Self>) {
        self.bind_slider(&self.opacity, "%", |_, v| {
            Some(("opacity", PropertyValue::Number(v / 100.0)))
        });
        self.bind_slider(&self.fill_opacity, "%", |_, v| {
            Some(("fill_opacity", PropertyValue::Number(v / 100.0)))
        });
        self.bind_slider(&self.smoothing, "", |_, v| {
            Some(("smoothing", PropertyValue::Number(v)))
        });
        self.bind_slider(&self.intensity, "", |_, v| {
            Some(("intensity", PropertyValue::Number(v)))
        });
    }

    fn setup_ends(self: &Rc<Self>) {
        for (tiles, start) in [(&self.start_tiles, true), (&self.end_tiles, false)] {
            for (style, button) in tiles {
                let style = *style;
                if let Some(area) = tile_area(button) {
                    area.set_draw_func(move |area, cr, w, h| {
                        let _ = glyphs::draw(
                            cr,
                            Glyph::End { style, start },
                            f64::from(w),
                            f64::from(h),
                            fg(area),
                        );
                    });
                }
                let weak = Rc::downgrade(self);
                button.connect_toggled(move |button| {
                    let Some(panel) = weak.upgrade() else {
                        return;
                    };
                    if !button.is_active() || panel.updating.get() {
                        return;
                    }
                    let key = if start { "start_style" } else { "end_style" };
                    panel.commit(key, PropertyValue::Choice(style.key().into()));
                    let popover = if start {
                        &panel.start_popover
                    } else {
                        &panel.end_popover
                    };
                    popover.popdown();
                });
            }
        }
        for (area, start) in [(&self.start_glyph, true), (&self.end_glyph, false)] {
            let weak = Rc::downgrade(self);
            area.set_draw_func(move |area, cr, w, h| {
                let Some(panel) = weak.upgrade() else {
                    return;
                };
                let (s, e) = panel.shown_ends.get();
                let style = if start { s } else { e };
                let _ = glyphs::draw(
                    cr,
                    Glyph::End { style, start },
                    f64::from(w),
                    f64::from(h),
                    fg(area),
                );
            });
        }
    }

    fn setup_step(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.restart.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.emit(PanelEvent::RestartSequence);
            }
        });
        let apply = |panel: &Self| {
            let text = panel.value_entry.text().trim().to_string();
            let shown = panel
                .info
                .borrow()
                .as_ref()
                .and_then(|i| i.text("value").map(str::to_string))
                .unwrap_or_default();
            if text != shown {
                panel.commit("value", PropertyValue::Text(text));
            }
        };
        let weak = Rc::downgrade(self);
        self.value_entry.connect_activate(move |_| {
            if let Some(panel) = weak.upgrade() {
                apply(&panel);
            }
        });
        let focus = gtk::EventControllerFocus::new();
        let weak = Rc::downgrade(self);
        focus.connect_leave(move |_| {
            if let Some(panel) = weak.upgrade() {
                apply(&panel);
            }
        });
        self.value_entry.add_controller(focus);
    }

    // ---- shadow editor (mockup 18) ----

    fn current_shadow(&self) -> Option<Shadow> {
        self.info.borrow().as_ref().and_then(PanelInfo::shadow)
    }

    /// The shown shadow with `change` applied, inside the editor's ranges.
    fn shadow_with(&self, change: impl FnOnce(&mut Shadow)) -> Option<PropertyValue> {
        let mut shadow = self.current_shadow()?;
        change(&mut shadow);
        shadow.bounded().map(PropertyValue::Shadow)
    }

    fn setup_shadow(self: &Rc<Self>) {
        let editor = &self.shadow;
        let weak = Rc::downgrade(self);
        editor.row.connect_enable_expansion_notify(move |row| {
            if let Some(panel) = weak.upgrade() {
                let on = row.enables_expansion();
                if let Some(v) = panel.shadow_with(|s| s.enabled = on) {
                    panel.commit("shadow", v);
                }
            }
        });
        let weak = Rc::downgrade(self);
        editor.row.connect_expanded_notify(move |row| {
            if let Some(panel) = weak.upgrade() {
                panel.shadow.reset.set_visible(row.is_expanded());
            }
        });
        let weak = Rc::downgrade(self);
        editor.reset.connect_clicked(move |_| {
            if let Some(panel) = weak.upgrade() {
                if let Some(v) = panel.shadow_with(|s| {
                    *s = Shadow {
                        enabled: s.enabled,
                        ..Shadow::DEFAULT
                    }
                }) {
                    panel.commit("shadow", v);
                }
            }
        });
        set_spin_unit(&editor.angle, "°");
        let weak = Rc::downgrade(self);
        editor.angle.connect_value_changed(move |spin| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            panel.shadow.dial.queue_draw();
            let shown = panel.current_shadow().map(|s| s.angle);
            if shown.is_some_and(|a| same_shown(a, spin.value())) {
                return;
            }
            if let Some(v) = panel.shadow_with(|s| s.angle = spin.value()) {
                panel.commit("shadow", v);
            }
        });
        let weak = Rc::downgrade(self);
        editor.dial.set_draw_func(move |area, cr, w, _| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let accent = fg(area);
            let track = (accent.0, accent.1, accent.2, 0.12);
            let _ = glyphs::draw_dial(cr, f64::from(w), panel.shadow.angle.value(), accent, track);
        });
        let drag = gtk::GestureDrag::new();
        let weak = Rc::downgrade(self);
        let turn = move |gesture: &gtk::GestureDrag, x: f64, y: f64, commit: bool| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let size = f64::from(panel.shadow.dial.width());
            let (sx, sy) = gesture.start_point().unwrap_or((x, y));
            let (px, py) = if commit || panel.shadow.turning.get() {
                (sx + x, sy + y)
            } else {
                (x, y)
            };
            let Some(angle) = glyphs::dial_angle(size, px, py) else {
                return;
            };
            let Some(v) = panel.shadow_with(|s| s.angle = angle) else {
                return;
            };
            // The spin follows the dial without committing on its own.
            panel.updating.set(true);
            panel.shadow.angle.set_value(angle);
            panel.updating.set(false);
            if commit {
                panel.shadow.turning.set(false);
                panel.commit("shadow", v);
            } else {
                panel.preview("shadow", v);
            }
        };
        let begin = turn.clone();
        drag.connect_drag_begin(move |g, x, y| begin(g, x, y, false));
        let weak = Rc::downgrade(self);
        let update = turn.clone();
        drag.connect_drag_update(move |g, x, y| {
            if let Some(panel) = weak.upgrade() {
                panel.shadow.turning.set(true);
            }
            update(g, x, y, false);
        });
        drag.connect_drag_end(move |g, x, y| turn(g, x, y, true));
        editor.dial.add_controller(drag);
        self.bind_slider(&self.shadow.distance, "px", |panel, v| {
            Some(("shadow", panel.shadow_with(|s| s.distance = v)?))
        });
        // The slider shows the blur as a CSS-style radius: twice the stored deviation.
        self.bind_slider(&self.shadow.blur, "px", |panel, v| {
            Some(("shadow", panel.shadow_with(|s| s.blur = v / 2.0)?))
        });
        self.bind_slider(&self.shadow.opacity, "%", |panel, v| {
            Some(("shadow", panel.shadow_with(|s| s.opacity = v / 100.0)?))
        });
    }

    fn show_shadow(self: &Rc<Self>, info: &PanelInfo) {
        let Some(shadow) = info.shadow() else {
            return;
        };
        let editor = &self.shadow;
        editor.row.set_enable_expansion(shadow.enabled);
        editor.reset.set_visible(editor.row.is_expanded());
        editor.angle.set_value(shadow.angle);
        editor.distance.scale.set_value(shadow.distance);
        editor.blur.scale.set_value(shadow.blur_radius());
        editor.opacity.scale.set_value(shadow.opacity * 100.0);
        label_of(&editor.distance.scale, &editor.distance.label, "px");
        label_of(&editor.blur.scale, &editor.blur.label, "px");
        label_of(&editor.opacity.scale, &editor.opacity.label, "%");
        editor.dial.queue_draw();
        if editor.swatch_buttons.borrow().is_empty() {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for c in colour::PALETTE {
                let button = self.swatch_button(Swatch::Colour(c), SwatchSet::Shadow.size());
                let weak = Rc::downgrade(self);
                button.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        // The shadow colour keeps the shadow's own opacity.
                        if let Some(v) = panel.shadow_with(|s| s.color = c) {
                            panel.commit("shadow", v);
                        }
                    }
                });
                line.append(&button);
                editor.swatch_buttons.borrow_mut().push((c, button));
            }
            editor.swatches.append(&line);
        }
        for (c, button) in editor.swatch_buttons.borrow().iter() {
            mark(button, *c == shadow.color);
        }
    }

    // ---- colour rows and the picker (mockups 02, 16) ----

    fn swatch_button(self: &Rc<Self>, swatch: Swatch, size: i32) -> gtk::Button {
        let button = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["annota-swatch"])
            .build();
        button.set_size_request(size, size);
        if size < 32 {
            button.add_css_class("annota-swatch-small");
        }
        let name = match swatch {
            Swatch::Colour(c) => swatch_name(c),
            Swatch::None => pgettext("colour", "None"),
            Swatch::Custom => gettext("Custom Colour"),
            Swatch::Eyedropper => gettext("Pick From Screen"),
        };
        button.set_tooltip_text(Some(&name));
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        if swatch == Swatch::Eyedropper {
            button.set_icon_name("color-select-symbolic");
            button.add_css_class("annota-swatch-eyedropper");
        } else {
            button.add_css_class("annota-swatch-drawn");
            let area = gtk::DrawingArea::new();
            area.set_content_width(size);
            area.set_content_height(size);
            area.set_can_target(false);
            let weak = button.downgrade();
            area.set_draw_func(move |_, cr, w, _| {
                let current = weak
                    .upgrade()
                    .is_some_and(|b| b.has_css_class("annota-swatch-current"));
                let _ = glyphs::draw_swatch(cr, swatch, f64::from(w), current);
            });
            button.set_child(Some(&area));
        }
        button
    }

    /// Fills the colour rows (rebuilt only when their swatches change; marks follow the value).
    fn show_colours(self: &Rc<Self>, info: &PanelInfo) {
        let rows = properties::rows(info);
        let recent = self.recent.borrow().clone();
        for (index, row) in self.colour_rows.iter().enumerate() {
            let Some(set) = rows.iter().find_map(|r| match r {
                Row::Colour { key, set } if *key == row.key => Some(*set),
                _ => None,
            }) else {
                continue;
            };
            row.caption
                .set_text(&colour_caption(info.type_name, row.key));
            let wanted = (
                set,
                if set.custom() {
                    recent.clone()
                } else {
                    Vec::new()
                },
            );
            if row.built.borrow().as_ref() != Some(&wanted) {
                while let Some(child) = row.container.first_child() {
                    row.container.remove(&child);
                }
                let mut buttons = Vec::new();
                for line in set.lines(&recent) {
                    let boxed = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    for swatch in line {
                        let button = self.swatch_button(swatch, set.size());
                        let weak = Rc::downgrade(self);
                        button.connect_clicked(move |button| {
                            if let Some(panel) = weak.upgrade() {
                                panel.swatch_clicked(index, swatch, button);
                            }
                        });
                        boxed.append(&button);
                        buttons.push((swatch, button));
                    }
                    row.container.append(&boxed);
                }
                row.buttons.replace(buttons);
                row.built.replace(Some(wanted));
            }
            let current = info.colour(row.key).unwrap_or(Rgba::TRANSPARENT);
            for (swatch, button) in row.buttons.borrow().iter() {
                mark(button, colour::is_current(*swatch, current));
            }
        }
        self.set_picking(self.picking.get());
    }

    fn swatch_clicked(self: &Rc<Self>, row: usize, swatch: Swatch, button: &gtk::Button) {
        let Some(key) = self.colour_rows.get(row).map(|r| r.key) else {
            return;
        };
        match swatch {
            Swatch::Colour(c) => self.commit(key, PropertyValue::Color(c)),
            Swatch::None => self.commit(key, PropertyValue::Color(Rgba::TRANSPARENT)),
            Swatch::Custom => self.open_picker(row, button),
            Swatch::Eyedropper => self.emit(PanelEvent::PickScreen(key)),
        }
    }

    fn open_picker(self: &Rc<Self>, row: usize, anchor: &gtk::Button) {
        let Some(row) = self.colour_rows.get(row) else {
            return;
        };
        let current = self
            .info
            .borrow()
            .as_ref()
            .and_then(|i| i.colour(row.key))
            .unwrap_or(palette::CORAL);
        let popover = &self.picker.popover;
        // Parented to the row's swatch box, which stays while the swatches are rebuilt.
        if popover.parent().as_ref() != Some(row.container.upcast_ref()) {
            if popover.parent().is_some() {
                popover.unparent();
            }
            popover.set_parent(&row.container);
        }
        if let Some(bounds) = anchor.compute_bounds(&row.container) {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(
                bounds.x() as i32,
                bounds.y() as i32,
                bounds.width() as i32,
                bounds.height() as i32,
            )));
        }
        popover.set_position(gtk::PositionType::Left);
        self.picker.state.set(Some(Picking {
            key: row.key,
            colour: Hsva::from_rgba(current),
            initial: current,
        }));
        self.show_picker();
        popover.popup();
        self.picker.hex.grab_focus();
    }

    /// Refreshes the popover from the colour being picked.
    fn show_picker(self: &Rc<Self>) {
        let Some(state) = self.picker.state.get() else {
            return;
        };
        let c = state.colour.to_rgba();
        let was = self.updating.replace(true);
        let hex = colour::hex(c);
        if self.picker.hex.text() != hex {
            self.picker.hex.set_text(&hex);
        }
        for (entry, v) in self.picker.channels.iter().zip([c.r, c.g, c.b]) {
            let text = v.to_string();
            if entry.text() != text {
                entry.set_text(&text);
            }
        }
        self.updating.set(was);
        self.picker.area.queue_draw();
        self.picker.hue.queue_draw();
        self.picker.alpha.queue_draw();
        if self.picker.recent.first_child().is_none() || self.picker_recent_stale() {
            while let Some(child) = self.picker.recent.first_child() {
                self.picker.recent.remove(&child);
            }
            for c in self.recent.borrow().iter().copied() {
                let button = self.swatch_button(Swatch::Colour(c), 24);
                let weak = Rc::downgrade(self);
                button.connect_clicked(move |_| {
                    if let Some(panel) = weak.upgrade() {
                        panel.picker_set(Hsva::from_rgba(c));
                    }
                });
                self.picker.recent.append(&button);
            }
        }
    }

    fn picker_recent_stale(&self) -> bool {
        let mut n = 0;
        let mut child = self.picker.recent.first_child();
        while let Some(c) = child {
            n += 1;
            child = c.next_sibling();
        }
        n != self.recent.borrow().len()
    }

    /// The picker's colour changed: shown in the popover and previewed on the canvas.
    fn picker_set(self: &Rc<Self>, colour: Hsva) {
        let Some(mut state) = self.picker.state.get() else {
            return;
        };
        state.colour = colour;
        self.picker.state.set(Some(state));
        self.show_picker();
        self.preview(state.key, PropertyValue::Color(colour.to_rgba()));
    }

    /// The popover closed: the colour picked is committed (and remembered), once.
    fn picker_closed(self: &Rc<Self>) {
        let Some(state) = self.picker.state.take() else {
            return;
        };
        let colour = state.colour.to_rgba();
        if colour == state.initial {
            self.emit(PanelEvent::EndPreview);
            return;
        }
        self.remember(colour);
        self.commit(state.key, PropertyValue::Color(colour));
    }

    fn setup_picker(self: &Rc<Self>) {
        let picker = &self.picker;
        let weak = Rc::downgrade(self);
        picker.popover.connect_closed(move |_| {
            if let Some(panel) = weak.upgrade() {
                panel.picker_closed();
            }
        });
        let weak: Weak<Self> = Rc::downgrade(self);
        picker.area.set_draw_func(move |_, cr, w, h| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let Some(state) = panel.picker.state.get() else {
                return;
            };
            let _ = draw_sv_area(cr, f64::from(w), f64::from(h), state.colour);
        });
        let weak: Weak<Self> = Rc::downgrade(self);
        picker.hue.set_draw_func(move |_, cr, w, h| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let Some(state) = panel.picker.state.get() else {
                return;
            };
            let _ = draw_bar(
                cr,
                f64::from(w),
                f64::from(h),
                PickerPart::Hue,
                state.colour,
            );
        });
        let weak: Weak<Self> = Rc::downgrade(self);
        picker.alpha.set_draw_func(move |_, cr, w, h| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let Some(state) = panel.picker.state.get() else {
                return;
            };
            let _ = draw_bar(
                cr,
                f64::from(w),
                f64::from(h),
                PickerPart::Alpha,
                state.colour,
            );
        });
        for (area, part) in [
            (&picker.area, PickerPart::Area),
            (&picker.hue, PickerPart::Hue),
            (&picker.alpha, PickerPart::Alpha),
        ] {
            let drag = gtk::GestureDrag::new();
            let weak = Rc::downgrade(self);
            let area_ref = area.clone();
            let moved = move |x: f64, y: f64| {
                let Some(panel) = weak.upgrade() else {
                    return;
                };
                let Some(state) = panel.picker.state.get() else {
                    return;
                };
                let (w, h) = (
                    f64::from(area_ref.width().max(1)),
                    f64::from(area_ref.height().max(1)),
                );
                let colour = match part {
                    PickerPart::Area => state.colour.with_sv(x / w, y / h),
                    PickerPart::Hue => state.colour.with_hue(x / w),
                    PickerPart::Alpha => state.colour.with_alpha(x / w),
                };
                panel.picker_set(colour);
            };
            let begin = moved.clone();
            drag.connect_drag_begin(move |_, x, y| begin(x, y));
            drag.connect_drag_update(move |g, dx, dy| {
                if let Some((x, y)) = g.start_point() {
                    moved(x + dx, y + dy);
                }
            });
            area.add_controller(drag);
        }
        let weak = Rc::downgrade(self);
        picker.hex.connect_changed(move |entry| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            if panel.updating.get() {
                return;
            }
            if let Some(c) = colour::parse_hex(&entry.text()) {
                if panel.picker.state.get().map(|s| s.colour.to_rgba()) != Some(c) {
                    panel.picker_set(Hsva::from_rgba(c));
                }
            }
        });
        for index in 0..3 {
            let weak = Rc::downgrade(self);
            picker.channels[index].connect_changed(move |entry| {
                let Some(panel) = weak.upgrade() else {
                    return;
                };
                if panel.updating.get() {
                    return;
                }
                let (Some(v), Some(state)) = (
                    colour::parse_channel(&entry.text()),
                    panel.picker.state.get(),
                ) else {
                    return;
                };
                let mut c = state.colour.to_rgba();
                match index {
                    0 => c.r = v,
                    1 => c.g = v,
                    _ => c.b = v,
                }
                if c != state.colour.to_rgba() {
                    panel.picker_set(Hsva::from_rgba(c));
                }
            });
        }
        // Enter in an entry commits, like closing the popover.
        for entry in std::iter::once(&picker.hex).chain(picker.channels.iter()) {
            let weak = Rc::downgrade(self);
            entry.connect_activate(move |_| {
                if let Some(panel) = weak.upgrade() {
                    panel.picker.popover.popdown();
                }
            });
        }
        let weak = Rc::downgrade(self);
        picker.pick_screen.connect_clicked(move |_| {
            let Some(panel) = weak.upgrade() else {
                return;
            };
            let key = panel.picker.state.get().map(|s| s.key);
            panel.picker.popover.popdown();
            if let Some(key) = key {
                panel.emit(PanelEvent::PickScreen(key));
            }
        });
    }

    // ---- showing ----

    /// Shows `info`'s rows with its values (`None`: hides the type properties).
    pub fn show(self: &Rc<Self>, info: Option<&PanelInfo>) {
        let Some(info) = info else {
            self.root.set_visible(false);
            self.info.replace(None);
            return;
        };
        let was = self.updating.replace(true);
        self.info.replace(Some(info.clone()));
        let rows = properties::rows(info);
        let names: Vec<&'static str> = rows.iter().map(|r| row_name(*r)).collect();
        for (name, widget) in &self.rows {
            widget.set_visible(names.contains(name));
        }
        let mut previous: Option<&gtk::Widget> = None;
        for name in &names {
            if let Some(widget) = self.rows.get(name) {
                self.root.reorder_child_after(widget, previous);
                previous = Some(widget);
            }
        }
        self.root.set_visible(!rows.is_empty());
        self.show_values(info);
        self.show_colours(info);
        self.show_shadow(info);
        self.updating.set(was);
    }

    fn show_values(&self, info: &PanelInfo) {
        let choose = |buttons: &[(&'static str, gtk::ToggleButton)], key: &str| {
            if let Some(current) = info.choice(key) {
                for (choice, button) in buttons {
                    if *choice == current && !button.is_active() {
                        button.set_active(true);
                    }
                }
            }
        };
        match info.type_name {
            Callout::TYPE => choose(&self.callout_shape, "shape"),
            Step::TYPE => choose(&self.step_shape, "shape"),
            Redaction::TYPE => choose(&self.redact_shape, "shape"),
            _ => choose(&self.shape_kind, "shape"),
        }
        choose(&self.redact_type, "redaction");
        choose(&self.line_style, "line_style");
        choose(&self.alignment, "alignment");
        for (key, button) in &self.flags {
            button.set_active(info.flag(key).unwrap_or(false));
        }
        // Ranges per type (the model refuses values outside them).
        let range = |key: &str| -> Option<(f64, f64)> {
            Some(match (info.type_name, key) {
                (pen::Pen::TYPE, "width") => (pen::MIN_WIDTH, pen::MAX_WIDTH),
                (_, "width") => (arrow::MIN_WIDTH, arrow::MAX_WIDTH),
                (shape::Shape::TYPE, "outline_width") => (0.0, shape::MAX_OUTLINE),
                (Callout::TYPE, "outline_width") => (0.0, callout::MAX_OUTLINE),
                (_, "outline_width") => (0.0, text::MAX_OUTLINE),
                (_, "padding") => (0.0, callout::MAX_PADDING),
                (_, "radius") => (step::MIN_RADIUS, step::MAX_RADIUS),
                (_, "font_size") => (text::MIN_FONT_SIZE, text::MAX_FONT_SIZE),
                (_, "start_size" | "end_size") => {
                    (arrow::MIN_END_SIZE * 100.0, arrow::MAX_END_SIZE * 100.0)
                }
                (_, "corner_radius") => (0.0, 1000.0),
                _ => return None,
            })
        };
        for (key, spin, factor) in &self.spins {
            if let Some((lower, upper)) = range(key) {
                spin.adjustment().set_lower(lower);
                spin.adjustment().set_upper(upper);
            }
            if let Some(v) = info.number(key) {
                spin.set_value(v * factor);
                // Shows the value even where the adjustment rounds it.
                spin.update();
            }
        }
        for (slider, key, factor, unit) in [
            (&self.opacity, "opacity", 100.0, "%"),
            (&self.fill_opacity, "fill_opacity", 100.0, "%"),
            (&self.smoothing, "smoothing", 1.0, ""),
            (&self.intensity, "intensity", 1.0, ""),
        ] {
            if let Some(v) = info.number(key) {
                slider.scale.set_value(v * factor);
                label_of(&slider.scale, &slider.label, unit);
            }
        }
        // Mockup 15: no intensity for a solid box; its colour only for it.
        let solid = info.choice("redaction") == Some(RedactionKind::Solid.key());
        if let Some(row) = self.rows.get("row_intensity") {
            row.set_sensitive(!solid);
        }
        if let Some(row) = self.rows.get("row_color") {
            row.set_sensitive(info.type_name != Redaction::TYPE || solid);
        }
        // Ends.
        let style = |key: &str| {
            info.choice(key)
                .and_then(EndStyle::from_key)
                .unwrap_or(EndStyle::None)
        };
        let ends = (style("start_style"), style("end_style"));
        self.shown_ends.set(ends);
        for (tiles, current) in [(&self.start_tiles, ends.0), (&self.end_tiles, ends.1)] {
            for (style, button) in tiles {
                button.set_active(*style == current);
            }
        }
        self.start_glyph.queue_draw();
        self.end_glyph.queue_draw();
        // Step.
        if let Some(index) = info
            .choice("counting")
            .and_then(Counting::from_key)
            .and_then(|c| Counting::ALL.iter().position(|k| *k == c))
        {
            self.sequence.set_selected(index as u32);
        }
        if let Some(next) = info.next_step {
            self.next_value.set_value(f64::from(next));
        }
        let value = info.text("value").unwrap_or_default();
        if self.value_entry.text() != value {
            self.value_entry.set_text(value);
        }
        // Font.
        if let Some(family) = info.text("font_family") {
            let position = (0..self.fonts.n_items()).find(|&i| {
                self.fonts
                    .string(i)
                    .is_some_and(|s| s.eq_ignore_ascii_case(family))
            });
            let position = position.unwrap_or_else(|| {
                self.fonts.append(family);
                self.fonts.n_items().saturating_sub(1)
            });
            if self.font_family.selected() != position {
                self.font_family.set_selected(position);
            }
        }
    }
}

/// The ring and check mark of the current swatch.
fn mark(button: &gtk::Button, current: bool) {
    if current == button.has_css_class("annota-swatch-current") {
        return;
    }
    if current {
        button.add_css_class("annota-swatch-current");
    } else {
        button.remove_css_class("annota-swatch-current");
    }
    if let Some(child) = button.child() {
        child.queue_draw();
    }
}

/// A swatch's tooltip: the palette colour's name, else its hex value.
fn swatch_name(c: Rgba) -> String {
    match c {
        palette::CORAL => gettext("Coral"),
        palette::AMBER => gettext("Amber"),
        palette::LIME => gettext("Lime"),
        palette::TEAL => gettext("Teal"),
        palette::AZURE => gettext("Azure"),
        palette::VIOLET => gettext("Violet"),
        palette::INK => gettext("Ink"),
        palette::PAPER => gettext("Paper"),
        colour::HIGHLIGHT_YELLOW => gettext("Yellow"),
        other => colour::hex(other),
    }
}

/// The saturation/value area (redline: 150 px high, radius 6) with its marker.
fn draw_sv_area(cr: &cairo::Context, w: f64, h: f64, c: Hsva) -> Result<(), cairo::Error> {
    let r = 6.0;
    cr.save()?;
    cr.new_sub_path();
    cr.arc(w - r, r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(w - r, h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(
        r,
        h - r,
        r,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    cr.arc(r, r, r, std::f64::consts::PI, 1.5 * std::f64::consts::PI);
    cr.close_path();
    cr.clip();
    let (red, green, blue) = colour::hsv_to_rgb(c.h, 1.0, 1.0);
    let across = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
    across.add_color_stop_rgb(0.0, 1.0, 1.0, 1.0);
    across.add_color_stop_rgb(1.0, red, green, blue);
    cr.set_source(&across)?;
    cr.paint()?;
    let down = cairo::LinearGradient::new(0.0, 0.0, 0.0, h);
    down.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.0);
    down.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 1.0);
    cr.set_source(&down)?;
    cr.paint()?;
    cr.restore()?;
    marker(cr, c.s * w, (1.0 - c.v) * h, c.to_rgba())
}

/// A slider knob: 18 px white disc with the colour inside (mockup 16).
fn marker(cr: &cairo::Context, x: f64, y: f64, inside: Rgba) -> Result<(), cairo::Error> {
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
    cr.arc(x, y + 0.5, 9.5, 0.0, std::f64::consts::TAU);
    cr.fill()?;
    cr.set_source_rgb(1.0, 1.0, 1.0);
    cr.arc(x, y, 9.0, 0.0, std::f64::consts::TAU);
    cr.fill()?;
    let (r, g, b, _) = inside.to_f64();
    cr.set_source_rgb(r, g, b);
    cr.arc(x, y, 6.0, 0.0, std::f64::consts::TAU);
    cr.fill()
}

/// The hue or alpha slider (redline: 12 px; alpha on a checker) and its knob.
fn draw_bar(
    cr: &cairo::Context,
    w: f64,
    h: f64,
    part: PickerPart,
    c: Hsva,
) -> Result<(), cairo::Error> {
    let (top, bar) = ((h - 12.0) / 2.0, 12.0);
    let inset = 9.0;
    let width = (w - 2.0 * inset).max(1.0);
    cr.save()?;
    cr.new_sub_path();
    cr.arc(
        inset,
        top + 6.0,
        6.0,
        0.5 * std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    cr.arc(
        inset + width,
        top + 6.0,
        6.0,
        -0.5 * std::f64::consts::PI,
        0.5 * std::f64::consts::PI,
    );
    cr.close_path();
    cr.clip();
    let gradient = cairo::LinearGradient::new(inset, 0.0, inset + width, 0.0);
    let position = match part {
        PickerPart::Hue => {
            for k in 0..=6 {
                let (r, g, b) = colour::hsv_to_rgb(f64::from(k) * 60.0, 1.0, 1.0);
                gradient.add_color_stop_rgb(f64::from(k) / 6.0, r, g, b);
            }
            c.h / 360.0
        }
        _ => {
            // Checker, then the colour from transparent to opaque.
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.paint()?;
            cr.set_source_rgb(0.8, 0.8, 0.8);
            let mut x = 0.0;
            let mut k = 0;
            while x < w {
                let y = if k % 2 == 0 { top } else { top + bar / 2.0 };
                cr.rectangle(x, y, bar / 2.0, bar / 2.0);
                x += bar / 2.0;
                k += 1;
            }
            cr.fill()?;
            let (r, g, b, _) = Hsva { a: 1.0, ..c }.to_rgba().to_f64();
            gradient.add_color_stop_rgba(0.0, r, g, b, 0.0);
            gradient.add_color_stop_rgba(1.0, r, g, b, 1.0);
            c.a
        }
    };
    cr.set_source(&gradient)?;
    cr.rectangle(0.0, top, w, bar);
    cr.fill()?;
    cr.restore()?;
    let knob = match part {
        PickerPart::Hue => Hsva {
            s: 1.0,
            v: 1.0,
            a: 1.0,
            ..c
        }
        .to_rgba(),
        _ => c.to_rgba(),
    };
    marker(cr, inset + position * width, h / 2.0, knob)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spin_texts_and_shown_values() {
        assert_eq!(spin_text(8.0, "px"), "8 px");
        assert_eq!(spin_text(100.0, "%"), "100 %");
        assert_eq!(spin_text(45.0, "°"), "45°");
        assert_eq!(spin_text(34.0, ""), "34");
        // A spin re-reading "8 px" for a stored 8.04 is not an edit.
        assert!(same_shown(8.04, 8.0));
        assert!(!same_shown(8.5, 8.0));
    }

    #[test]
    fn multi_selection_edits_are_never_suppressed() {
        let coral = PropertyValue::Color(palette::CORAL);
        let teal = PropertyValue::Color(palette::TEAL);
        // A single selected object: clicking its own colour is a no-op, a different one is not.
        assert!(unchanged(Mode::Selected(1), Some(&coral), &coral));
        assert!(!unchanged(Mode::Selected(1), Some(&coral), &teal));
        // Two selected objects, shown reflects only the first (Coral): clicking Coral must still
        // reach the others, which may be Teal — never suppress for a multi-selection.
        assert!(!unchanged(Mode::Selected(2), Some(&coral), &coral));
        assert!(!unchanged(Mode::Selected(3), Some(&coral), &teal));
        // Tool defaults and a single-object edit keep the guard.
        assert!(unchanged(Mode::ToolDefaults, Some(&coral), &coral));
    }

    #[test]
    fn every_row_has_a_widget_name() {
        use super::properties::Row;
        let names: std::collections::HashSet<&str> = ROW_NAMES.into_iter().collect();
        for row in [
            Row::Sequence,
            Row::ShapeKind,
            Row::CalloutShape,
            Row::StepShape,
            Row::RedactType,
            Row::Font,
            Row::TextStyle,
            Row::Colour {
                key: "color",
                set: SwatchSet::Palette,
            },
            Row::Colour {
                key: "fill",
                set: SwatchSet::Palette,
            },
            Row::Colour {
                key: "outline_color",
                set: SwatchSet::Palette,
            },
            Row::FillOpacity,
            Row::Width,
            Row::OutlineWidth,
            Row::OutlineAndRadius,
            Row::OutlineAndPadding,
            Row::Ends,
            Row::LineStyle,
            Row::Smoothing,
            Row::Intensity,
            Row::RedactShape,
            Row::Size,
            Row::Value,
            Row::NextValue,
            Row::RestartSequence,
            Row::Opacity,
            Row::Shadow,
            Row::HelpPen,
            Row::HelpPolygon,
            Row::HelpHighlighter,
            Row::HelpCallout,
            Row::WarningRedact,
        ] {
            assert!(names.contains(row_name(row)), "{row:?}");
        }
    }

    #[test]
    fn headings_name_the_mode() {
        let info = |mode, tails| PanelInfo {
            type_name: Callout::TYPE,
            mode,
            values: Vec::new(),
            tails,
            next_step: None,
        };
        assert_eq!(
            heading(&info(Mode::Selected(1), Some(2))),
            ("Callout".into(), "1 selected · 2 tails".into())
        );
        assert_eq!(heading(&info(Mode::ToolDefaults, None)).1, "Tool defaults");
        assert_eq!(heading(&info(Mode::Editing, None)).1, "Editing");
        assert_eq!(
            heading(&info(Mode::DrawingPolygon, None)).1,
            "Drawing polygon"
        );
        assert_eq!(colour_caption(Redaction::TYPE, "color"), "Solid colour");
        assert_eq!(colour_caption(Callout::TYPE, "color"), "Text colour");
        assert_eq!(colour_caption(Callout::TYPE, "fill"), "Fill");
    }

    #[test]
    fn picker_pieces_draw() {
        use annota::document::bitmap::Bitmap;
        let c = Hsva::from_rgba(Rgba::opaque(0x2F, 0x6F, 0xD0));
        let area = Bitmap::draw(276, 150, |cr| draw_sv_area(cr, 276.0, 150.0, c)).unwrap();
        // White at the top left, black at the bottom.
        assert!(area.pixel(2, 2).is_some_and(|p| p.r > 240 && p.g > 240));
        assert!(area.pixel(138, 148).is_some_and(|p| p.r < 20));
        let hue =
            Bitmap::draw(276, 18, |cr| draw_bar(cr, 276.0, 18.0, PickerPart::Hue, c)).unwrap();
        assert!(
            hue.pixel(12, 9).is_some_and(|p| p.r > 200 && p.b < 60),
            "red first"
        );
        let alpha = Bitmap::draw(276, 18, |cr| {
            draw_bar(cr, 276.0, 18.0, PickerPart::Alpha, c)
        })
        .unwrap();
        assert!(alpha.pixel(265, 9).is_some());
    }
}
