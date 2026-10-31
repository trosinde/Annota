//! Editing text in place (spec annotation-tools: Text — "Edit in place"; mockup 08): the text
//! of a text object or a callout is edited on the canvas with a caret and a selection.
//!
//! The session works on a copy of the object; the document is untouched until it finishes
//! (Esc, a click outside, another tool, undo …), which makes it **one** undo step: inserting a
//! new object, replacing the edited one, or removing a text object whose text was deleted.
//! Indices are byte offsets into the text, always on char boundaries.

use crate::document::annotation::{Annotation, PropertyValue};
use crate::document::command::Command;
use crate::document::geometry::Point;
use crate::document::scene::ObjectId;
use crate::document::Document;
use crate::tools::text::{editable, editable_mut, floor_boundary, TextBlock};

use super::ops::{self, Action, Edit, Subject};

/// Where the caret goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    Up,
    Down,
    LineStart,
    LineEnd,
    Start,
    End,
}

#[derive(Debug)]
pub struct TextEdit {
    /// The edited object's id; `None` for a new object that is not in the document yet.
    id: Option<ObjectId>,
    /// The object with the text as edited so far.
    object: Box<dyn Annotation>,
    /// As it was when editing started (existing objects).
    original: Option<Box<dyn Annotation>>,
    /// A new object is added even without text (a callout is a shape too).
    keep_empty: bool,
    caret: usize,
    /// The other end of the selection (equal to `caret`: nothing selected).
    anchor: usize,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl TextEdit {
    /// Editing a new object (text, callout) that is added when editing ends. `None` if the
    /// object has no editable text.
    pub fn new_object(object: Box<dyn Annotation>, keep_empty: bool) -> Option<Self> {
        let len = editable(object.as_ref())?.text().len();
        Some(Self {
            id: None,
            object,
            original: None,
            keep_empty,
            caret: len,
            anchor: len,
        })
    }

    /// Editing the text of object `id` of the document, the caret at the end.
    pub fn existing(doc: &Document, id: ObjectId) -> Option<Self> {
        let object = doc.scene().get(id)?.object.clone();
        let len = editable(object.as_ref())?.text().len();
        Some(Self {
            id: Some(id),
            original: Some(object.clone()),
            object,
            keep_empty: true,
            caret: len,
            anchor: len,
        })
    }

    pub fn id(&self) -> Option<ObjectId> {
        self.id
    }

    /// The object as edited so far (drawn instead of the original).
    pub fn object(&self) -> &dyn Annotation {
        self.object.as_ref()
    }

    pub fn text(&self) -> &str {
        editable(self.object.as_ref()).map_or("", |e| e.text())
    }

    pub fn block(&self) -> Option<TextBlock> {
        editable(self.object.as_ref()).map(|e| e.text_block())
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selection as an ordered byte range (empty when nothing is selected).
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn selected_text(&self) -> &str {
        let (s, e) = self.selection();
        self.text().get(s..e).unwrap_or("")
    }

    fn set_text(&mut self, text: String) {
        if let Some(e) = editable_mut(self.object.as_mut()) {
            e.set_text(text);
        }
    }

    /// Types `s` over the selection.
    pub fn insert(&mut self, s: &str) {
        let (start, end) = self.selection();
        let mut text = self.text().to_string();
        text.replace_range(start..end, s);
        self.set_text(text);
        self.caret = start + s.len();
        self.anchor = self.caret;
    }

    fn prev(&self, i: usize) -> usize {
        let text = self.text();
        text[..i]
            .chars()
            .next_back()
            .map_or(0, |c| i - c.len_utf8())
    }

    fn next(&self, i: usize) -> usize {
        let text = self.text();
        text[i..].chars().next().map_or(i, |c| i + c.len_utf8())
    }

    fn word_left(&self, i: usize) -> usize {
        let text = self.text();
        let mut i = i;
        while i > 0 && !text[..i].chars().next_back().is_some_and(is_word) {
            i = self.prev(i);
        }
        while i > 0 && text[..i].chars().next_back().is_some_and(is_word) {
            i = self.prev(i);
        }
        i
    }

    fn word_right(&self, i: usize) -> usize {
        let text = self.text();
        let mut i = i;
        while i < text.len() && !text[i..].chars().next().is_some_and(is_word) {
            i = self.next(i);
        }
        while i < text.len() && text[i..].chars().next().is_some_and(is_word) {
            i = self.next(i);
        }
        i
    }

    fn target(&self, motion: Motion) -> usize {
        let i = self.caret;
        let block = || self.block();
        match motion {
            Motion::Left => self.prev(i),
            Motion::Right => self.next(i),
            Motion::WordLeft => self.word_left(i),
            Motion::WordRight => self.word_right(i),
            Motion::Up => block().map_or(0, |b| b.vertical(i, -1)),
            Motion::Down => block().map_or(self.text().len(), |b| b.vertical(i, 1)),
            Motion::LineStart => block().map_or(0, |b| b.line_range(i).0),
            Motion::LineEnd => block().map_or(self.text().len(), |b| b.line_range(i).1),
            Motion::Start => 0,
            Motion::End => self.text().len(),
        }
    }

    /// Moves the caret; with `extend` the selection grows (Shift). Without it, Left/Right
    /// first collapse a selection to its start/end.
    pub fn move_caret(&mut self, motion: Motion, extend: bool) {
        let (start, end) = self.selection();
        let to = match motion {
            Motion::Left if !extend && start != end => start,
            Motion::Right if !extend && start != end => end,
            _ => self.target(motion),
        };
        self.caret = floor_boundary(self.text(), to);
        if !extend {
            self.anchor = self.caret;
        }
    }

    /// Backspace (with `word`: Ctrl+Backspace): deletes the selection or what is before the
    /// caret.
    pub fn backspace(&mut self, word: bool) {
        if self.caret == self.anchor {
            self.anchor = if word {
                self.word_left(self.caret)
            } else {
                self.prev(self.caret)
            };
        }
        self.insert("");
    }

    /// Delete (with `word`: Ctrl+Delete): deletes the selection or what follows the caret.
    pub fn delete(&mut self, word: bool) {
        if self.caret == self.anchor {
            self.anchor = if word {
                self.word_right(self.caret)
            } else {
                self.next(self.caret)
            };
        }
        self.insert("");
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text().len();
    }

    /// The word at `index` selected (double-click).
    pub fn select_word(&mut self, index: usize) {
        let text = self.text();
        let index = floor_boundary(text, index);
        let mut start = index;
        while start > 0 && text[..start].chars().next_back().is_some_and(is_word) {
            start = self.prev(start);
        }
        let mut end = index;
        while end < text.len() && text[end..].chars().next().is_some_and(is_word) {
            end = self.next(end);
        }
        if start == end {
            // Not on a word: the character there.
            end = self.next(index);
        }
        self.anchor = start;
        self.caret = end;
    }

    /// A click at `at` (document space): the caret goes there; with `extend` (Shift) the
    /// selection grows to it.
    pub fn click(&mut self, at: Point, extend: bool) {
        if let Some(block) = self.block() {
            self.caret = block.index_at(at);
            if !extend {
                self.anchor = self.caret;
            }
        }
    }

    /// The pointer dragged to `at` after a click: selects from the click to there.
    pub fn drag_to(&mut self, at: Point) {
        if let Some(block) = self.block() {
            self.caret = block.index_at(at);
        }
    }

    /// Toggles a style switch of the edited object (Ctrl+B "bold", Ctrl+I "italic", Ctrl+U
    /// "underline"); returns whether the object has it.
    pub fn toggle(&mut self, key: &str) -> bool {
        let current = self
            .object
            .properties()
            .into_iter()
            .find(|p| p.key == key)
            .map(|p| p.value);
        match current {
            Some(PropertyValue::Bool(on)) => self
                .object
                .set_property(key, &PropertyValue::Bool(!on))
                .is_ok(),
            _ => false,
        }
    }

    /// Ends editing: the change as one undo step, or `None` if nothing changes (a new text
    /// object left empty is not added).
    pub fn finish(self, doc: &Document) -> Option<Edit> {
        let empty = self.text().is_empty();
        let Some(id) = self.id else {
            if empty && !self.keep_empty {
                return None;
            }
            return ops::add(doc, self.object);
        };
        doc.scene().get(id)?;
        let subject = Subject::One(self.object.type_name());
        let removable = empty && self.object.as_any().is::<crate::tools::text::Text>();
        if removable {
            return Some(Edit {
                action: Action::Delete,
                subject,
                command: Command::Remove { ids: vec![id] },
                select: Some(Vec::new()),
            });
        }
        let unchanged = self
            .original
            .as_ref()
            .is_some_and(|o| o.to_json().ok() == self.object.to_json().ok());
        if unchanged {
            return None;
        }
        Some(Edit {
            action: Action::Reshape,
            subject,
            command: Command::Replace {
                objects: vec![(id, self.object)],
            },
            select: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;
    use crate::document::command::History;
    use crate::document::geometry::Rect;
    use crate::tools::callout::Callout;
    use crate::tools::text::Text;

    fn doc() -> Document {
        Document::new(Bitmap::filled(800, 400, Rgba::WHITE).unwrap())
    }

    fn new_text() -> TextEdit {
        TextEdit::new_object(Box::new(Text::new(Point::new(100.0, 100.0))), false).unwrap()
    }

    #[test]
    fn typing_editing_keys_and_selection() {
        let mut e = new_text();
        e.insert("Runs nightly");
        e.insert("\n");
        e.insert("on mains");
        assert_eq!(e.text(), "Runs nightly\non mains");
        e.move_caret(Motion::WordLeft, false);
        assert_eq!(e.caret(), "Runs nightly\non ".len());
        e.move_caret(Motion::LineEnd, true);
        assert_eq!(e.selected_text(), "mains");
        e.insert("battery");
        assert_eq!(e.text(), "Runs nightly\non battery");
        e.backspace(true);
        assert_eq!(e.text(), "Runs nightly\non ");
        e.move_caret(Motion::Start, false);
        e.delete(false);
        assert_eq!(e.text(), "uns nightly\non ");
        e.move_caret(Motion::Down, false);
        assert!(e.caret() >= "uns nightly\n".len());
        e.select_all();
        e.insert("Ä–ü");
        assert_eq!(e.text(), "Ä–ü");
        e.move_caret(Motion::Left, false);
        e.backspace(false);
        assert_eq!(e.text(), "Äü", "multi-byte characters are kept whole");
        e.select_word(0);
        assert_eq!(e.selected_text(), "Äü");
    }

    #[test]
    fn a_new_text_is_one_undo_step_and_an_empty_one_nothing() {
        let mut d = doc();
        let mut history = History::new();
        assert!(new_text().finish(&d).is_none(), "empty: nothing added");
        let mut e = new_text();
        e.insert("Hello");
        assert!(e.toggle("italic"));
        let edit = e.finish(&d).unwrap();
        assert_eq!(
            (edit.action, edit.subject),
            (Action::Add, Subject::One("text"))
        );
        history.execute(&mut d, "Add", edit.command).unwrap();
        assert_eq!(d.scene().len(), 1);
        let id = d.scene().ids()[0];
        // Spec "Edit in place": editing it again changes the same object, one step.
        let mut e = TextEdit::existing(&d, id).unwrap();
        assert!(e.finish_unchanged(&d));
        e = TextEdit::existing(&d, id).unwrap();
        e.insert(", world");
        let edit = e.finish(&d).unwrap();
        assert_eq!(edit.action, Action::Reshape);
        history.execute(&mut d, "Edit", edit.command).unwrap();
        let t = d
            .scene()
            .get(id)
            .unwrap()
            .object
            .as_any()
            .downcast_ref::<Text>()
            .unwrap()
            .clone();
        assert_eq!((t.text.as_str(), t.italic), ("Hello, world", true));
        // Emptied: the text object goes.
        let mut e = TextEdit::existing(&d, id).unwrap();
        e.select_all();
        e.backspace(false);
        let edit = e.finish(&d).unwrap();
        assert_eq!(edit.action, Action::Delete);
        history.execute(&mut d, "Delete", edit.command).unwrap();
        assert!(d.scene().is_empty());
        history.undo(&mut d).unwrap();
        assert_eq!(d.scene().len(), 1);
    }

    #[test]
    fn a_callout_is_added_even_without_text() {
        let d = doc();
        let callout = Callout::new(Rect::new(10.0, 10.0, 200.0, 80.0), Point::new(50.0, 200.0));
        let e = TextEdit::new_object(Box::new(callout), true).unwrap();
        let edit = e.finish(&d).unwrap();
        assert_eq!(edit.subject, Subject::One("callout"));
    }

    #[test]
    fn clicks_place_the_caret() {
        let mut e = new_text();
        e.insert("one\ntwo");
        let block = e.block().unwrap();
        let (top, _) = block.caret(5);
        e.click(Point::new(top.x + 0.5, top.y + 2.0), false);
        assert_eq!(e.caret(), 5);
        e.drag_to(Point::new(top.x + 500.0, top.y + 2.0));
        assert_eq!(e.selected_text(), "wo");
    }

    impl TextEdit {
        fn finish_unchanged(self, doc: &Document) -> bool {
            self.finish(doc).is_none()
        }
    }
}
