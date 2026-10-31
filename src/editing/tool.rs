//! The drawing tools (spec annotation-tools: Tool switching and one requirement per tool):
//! which tools exist and their keys, the defaults new objects get ([`Toolbox`]: one prototype
//! object per tool, edited through the same `properties`/`set_property` as objects — the
//! properties sidebar will edit them as "Tool defaults"), and the gestures that create objects:
//!
//! * [`Draw`] — press, drag, release: arrows and lines (end to end, Shift 0/45/90°), pen strokes
//!   (Shift draws straight segments), shapes, highlights and redactions (a box; Shift a
//!   square), callouts (the body's box), text (a click), step markers (a click).
//! * [`PolygonDraft`] — a polygon, one click per vertex; Enter, a double-click or a click on
//!   the first vertex finishes it, Backspace removes the last vertex, Esc cancels.
//!
//! Like the Select tool's gestures, a draw changes nothing until it ends and then becomes one
//! `Edit` (one undo step). Text and callouts end in in-place text editing ([`TextEdit`]); the
//! object is added when that ends.

use crate::document::annotation::{Annotation, Modifiers, Property, PropertyError, PropertyValue};
use crate::document::geometry::{Arrange, Point, Rect};
use crate::document::scene::ObjectId;
use crate::document::Document;
use crate::render::Viewport;
use crate::tools::arrow::Arrow;
use crate::tools::callout::Callout;
use crate::tools::common::chip_angle;
use crate::tools::highlighter::Highlighter;
use crate::tools::pen::Pen;
use crate::tools::redaction::Redaction;
use crate::tools::shape::{Shape, ShapeKind};
use crate::tools::step::{Counting, Step};
use crate::tools::text::{editable, Text};

use super::drag::{self, Hover, DRAG_THRESHOLD};
use super::ops::{self, Edit};
use super::selection::Selection;
use super::text_edit::TextEdit;

/// The tools of the tool strip (decision D2's order; Crop is a mode of its own).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tool {
    #[default]
    Select,
    Arrow,
    Line,
    Pen,
    Shape,
    Text,
    Callout,
    Step,
    Highlighter,
    Redact,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::Select,
        Tool::Arrow,
        Tool::Line,
        Tool::Pen,
        Tool::Shape,
        Tool::Text,
        Tool::Callout,
        Tool::Step,
        Tool::Highlighter,
        Tool::Redact,
    ];

    /// The single-key shortcut (spec "Tool switching": V, A, L, P, U, T, K, S, H, B).
    pub fn shortcut(self) -> char {
        match self {
            Tool::Select => 'v',
            Tool::Arrow => 'a',
            Tool::Line => 'l',
            Tool::Pen => 'p',
            Tool::Shape => 'u',
            Tool::Text => 't',
            Tool::Callout => 'k',
            Tool::Step => 's',
            Tool::Highlighter => 'h',
            Tool::Redact => 'b',
        }
    }

    pub fn from_shortcut(key: char) -> Option<Tool> {
        let key = key.to_ascii_lowercase();
        Tool::ALL.into_iter().find(|t| t.shortcut() == key)
    }

    /// The type of the objects it makes.
    pub fn object_type(self) -> Option<&'static str> {
        match self {
            Tool::Select => None,
            Tool::Arrow => Some(Arrow::TYPE),
            Tool::Line => Some(Arrow::LINE_TYPE),
            Tool::Pen => Some(Pen::TYPE),
            Tool::Shape => Some(Shape::TYPE),
            Tool::Text => Some(Text::TYPE),
            Tool::Callout => Some(Callout::TYPE),
            Tool::Step => Some(Step::TYPE),
            Tool::Highlighter => Some(Highlighter::TYPE),
            Tool::Redact => Some(Redaction::TYPE),
        }
    }
}

/// The defaults of new objects, one prototype per tool (redline "Defaults (new objects)").
/// New objects copy their prototype's style; only their geometry is new.
#[derive(Clone, Debug)]
pub struct Toolbox {
    arrow: Arrow,
    line: Arrow,
    pen: Pen,
    shape: Shape,
    text: Text,
    callout: Callout,
    step: Step,
    highlighter: Highlighter,
    redaction: Redaction,
}

impl Default for Toolbox {
    fn default() -> Self {
        let p = Point::default();
        let r = Rect::default();
        Self {
            arrow: Arrow::new(p, p),
            line: Arrow::line(p, p),
            pen: Pen::new(p),
            shape: Shape::new(r),
            text: Text::new(p),
            callout: Callout::new(r, p),
            step: Step::new(p, 1, 1),
            highlighter: Highlighter::new(r),
            redaction: Redaction::new(r),
        }
    }
}

