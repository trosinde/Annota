//! What the type properties of the sidebar show (add-core-annotation-tools task 3.1; redline
//! "Sidebar building blocks", mockups 01–18): for the selected objects, the text being edited,
//! a polygon being drawn, or — nothing selected, a drawing tool active — the tool's defaults.
//!
//! This extends the pattern the Image/Canvas controls of `super::sidebar` set up: the state is
//! computed here from the document without widgets ([`panel_info`]); which rows a type shows,
//! and in which order, is data ([`rows`]); `super::panel` shows it in the Blueprint rows of
//! `editor-window.blp` and turns edits into `properties`/`set_property` values — one
//! `Command` per committed change, none when the value shown did not change.
//!
//! The common rows follow the spec's "Common style properties" and "Style exceptions": every
//! type has opacity and a shadow, except the highlighter (no shadow) and the redaction (neither).

use annota::document::annotation::{Annotation, Property, PropertyValue};
use annota::document::color::Rgba;
use annota::document::scene::ObjectId;
use annota::document::style::Shadow;
use annota::document::Document;
use annota::editing::steps;
use annota::editing::tool::{Tool, Toolbox};
use annota::tools::arrow::Arrow;
use annota::tools::callout::Callout;
use annota::tools::highlighter::Highlighter;
use annota::tools::pen::Pen;
use annota::tools::redaction::Redaction;
use annota::tools::shape::Shape;
use annota::tools::step::Step;
use annota::tools::text::Text;

use super::colour::SwatchSet;

/// The types with type properties (arrow and line are one type saved under two tags).
pub const TYPES: [&str; 9] = [
    Arrow::TYPE,
    Arrow::LINE_TYPE,
    Pen::TYPE,
    Shape::TYPE,
    Text::TYPE,
    Callout::TYPE,
    Step::TYPE,
    Highlighter::TYPE,
    Redaction::TYPE,
];

/// Whose properties are shown (the heading's caption).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// "1 selected", "3 selected".
    Selected(usize),
    /// Nothing selected, a drawing tool active: its defaults for new objects (mockup 01).
    ToolDefaults,
    /// The text or callout being typed (mockup 08).
    Editing,
    /// A polygon being drawn with the shape tool (mockup 07).
    DrawingPolygon,
}

/// The type properties to show.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelInfo {
    pub type_name: &'static str,
    pub mode: Mode,
    /// The values of the (first) object, or of the tool's prototype.
    pub values: Vec<Property>,
    /// A single selected callout's tail count (mockup 09: "1 selected · 2 tails").
    pub tails: Option<usize>,
    /// The step tool's "Next value": the number the next marker gets.
    pub next_step: Option<u32>,
}

impl PanelInfo {
    pub fn value(&self, key: &str) -> Option<&PropertyValue> {
        self.values.iter().find(|p| p.key == key).map(|p| &p.value)
    }

