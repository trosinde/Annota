//! Editing on the canvas with the Select tool (add-annotation-editor tasks 3.2–3.4): the
//! selection, pointer gestures (move, resize, rotate, rubber band), snapping, and the one-shot
//! operations (nudge, delete, duplicate, stacking order, group/ungroup).
//!
//! Everything here is free of widgets and tested without a display; `src/ui/canvas.rs` only
//! feeds it pointer and key events and draws the result (`render::overlay`). Changes to the
//! document leave this module as [`ops::Edit`]s — one `Command` each, executed through the
//! `History` — so every move, resize, rotation, reorder and (un)grouping is one undo step.
//! The selection itself is editor state, not part of the document, and is not undoable.

pub mod drag;
pub mod ops;
pub mod selection;
pub mod snap;