/// Where a new step marker goes in its sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StepPlace {
    pub sequence: u32,
    pub number: u32,
    /// How the sequence counts so far (`None`: a new sequence counts like the prototype).
    pub counting: Option<Counting>,
}

impl Toolbox {
    pub fn prototype(&self, tool: Tool) -> Option<&dyn Annotation> {
        Some(match tool {
            Tool::Select => return None,
            Tool::Arrow => &self.arrow,
            Tool::Line => &self.line,
            Tool::Pen => &self.pen,
            Tool::Shape => &self.shape,
            Tool::Text => &self.text,
            Tool::Callout => &self.callout,
            Tool::Step => &self.step,
            Tool::Highlighter => &self.highlighter,
            Tool::Redact => &self.redaction,
        })
    }

    fn prototype_mut(&mut self, tool: Tool) -> Option<&mut dyn Annotation> {
        Some(match tool {
            Tool::Select => return None,
            Tool::Arrow => &mut self.arrow,
            Tool::Line => &mut self.line,
            Tool::Pen => &mut self.pen,
            Tool::Shape => &mut self.shape,
            Tool::Text => &mut self.text,
            Tool::Callout => &mut self.callout,
            Tool::Step => &mut self.step,
            Tool::Highlighter => &mut self.highlighter,
            Tool::Redact => &mut self.redaction,
        })
    }

    /// The tool's defaults ("Tool defaults" in the sidebar).
    pub fn properties(&self, tool: Tool) -> Vec<Property> {
        self.prototype(tool)
            .map(Annotation::properties)
            .unwrap_or_default()
    }

    /// Changes a default of the tool; new objects get it.
    pub fn set_property(
        &mut self,
        tool: Tool,
        key: &str,
        value: &PropertyValue,
    ) -> Result<(), PropertyError> {
        match self.prototype_mut(tool) {
            Some(p) => p.set_property(key, value),
            None => Err(PropertyError::Unknown(key.to_string())),
        }
    }

    /// Whether the shape tool draws polygons (vertex by vertex) rather than boxes.
    pub fn draws_polygons(&self) -> bool {
        self.shape.kind == ShapeKind::Polygon
    }

    /// The shape style new polygons get.
    pub fn shape_style(&self) -> &Shape {
        &self.shape
    }

    /// A new step marker at `at`.
    pub fn step(&self, at: Point, place: StepPlace) -> Step {
        let mut step = self.step.clone();
        step.x = at.x;
        step.y = at.y;
        step.sequence = place.sequence;
        step.number = place.number;
        step.value = None;
        if let Some(counting) = place.counting {
            step.counting = counting;
        }
        step
    }

    /// A new text whose first line is centred vertically on `at` (where the caret appears).
    pub fn text(&self, at: Point) -> Text {
        let mut text = self.text.clone();
        text.text.clear();
        text.x = at.x;
        text.y = at.y;
        let h = text.rect().height;
        text.y -= h / 2.0;
        text
    }

    /// The object `tool` starts with at `at` (a box of no size, a line of no length …).
    fn start(&self, tool: Tool, at: Point, step: StepPlace) -> Option<Box<dyn Annotation>> {
        let rect = Rect::new(at.x, at.y, 0.0, 0.0);
        let boxed = |mut object: Box<dyn Annotation>| {
            object.set_arrange(Arrange::new(rect, 0.0));
            object
        };
        Some(match tool {
            Tool::Select => return None,
            Tool::Arrow | Tool::Line => {
                let mut a = if tool == Tool::Arrow {
                    self.arrow.clone()
                } else {
                    self.line.clone()
                };
                (a.start, a.end, a.middle) = (at, at, at);
                Box::new(a)
            }
            Tool::Pen => {
                let mut pen = self.pen.clone();
                pen.points = vec![at];
                Box::new(pen)
            }
            Tool::Shape => {
                let mut shape = self.shape.clone();
                shape.points.clear();
                if shape.kind == ShapeKind::Polygon {
                    shape.kind = ShapeKind::Rectangle;
                }
                boxed(Box::new(shape))
            }
            Tool::Highlighter => boxed(Box::new(self.highlighter.clone())),
            Tool::Redact => boxed(Box::new(self.redaction.clone())),
            Tool::Callout => {
                let mut c = self.callout.clone();
                c.text.clear();
                c.rotation = 0.0;
                (c.x, c.y, c.width, c.height) = (at.x, at.y, 0.0, 0.0);
                c.tails = vec![at];
                Box::new(c)
            }
            Tool::Text => Box::new(self.text(at)),
            Tool::Step => Box::new(self.step(at, step)),
        })
    }
}