    pub fn number(&self, key: &str) -> Option<f64> {
        match self.value(key)? {
            PropertyValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn colour(&self, key: &str) -> Option<Rgba> {
        match self.value(key)? {
            PropertyValue::Color(c) => Some(*c),
            _ => None,
        }
    }

    pub fn choice(&self, key: &str) -> Option<&str> {
        match self.value(key)? {
            PropertyValue::Choice(c) => Some(c),
            _ => None,
        }
    }

    pub fn flag(&self, key: &str) -> Option<bool> {
        match self.value(key)? {
            PropertyValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        match self.value(key)? {
            PropertyValue::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn shadow(&self) -> Option<Shadow> {
        match self.value("shadow")? {
            PropertyValue::Shadow(s) => Some(*s),
            _ => None,
        }
    }
}

/// One row of the type properties (redline "Sidebar building blocks").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    /// Step: Sequence dropdown (1, 2, 3 · A, B, C · a, b, c · I, II, III).
    Sequence,
    /// Shape: Rectangle / Rounded Rectangle / Ellipse / Polygon.
    ShapeKind,
    /// Callout: Rectangle / Rounded / Ellipse / Cloud.
    CalloutShape,
    /// Step: Circle / Square / Rounded Square / Pin.
    StepShape,
    /// Redact: Blur / Pixelate / Solid.
    RedactType,
    /// Font family dropdown and size spin.
    Font,
    /// Bold / Italic / Underline / Strikethrough and the alignment.
    TextStyle,
    /// A colour row: the property and its swatches.
    Colour {
        key: &'static str,
        set: SwatchSet,
    },
    FillOpacity,
    /// Width spin (arrow, line, pen).
    Width,
    /// "Outline width" as a row of its own (text, polygon).
    OutlineWidth,
    /// Outline width and corner radius side by side (rounded rectangle, mockup 06).
    OutlineAndRadius,
    /// Outline width and padding side by side (callout, mockup 09).
    OutlineAndPadding,
    /// Start/End styles and sizes.
    Ends,
    LineStyle,
    Smoothing,
    Intensity,
    /// Redact: Rectangle / Ellipse.
    RedactShape,
    /// Step marker size.
    Size,
    /// A single marker's own value ("This marker only").
    Value,
    NextValue,
    RestartSequence,
    Opacity,
    Shadow,
    HelpPen,
    HelpPolygon,
    HelpHighlighter,
    HelpCallout,
    WarningRedact,
}

/// The rows `info` shows, top to bottom ("Select All <Type>" follows them).
pub fn rows(info: &PanelInfo) -> Vec<Row> {
    use Row::*;
    let colour = Colour {
        key: "color",
        set: SwatchSet::Full { none: false },
    };
    let mut rows = match info.type_name {
        Arrow::TYPE | Arrow::LINE_TYPE => vec![colour, Width, Ends, LineStyle],
        Pen::TYPE => vec![colour, Width, Smoothing],
        Shape::TYPE => {
            let mut rows = vec![
                ShapeKind,
                Colour {
                    key: "outline_color",
                    set: SwatchSet::Palette,
                },
                Colour {
                    key: "fill",
                    set: SwatchSet::Full { none: true },
                },
            ];
            // Mockup 07: a fill's opacity only where there is a fill.
            if info.colour("fill").is_some_and(|c| c.a > 0) {
                rows.push(FillOpacity);
            }
            if info.choice("shape") == Some("rounded_rectangle") {
                rows.push(OutlineAndRadius);
            } else {
                rows.push(OutlineWidth);
            }
            if info.mode == Mode::DrawingPolygon {
                // Mockup 07: while drawing, the help card ends the panel.
                rows.push(HelpPolygon);
                return rows;
            }
            rows.push(LineStyle);
            rows
        }
        Text::TYPE => vec![
            Font,
            TextStyle,
            colour,
            Colour {
                key: "outline_color",
                set: SwatchSet::Outline,
            },
            OutlineWidth,
        ],
        Callout::TYPE => vec![
            CalloutShape,
            Colour {
                key: "fill",
                set: SwatchSet::Full { none: true },
            },
            Colour {
                key: "outline_color",
                set: SwatchSet::Palette,
            },
            OutlineAndPadding,
            Font,
            Colour {
                key: "color",
                set: SwatchSet::Text,
            },
            HelpCallout,
        ],
        Step::TYPE => {
            let mut rows = vec![Sequence, StepShape, colour, Size];
            // Redline: the value "only with one marker selected".
            if info.mode == Mode::Selected(1) {
                rows.push(Value);
            }
            rows.extend([NextValue, RestartSequence]);
            rows
        }
        Highlighter::TYPE => {
            return vec![
                Colour {
                    key: "color",
                    set: SwatchSet::Highlight,
                },
                Opacity,
                HelpHighlighter,
            ];
        }
        Redaction::TYPE => {
            let mut rows = vec![
                RedactType,
                Intensity,
                RedactShape,
                Colour {
                    key: "color",
                    set: SwatchSet::Palette,
                },
            ];
            if info.choice("redaction") != Some("solid") {
                rows.push(WarningRedact);
            }
            return rows;
        }
        _ => return Vec::new(),
    };
    rows.extend([Opacity, Shadow]);
    if info.type_name == Pen::TYPE {
        rows.push(HelpPen);
    }
    rows
}

/// Whether "Select All <Type>" ends the panel: for selections and tool defaults (mockups 01,
/// 02), not while a text is typed or a polygon drawn.
pub fn select_all(info: &PanelInfo) -> bool {
    matches!(info.mode, Mode::Selected(_) | Mode::ToolDefaults)
}

/// What the canvas is doing, as far as the type properties care.
pub struct CanvasFacts<'a> {
    pub selection: &'a [ObjectId],
    pub tool: Tool,
    pub toolbox: &'a Toolbox,
    /// The text or callout being edited (a copy with the text so far).
    pub editing: Option<&'a dyn Annotation>,
    pub drawing_polygon: bool,
    /// The sequence the next marker joins and the number it gets.
    pub next_step: (u32, u32),
}

/// The type properties for `doc` and what the canvas is doing, if any: the text being edited,
/// else the polygon being drawn, else the selection (all of one of the nine types), else —
/// nothing selected, a drawing tool active — the tool's defaults.
pub fn panel_info(doc: &Document, facts: &CanvasFacts<'_>) -> Option<PanelInfo> {
    let info = |type_name: &'static str, mode, object: &dyn Annotation| PanelInfo {
        type_name,
        mode,
        values: object.properties(),
        tails: None,
        next_step: None,
    };
    let mut panel = if let Some(edited) = facts.editing {
        let type_name = TYPES.into_iter().find(|t| *t == edited.type_name())?;
        info(type_name, Mode::Editing, edited)
    } else if facts.drawing_polygon {
        let prototype = facts.toolbox.prototype(Tool::Shape)?;
        let mut panel = info(Shape::TYPE, Mode::DrawingPolygon, prototype);
        panel.values = facts.toolbox.properties(Tool::Shape);
        panel
    } else if !facts.selection.is_empty() {
        let scene = doc.scene();
        let objects: Vec<&dyn Annotation> = facts
            .selection
            .iter()
            .filter_map(|id| scene.get(*id).map(|n| n.object.as_ref()))
            .collect();
        let first = *objects.first()?;
        let type_name = TYPES.into_iter().find(|t| *t == first.type_name())?;
        if objects.iter().any(|o| o.type_name() != type_name) {
            return None;
        }
        let mut panel = info(type_name, Mode::Selected(objects.len()), first);
        if objects.len() == 1 {
            panel.tails = first
                .as_any()
                .downcast_ref::<Callout>()
                .map(|c| c.tails.len());
        }
        panel
    } else {
        let tool = facts.tool;
        let type_name = TYPES.into_iter().find(|t| Some(*t) == tool.object_type())?;
        let mut panel = info(
            type_name,
            Mode::ToolDefaults,
            facts.toolbox.prototype(tool)?,
        );
        panel.values = facts.toolbox.properties(tool);
        // The Sequence a new marker joins counts like that sequence already does.
        if tool == Tool::Step {
            if let Some(counting) = steps::counting(doc.scene(), facts.next_step.0) {
                for p in &mut panel.values {
                    if p.key == "counting" {
                        p.value = PropertyValue::Choice(counting.key().into());
                    }
                }
            }
        }
        panel
    };
    if panel.type_name == Step::TYPE {
        panel.next_step = Some(facts.next_step.1);
    }
    Some(panel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::bitmap::Bitmap;
    use annota::document::color::palette;
    use annota::document::command::History;
    use annota::document::geometry::{Point, Rect};
    use annota::editing::ops;

    fn doc_with(objects: Vec<Box<dyn Annotation>>) -> (Document, Vec<ObjectId>) {
        let mut doc = Document::new(Bitmap::filled(800, 600, palette::PAPER).unwrap());
        let mut history = History::new();
        for object in objects {
            let edit = ops::add(&doc, object).unwrap();
            history.execute(&mut doc, "Add", edit.command).unwrap();
        }
        let ids = doc.scene().ids();
        (doc, ids)
    }

    fn facts<'a>(selection: &'a [ObjectId], tool: Tool, toolbox: &'a Toolbox) -> CanvasFacts<'a> {
        CanvasFacts {
            selection,
            tool,
            toolbox,
            editing: None,
            drawing_polygon: false,
            next_step: (1, 1),
        }
    }

    #[test]
    fn every_type_has_its_redline_rows() {
        let toolbox = Toolbox::default();
        let (doc, _) = doc_with(Vec::new());
        for tool in Tool::ALL.into_iter().skip(1) {
            let info = panel_info(&doc, &facts(&[], tool, &toolbox)).unwrap();
            assert_eq!(info.mode, Mode::ToolDefaults);
            let rows = rows(&info);
            assert!(!rows.is_empty(), "{tool:?}");
            // Spec "Style exceptions": no shadow for highlights, neither shadow nor opacity
            // for redactions; everything else has both.
            let has = |r: Row| rows.contains(&r);
            match tool {
                Tool::Highlighter => assert!(has(Row::Opacity) && !has(Row::Shadow)),
                Tool::Redact => assert!(!has(Row::Opacity) && !has(Row::Shadow)),
                _ => assert!(has(Row::Opacity) && has(Row::Shadow), "{tool:?}"),
            }
            // Every key a row edits is a property of the type.
            for row in &rows {
                if let Row::Colour { key, .. } = row {
                    assert!(info.value(key).is_some(), "{tool:?} {key}");
                }
            }
            assert!(select_all(&info));
        }
        // Mockup 02: Arrow — Colour, Width, Ends, Line style, Opacity, Shadow.
        let arrow = panel_info(&doc, &facts(&[], Tool::Arrow, &toolbox)).unwrap();
        assert_eq!(
            rows(&arrow),
            vec![
                Row::Colour {
                    key: "color",
                    set: SwatchSet::Full { none: false }
                },
                Row::Width,
                Row::Ends,
                Row::LineStyle,
                Row::Opacity,
                Row::Shadow
            ]
        );
        // Mockup 06: the default rounded frame (fill None) — radius beside the width, no fill
        // opacity.
        let shape = panel_info(&doc, &facts(&[], Tool::Shape, &toolbox)).unwrap();
        let shape_rows = rows(&shape);
        assert!(shape_rows.contains(&Row::OutlineAndRadius));
        assert!(!shape_rows.contains(&Row::FillOpacity));
        assert_eq!(shape_rows[0], Row::ShapeKind, "polygons are reachable");
        // Mockup 15: the warning card for blur and pixelate only.
        let redact = panel_info(&doc, &facts(&[], Tool::Redact, &toolbox)).unwrap();
        assert!(rows(&redact).contains(&Row::WarningRedact));
    }

    #[test]
    fn tool_defaults_hide_what_belongs_to_one_object() {
        // B2: no text and no marker value among the tool defaults; the Value row only for one
        // selected marker.
        let toolbox = Toolbox::default();
        let (doc, ids) = doc_with(vec![
            Box::new(Step::new(Point::new(50.0, 50.0), 1, 1)),
            Box::new(Step::new(Point::new(150.0, 50.0), 1, 2)),
        ]);
        let defaults = panel_info(&doc, &facts(&[], Tool::Step, &toolbox)).unwrap();
        assert!(defaults.value("value").is_none());
        assert!(!rows(&defaults).contains(&Row::Value));
        for tool in [Tool::Text, Tool::Callout] {
            let info = panel_info(&doc, &facts(&[], tool, &toolbox)).unwrap();
            assert!(info.value("text").is_none(), "{tool:?}");
        }
        let one = panel_info(&doc, &facts(&ids[..1], Tool::Select, &toolbox)).unwrap();
        assert_eq!(one.mode, Mode::Selected(1));
        assert!(rows(&one).contains(&Row::Value));
        let two = panel_info(&doc, &facts(&ids, Tool::Select, &toolbox)).unwrap();
        assert!(!rows(&two).contains(&Row::Value));
        assert_eq!(two.next_step, Some(1));
    }

    #[test]
    fn what_the_panel_is_for() {
        let toolbox = Toolbox::default();
        let mut callout = Callout::new(Rect::new(10.0, 10.0, 200.0, 80.0), Point::new(50.0, 200.0));
        callout.tails.push(Point::new(300.0, 300.0));
        let (doc, ids) = doc_with(vec![
            Box::new(callout),
            Box::new(Highlighter::new(Rect::new(0.0, 0.0, 10.0, 10.0))),
        ]);
        // Mockup 09: "1 selected · 2 tails".
        let one = panel_info(&doc, &facts(&ids[..1], Tool::Select, &toolbox)).unwrap();
        assert_eq!((one.type_name, one.tails), (Callout::TYPE, Some(2)));
        // Mixed types: no type properties.
        assert!(panel_info(&doc, &facts(&ids, Tool::Select, &toolbox)).is_none());
        // Nothing selected with Select: the document page instead.
        assert!(panel_info(&doc, &facts(&[], Tool::Select, &toolbox)).is_none());
        // Typing wins over the selection; a polygon over the tool defaults.
        let text = Text::new(Point::new(0.0, 0.0));
        let mut f = facts(&ids[1..], Tool::Text, &toolbox);
        f.editing = Some(&text);
        let editing = panel_info(&doc, &f).unwrap();
        assert_eq!(
            (editing.type_name, editing.mode),
            (Text::TYPE, Mode::Editing)
        );
        assert!(!select_all(&editing));
        let mut f = facts(&[], Tool::Shape, &toolbox);
        f.drawing_polygon = true;
        let drawing = panel_info(&doc, &f).unwrap();
        assert_eq!(drawing.mode, Mode::DrawingPolygon);
        assert_eq!(rows(&drawing).last(), Some(&Row::HelpPolygon));
    }

    #[test]
    fn the_step_tool_shows_how_the_current_sequence_counts() {
        let toolbox = Toolbox::default();
        let mut a = Step::new(Point::new(50.0, 50.0), 4, 1);
        a.counting = annota::tools::step::Counting::Roman;
        let (doc, _) = doc_with(vec![Box::new(a)]);
        let mut f = facts(&[], Tool::Step, &toolbox);
        f.next_step = (4, 2);
        let info = panel_info(&doc, &f).unwrap();
        assert_eq!(info.choice("counting"), Some("roman"));
        assert_eq!(info.next_step, Some(2));
    }
}