/// What a press with a drawing tool does (maintainer decision after the phase-A review:
/// drawing tools always draw, except on a handle of the current selection).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// A handle of the selection: adjust what was just drawn (an arrow's curve or end, a
    /// box handle) without switching to Select — the Select tool's handle drag.
    Handle,
    /// The Text or Callout tool on the *selected* text or callout: edit its text in place.
    EditText(ObjectId),
    /// Anything else — empty canvas, any other object, the body of the selected object — draws
    /// a new object on top.
    Draw,
}

/// Routes a press at `screen` with the drawing tool `tool` (not Select). Objects never catch
/// the press with their body: a second highlight started inside the selected one is drawn
/// instead of moving it, an arrow started on the line of the selected arrow is a new arrow
/// instead of a move, an arrow started on another existing arrow is a new arrow. Only the
/// selection's handles take a press — including the selected arrow's curve diamond on its
/// middle, so the arrow just drawn can be bent without switching tools.
pub fn route_press(
    doc: &Document,
    selection: &Selection,
    viewport: &Viewport,
    screen: Point,
    tool: Tool,
) -> Press {
    if matches!(
        drag::hover(doc, selection, viewport, screen),
        Hover::Handle { .. }
    ) {
        return Press::Handle;
    }
    if matches!(tool, Tool::Text | Tool::Callout) {
        let own = drag::hit(doc, viewport, screen).filter(|id| {
            selection.contains(*id)
                && doc.scene().get(*id).is_some_and(|n| {
                    Some(n.object.type_name()) == tool.object_type()
                        && editable(n.object.as_ref()).is_some()
                })
        });
        if let Some(id) = own {
            return Press::EditText(id);
        }
    }
    Press::Draw
}

/// Size of a callout made with a click (no drag), document pixels.
pub const CLICK_CALLOUT: (f64, f64) = (320.0, 96.0);
/// Smallest box a drag makes, document pixels; smaller drags make nothing.
pub const MIN_BOX: f64 = 2.0;
/// Pen points closer than this (screen pixels) to the previous one are skipped.
pub const PEN_SPACING: f64 = 1.5;

/// Live values for the drawing chips.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawFeedback {
    None,
    /// An arrow or line drawn with Shift: its angle (redline: "0° · Shift: 0/45/90°").
    Angle {
        degrees: f64,
    },
}

/// What a finished draw gives: an object to add, or one whose text is edited first.
#[derive(Debug)]
pub enum Drawn {
    Edit(Edit),
    Text(TextEdit),
}

/// One draw, from press to release.
#[derive(Debug)]
pub struct Draw {
    tool: Tool,
    start_screen: Point,
    start: Point,
    object: Box<dyn Annotation>,
    moved: bool,
    /// Pen: the last freehand point while Shift draws a straight segment from it.
    straight_from: Option<usize>,
    feedback: DrawFeedback,
}

/// The box from `a` to `b`; with `square`, the larger side both ways (towards the pointer).
fn drag_box(a: Point, b: Point, square: bool) -> Rect {
    let (mut dx, mut dy) = (b.x - a.x, b.y - a.y);
    if square {
        let side = dx.abs().max(dy.abs());
        dx = side.copysign(if dx == 0.0 { 1.0 } else { dx });
        dy = side.copysign(if dy == 0.0 { 1.0 } else { dy });
    }
    Rect::from_points(a, Point::new(a.x + dx, a.y + dy))
}

impl Draw {
    /// A press with `tool` at `screen` (view coordinates). `step` says where a step marker
    /// would go. `None` for the Select tool.
    pub fn press(
        toolbox: &Toolbox,
        tool: Tool,
        step: StepPlace,
        viewport: &Viewport,
        screen: Point,
    ) -> Option<Draw> {
        let start = viewport.to_document(screen);
        let object = toolbox.start(tool, start, step)?;
        Some(Draw {
            tool,
            start_screen: screen,
            start,
            object,
            moved: false,
            straight_from: None,
            feedback: DrawFeedback::None,
        })
    }

    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// The pointer moved to `screen` with the button held.
    pub fn motion(&mut self, viewport: &Viewport, screen: Point, modifiers: Modifiers) {
        if !self.moved {
            if screen.distance(self.start_screen) < DRAG_THRESHOLD {
                return;
            }
            self.moved = true;
        }
        let at = viewport.to_document(screen);
        self.feedback = DrawFeedback::None;
        let any = self.object.as_any_mut();
        match self.tool {
            Tool::Select => {}
            Tool::Arrow | Tool::Line => {
                if let Some(a) = any.downcast_mut::<Arrow>() {
                    a.draw_to(at, modifiers);
                    if modifiers.shift {
                        self.feedback = DrawFeedback::Angle {
                            degrees: chip_angle(a.start, a.end),
                        };
                    }
                }
            }
            Tool::Pen => {
                if let Some(pen) = any.downcast_mut::<Pen>() {
                    if modifiers.shift {
                        let from = *self
                            .straight_from
                            .get_or_insert(pen.points.len().saturating_sub(1));
                        pen.points.truncate(from + 1);
                        pen.points.push(at);
                    } else {
                        self.straight_from = None;
                        let far = pen
                            .points
                            .last()
                            .is_none_or(|last| last.distance(at) * viewport.zoom >= PEN_SPACING);
                        if far {
                            pen.points.push(at);
                        }
                    }
                }
            }
            Tool::Shape | Tool::Highlighter | Tool::Redact => {
                let rect = drag_box(self.start, at, modifiers.shift);
                self.object.set_arrange(Arrange::new(rect, 0.0));
            }
            Tool::Callout => {
                if let Some(c) = any.downcast_mut::<Callout>() {
                    let rect = drag_box(self.start, at, modifiers.shift);
                    (c.x, c.y, c.width, c.height) = (rect.x, rect.y, rect.width, rect.height);
                    c.tails = vec![Callout::default_tip(rect)];
                }
            }
            Tool::Step => {
                if let Some(s) = any.downcast_mut::<Step>() {
                    (s.x, s.y) = (at.x, at.y);
                }
            }
            Tool::Text => {}
        }
    }

    /// The object as drawn so far, for the canvas (nothing before the pointer moved for tools
    /// that need a drag).
    pub fn preview(&self) -> Option<&dyn Annotation> {
        let needs_drag = !matches!(self.tool, Tool::Step | Tool::Pen);
        (self.moved || !needs_drag).then_some(self.object.as_ref())
    }

    pub fn feedback(&self) -> DrawFeedback {
        self.feedback
    }

    pub fn is_dragging(&self) -> bool {
        self.moved
    }

    /// Released: what to add (one undo step), or the text to edit first.
    pub fn release(mut self, doc: &Document) -> Option<Drawn> {
        let size = self.object.arrange().rect;
        match self.tool {
            Tool::Select => None,
            Tool::Arrow | Tool::Line | Tool::Shape | Tool::Highlighter | Tool::Redact
                if !self.moved =>
            {
                None
            }
            Tool::Shape | Tool::Highlighter | Tool::Redact
                if size.width < MIN_BOX || size.height < MIN_BOX =>
            {
                None
            }
            Tool::Text => TextEdit::new_object(self.object, false).map(Drawn::Text),
            Tool::Callout => {
                if let Some(c) = self.object.as_any_mut().downcast_mut::<Callout>() {
                    if !self.moved || c.width < MIN_BOX || c.height < MIN_BOX {
                        let (w, h) = CLICK_CALLOUT;
                        let rect = Rect::new(self.start.x - w / 2.0, self.start.y - h / 2.0, w, h);
                        (c.x, c.y, c.width, c.height) = (rect.x, rect.y, rect.width, rect.height);
                        c.tails = vec![Callout::default_tip(rect)];
                    }
                }
                TextEdit::new_object(self.object, true).map(Drawn::Text)
            }
            _ => ops::add(doc, self.object).map(Drawn::Edit),
        }
    }
}

/// A polygon being drawn, vertex by vertex (mockup 07).
#[derive(Clone, Debug, PartialEq)]
pub struct PolygonDraft {
    vertices: Vec<Point>,
    pointer: Point,
}

/// A click this close (screen pixels) to the first vertex closes the polygon; this close to the
/// last vertex adds none (the second click of a double-click).
pub const CLOSE_DISTANCE: f64 = 8.0;

impl PolygonDraft {
    pub fn new(at: Point) -> Self {
        Self {
            vertices: vec![at],
            pointer: at,
        }
    }

    pub fn vertices(&self) -> &[Point] {
        &self.vertices
    }

    pub fn pointer(&self) -> Point {
        self.pointer
    }

    pub fn set_pointer(&mut self, at: Point) {
        self.pointer = at;
    }

    /// Whether a click at `screen` is on the first vertex of a polygon that can close.
    pub fn closes(&self, viewport: &Viewport, screen: Point) -> bool {
        self.vertices.len() >= 3
            && self
                .vertices
                .first()
                .is_some_and(|v| viewport.to_device(*v).distance(screen) <= CLOSE_DISTANCE)
    }

    /// A click at `screen`: a new vertex, unless it is on the last one.
    pub fn click(&mut self, viewport: &Viewport, screen: Point) {
        let at = viewport.to_document(screen);
        self.pointer = at;
        let on_last = self
            .vertices
            .last()
            .is_some_and(|v| viewport.to_device(*v).distance(screen) <= DRAG_THRESHOLD);
        if !on_last {
            self.vertices.push(at);
        }
    }

    /// Backspace: removes the last vertex; `false` if none is left (the draft ends).
    pub fn remove_last(&mut self) -> bool {
        self.vertices.pop();
        !self.vertices.is_empty()
    }

    /// Finishes the polygon with the shape tool's style: one undo step, or `None` with fewer
    /// than three vertices.
    pub fn finish(self, doc: &Document, toolbox: &Toolbox) -> Option<Edit> {
        let shape = Shape::polygon(&self.vertices, toolbox.shape_style())?;
        ops::add(doc, Box::new(shape))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::{palette, Rgba};
    use crate::document::command::History;
    use crate::editing::ops::{Action, Subject};
    use crate::tools::arrow::EndStyle;

    fn viewport() -> Viewport {
        Viewport::new(0.5, Point::new(44.0, 44.0))
    }

    fn screen(p: Point) -> Point {
        viewport().to_device(p)
    }

    fn doc() -> Document {
        Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap())
    }

    const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
    };
    const SHIFT: Modifiers = Modifiers {
        shift: true,
        ctrl: false,
        alt: false,
    };

    fn place() -> StepPlace {
        StepPlace {
            sequence: 1,
            number: 1,
            counting: None,
        }
    }

    /// Press at `from`, move through `path`, release (document points).
    fn draw(
        doc: &Document,
        toolbox: &Toolbox,
        tool: Tool,
        from: Point,
        path: &[Point],
        modifiers: Modifiers,
    ) -> Option<Drawn> {
        let vp = viewport();
        let mut d = Draw::press(toolbox, tool, place(), &vp, screen(from))?;
        for p in path {
            d.motion(&vp, screen(*p), modifiers);
        }
        d.release(doc)
    }

    fn added(drawn: Option<Drawn>) -> Edit {
        match drawn {
            Some(Drawn::Edit(edit)) => edit,
            other => panic!("an edit expected, got {other:?}"),
        }
    }

    fn object(edit: &Edit) -> &dyn Annotation {
        match &edit.command {
            crate::document::command::Command::Insert { nodes, .. } => nodes[0].1.object.as_ref(),
            other => panic!("insert expected: {other:?}"),
        }
    }

    #[test]
    fn keys_switch_tools() {
        // Spec "Key switches tool": A → arrow, and every tool has its key.
        assert_eq!(Tool::from_shortcut('a'), Some(Tool::Arrow));
        assert_eq!(Tool::from_shortcut('A'), Some(Tool::Arrow));
        let keys: String = Tool::ALL.iter().map(|t| t.shortcut()).collect();
        assert_eq!(keys, "valputkshb");
        assert_eq!(
            Tool::from_shortcut('c'),
            None,
            "Crop is a mode of the canvas"
        );
        assert_eq!(Tool::Redact.object_type(), Some("redaction"));
    }

    #[test]
    fn an_arrow_is_drawn_end_to_end_as_one_step() {
        let d = doc();
        let tb = Toolbox::default();
        let edit = added(draw(
            &d,
            &tb,
            Tool::Arrow,
            Point::new(100.0, 100.0),
            &[Point::new(300.0, 200.0), Point::new(500.0, 300.0)],
            NONE,
        ));
        assert_eq!(
            (edit.action, edit.subject),
            (Action::Add, Subject::One("arrow"))
        );
        let a = object(&edit).as_any().downcast_ref::<Arrow>().unwrap();
        assert_eq!(
            (a.start, a.end),
            (Point::new(100.0, 100.0), Point::new(500.0, 300.0))
        );
        assert_eq!(a.end_style, EndStyle::Triangle);
        assert_eq!(edit.select, Some(vec![d.scene().fresh_ids(1)[0]]));
        // A click makes nothing.
        assert!(draw(&d, &tb, Tool::Arrow, Point::new(100.0, 100.0), &[], NONE).is_none());
    }

    #[test]
    fn shift_constrains_the_arrow_and_shows_the_angle() {
        let tb = Toolbox::default();
        let vp = viewport();
        let mut d = Draw::press(
            &tb,
            Tool::Line,
            place(),
            &vp,
            screen(Point::new(100.0, 100.0)),
        )
        .unwrap();
        d.motion(&vp, screen(Point::new(400.0, 112.0)), SHIFT);
        assert_eq!(d.feedback(), DrawFeedback::Angle { degrees: 0.0 });
        let line = d
            .preview()
            .unwrap()
            .as_any()
            .downcast_ref::<Arrow>()
            .unwrap();
        assert_eq!(line.end.y, 100.0);
        // Spec "Plain line": no arrowheads.
        assert_eq!(
            (line.start_style, line.end_style),
            (EndStyle::None, EndStyle::None)
        );
        d.motion(&vp, screen(Point::new(400.0, 112.0)), NONE);
        assert_eq!(d.feedback(), DrawFeedback::None);
    }

    #[test]
    fn pen_strokes_and_straight_segments() {
        let d = doc();
        let tb = Toolbox::default();
        let vp = viewport();
        let mut draw =
            Draw::press(&tb, Tool::Pen, place(), &vp, screen(Point::new(0.0, 0.0))).unwrap();
        for k in 1..=10 {
            draw.motion(&vp, screen(Point::new(f64::from(k) * 10.0, 0.0)), NONE);
        }
        // Shift: a straight segment from the last freehand point, following the pointer.
        draw.motion(&vp, screen(Point::new(150.0, 80.0)), SHIFT);
        draw.motion(&vp, screen(Point::new(200.0, 100.0)), SHIFT);
        let pen = draw
            .preview()
            .unwrap()
            .as_any()
            .downcast_ref::<Pen>()
            .unwrap();
        assert_eq!(pen.points.last(), Some(&Point::new(200.0, 100.0)));
        assert_eq!(pen.points[pen.points.len() - 2], Point::new(100.0, 0.0));
        let edit = added(draw.release(&d));
        assert_eq!(edit.subject, Subject::One("pen"));
        // A click is a dot.
        let dot = added(super::tests::draw(
            &d,
            &tb,
            Tool::Pen,
            Point::new(5.0, 5.0),
            &[],
            NONE,
        ));
        assert!(object(&dot).hit_test(Point::new(5.0, 5.0), 0.0));
    }

    #[test]
    fn boxes_shift_squares_and_tiny_drags() {
        let d = doc();
        let tb = Toolbox::default();
        let edit = added(draw(
            &d,
            &tb,
            Tool::Shape,
            Point::new(100.0, 100.0),
            &[Point::new(300.0, 160.0)],
            SHIFT,
        ));
        assert_eq!(
            object(&edit).arrange().rect,
            Rect::new(100.0, 100.0, 200.0, 200.0)
        );
        let edit = added(draw(
            &d,
            &tb,
            Tool::Highlighter,
            Point::new(100.0, 100.0),
            &[Point::new(40.0, 130.0)],
            NONE,
        ));
        assert_eq!(
            object(&edit).arrange().rect,
            Rect::new(40.0, 100.0, 60.0, 30.0)
        );
        // A flat drag makes no box.
        assert!(draw(
            &d,
            &tb,
            Tool::Redact,
            Point::new(100.0, 100.0),
            &[Point::new(300.0, 100.5)],
            NONE
        )
        .is_none());
    }

    #[test]
    fn text_and_callouts_start_editing() {
        let d = doc();
        let tb = Toolbox::default();
        match draw(&d, &tb, Tool::Text, Point::new(100.0, 100.0), &[], NONE) {
            Some(Drawn::Text(edit)) => {
                assert_eq!(edit.text(), "");
                assert_eq!(edit.id(), None);
                assert!(edit.finish(&d).is_none(), "nothing typed: nothing added");
            }
            other => panic!("text editing expected: {other:?}"),
        }
        match draw(&d, &tb, Tool::Callout, Point::new(400.0, 300.0), &[], NONE) {
            Some(Drawn::Text(edit)) => {
                let c = edit.object().as_any().downcast_ref::<Callout>().unwrap();
                assert_eq!((c.width, c.height), CLICK_CALLOUT);
                assert_eq!(c.body().centre(), Point::new(400.0, 300.0));
                assert_eq!(c.tails.len(), 1);
                assert!(
                    edit.finish(&d).is_some(),
                    "a callout is added even without text"
                );
            }
            other => panic!("text editing expected: {other:?}"),
        }
    }

    #[test]
    fn steps_are_placed_where_released() {
        let d = doc();
        let tb = Toolbox::default();
        let vp = viewport();
        let at = StepPlace {
            sequence: 3,
            number: 4,
            counting: Some(Counting::UpperLetters),
        };
        let mut draw =
            Draw::press(&tb, Tool::Step, at, &vp, screen(Point::new(100.0, 100.0))).unwrap();
        draw.motion(&vp, screen(Point::new(140.0, 120.0)), NONE);
        let edit = added(draw.release(&d));
        let s = object(&edit).as_any().downcast_ref::<Step>().unwrap();
        assert_eq!(
            (s.x, s.y, s.sequence, s.label().as_str()),
            (140.0, 120.0, 3, "D")
        );
    }

    #[test]
    fn tool_defaults_are_prototypes() {
        let d = doc();
        let mut tb = Toolbox::default();
        tb.set_property(Tool::Arrow, "color", &PropertyValue::Color(palette::AZURE))
            .unwrap();
        assert!(tb
            .set_property(Tool::Select, "color", &PropertyValue::Color(palette::AZURE))
            .is_err());
        assert!(tb
            .properties(Tool::Arrow)
            .iter()
            .any(|p| p.key == "color" && p.value == PropertyValue::Color(palette::AZURE)));
        let edit = added(draw(
            &d,
            &tb,
            Tool::Arrow,
            Point::new(0.0, 0.0),
            &[Point::new(90.0, 0.0)],
            NONE,
        ));
        let a = object(&edit).as_any().downcast_ref::<Arrow>().unwrap();
        assert_eq!(a.color, palette::AZURE);
        // Each tool's prototype is its redline default type.
        for tool in Tool::ALL {
            assert_eq!(
                tb.prototype(tool).map(|p| p.type_name()),
                tool.object_type()
            );
        }
    }

    #[test]
    fn polygons_vertex_by_vertex() {
        let mut d = doc();
        let mut history = History::new();
        let mut tb = Toolbox::default();
        tb.set_property(
            Tool::Shape,
            "shape",
            &PropertyValue::Choice("polygon".into()),
        )
        .unwrap();
        assert!(tb.draws_polygons());
        let vp = viewport();
        let pts = [
            Point::new(600.0, 520.0),
            Point::new(960.0, 470.0),
            Point::new(1130.0, 600.0),
            Point::new(1060.0, 860.0),
        ];
        let mut draft = PolygonDraft::new(pts[0]);
        for p in &pts[1..] {
            draft.click(&vp, screen(*p));
        }
        // The second click of a double-click adds nothing.
        draft.click(&vp, screen(Point::new(1061.0, 861.0)));
        assert_eq!(draft.vertices().len(), 4);
        assert!(draft.closes(&vp, screen(Point::new(602.0, 522.0))));
        // Backspace removes the last point.
        let mut back = draft.clone();
        assert!(back.remove_last());
        assert_eq!(back.vertices().len(), 3);
        let edit = draft.finish(&d, &tb).unwrap();
        history.execute(&mut d, "Add", edit.command).unwrap();
        let shape = d.scene().nodes()[0]
            .object
            .as_any()
            .downcast_ref::<Shape>()
            .unwrap();
        assert_eq!(shape.kind, ShapeKind::Polygon);
        assert_eq!(shape.points.len(), 4);
        // Too few points: nothing.
        let two = PolygonDraft::new(pts[0]);
        assert!(two.finish(&d, &tb).is_none());
        let mut one = PolygonDraft::new(pts[0]);
        assert!(!one.remove_last());
    }

    /// Draws with `tool` from `from` to `to` on `doc` and executes it; the new object is the
    /// selection afterwards, as on the canvas.
    fn draw_into(
        doc: &mut Document,
        history: &mut History,
        selection: &mut Selection,
        tool: Tool,
        from: Point,
        to: Point,
    ) -> ObjectId {
        let tb = Toolbox::default();
        let edit = added(draw(doc, &tb, tool, from, &[to], NONE));
        let select = edit.select.clone().unwrap();
        history.execute(doc, "Add", edit.command).unwrap();
        selection.set(doc.scene(), select.clone());
        select[0]
    }

    fn json(doc: &Document, id: ObjectId) -> serde_json::Value {
        doc.scene().get(id).unwrap().object.to_json().unwrap()
    }

    #[test]
    fn a_second_arrow_started_on_an_existing_arrow_is_a_new_arrow() {
        // Phase-A review repro: starting a second arrow at the middle of an existing arrow
        // bent the existing one instead of drawing. Drawing tools now always draw, except on a
        // handle of the *current* selection.
        let mut d = doc();
        let mut history = History::new();
        let mut sel = Selection::new();
        let vp = viewport();
        let first = draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Arrow,
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
        );
        // Another arrow is drawn; the first is no longer selected.
        draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Arrow,
            Point::new(100.0, 600.0),
            Point::new(500.0, 600.0),
        );
        let before = json(&d, first);
        let middle = Point::new(300.0, 100.0);
        assert_eq!(
            route_press(&d, &sel, &vp, screen(middle), Tool::Arrow),
            Press::Draw
        );
        let third = draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Arrow,
            middle,
            Point::new(300.0, 400.0),
        );
        assert_eq!(d.scene().len(), 3, "a new arrow");
        assert_eq!(json(&d, first), before, "the first arrow is untouched");
        let a = d
            .scene()
            .get(third)
            .unwrap()
            .object
            .as_any()
            .downcast_ref::<Arrow>()
            .unwrap();
        assert_eq!((a.start, a.end), (middle, Point::new(300.0, 400.0)));
    }

    #[test]
    fn the_selected_arrow_keeps_its_handles_but_not_its_line() {
        // Fine-tuning what was just drawn still works on its handles; a press on its line
        // (which used to move it) draws a new arrow.
        let mut d = doc();
        let mut history = History::new();
        let mut sel = Selection::new();
        let vp = viewport();
        let arrow = draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Arrow,
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
        );
        assert_eq!(sel.ids(), &[arrow]);
        // The curve diamond (middle), and the ends.
        for handle in [
            Point::new(300.0, 100.0),
            Point::new(100.0, 100.0),
            Point::new(500.0, 100.0),
        ] {
            assert_eq!(
                route_press(&d, &sel, &vp, screen(handle), Tool::Arrow),
                Press::Handle,
                "{handle:?}"
            );
        }
        // On the line, away from the handles.
        assert_eq!(
            route_press(&d, &sel, &vp, screen(Point::new(200.0, 100.0)), Tool::Arrow),
            Press::Draw
        );
        // Any drawing tool, the same.
        assert_eq!(
            route_press(&d, &sel, &vp, screen(Point::new(200.0, 100.0)), Tool::Pen),
            Press::Draw
        );
    }

    #[test]
    fn a_second_highlight_started_inside_the_selected_one_is_drawn() {
        // Phase-A review repro: a second highlight started inside an existing (just drawn,
        // selected) one moved it.
        let mut d = doc();
        let mut history = History::new();
        let mut sel = Selection::new();
        let vp = viewport();
        let first = draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Highlighter,
            Point::new(100.0, 100.0),
            Point::new(700.0, 300.0),
        );
        assert_eq!(sel.ids(), &[first]);
        let before = json(&d, first);
        let inside = Point::new(300.0, 200.0);
        assert_eq!(
            route_press(&d, &sel, &vp, screen(inside), Tool::Highlighter),
            Press::Draw
        );
        draw_into(
            &mut d,
            &mut history,
            &mut sel,
            Tool::Highlighter,
            inside,
            Point::new(600.0, 260.0),
        );
        assert_eq!(d.scene().len(), 2, "a second highlight");
        assert_eq!(json(&d, first), before, "the first one did not move");
        // Its resize handles still resize it while it is selected.
        sel.set(d.scene(), [first]);
        assert_eq!(
            route_press(
                &d,
                &sel,
                &vp,
                screen(Point::new(700.0, 300.0)),
                Tool::Highlighter
            ),
            Press::Handle
        );
    }

    #[test]
    fn text_tools_edit_only_the_selected_text() {
        let mut d = doc();
        let mut history = History::new();
        let mut text = Toolbox::default().text(Point::new(200.0, 200.0));
        text.text = "Runs nightly".into();
        let edit = ops::add(&d, Box::new(text)).unwrap();
        history.execute(&mut d, "Add", edit.command).unwrap();
        let id = d.scene().ids()[0];
        let centre = d.scene().get(id).unwrap().object.arrange().rect.centre();
        let vp = viewport();
        let mut sel = Selection::new();
        // Not selected: a new text on top.
        assert_eq!(
            route_press(&d, &sel, &vp, screen(centre), Tool::Text),
            Press::Draw
        );
        // Selected: its text is edited; the callout tool does not edit a text.
        sel.set(d.scene(), [id]);
        assert_eq!(
            route_press(&d, &sel, &vp, screen(centre), Tool::Text),
            Press::EditText(id)
        );
        assert_eq!(
            route_press(&d, &sel, &vp, screen(centre), Tool::Callout),
            Press::Draw
        );
    }
}
