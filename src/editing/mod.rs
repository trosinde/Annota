//! Editing on the canvas with the Select tool and the drawing tools (`tool`, `steps`,
//! `text_edit`; add-core-annotation-tools) (add-annotation-editor tasks 3.2–3.4): the
//! selection, pointer gestures (move, resize, rotate, rubber band), snapping, and the one-shot
//! operations (nudge, delete, duplicate, stacking order, group/ungroup), and the operations on
//! the whole document (task 4.3): crop mode, canvas edges and the image's corner radius on the
//! canvas, crop/resize/rotate/flip/flatten (`image_ops`).
//!
//! Everything here is free of widgets and tested without a display; `src/ui/canvas.rs` only
//! feeds it pointer and key events and draws the result (`render::overlay`). Changes to the
//! document leave this module as [`ops::Edit`]s — one `Command` each, executed through the
//! `History` — so every move, resize, rotation, reorder and (un)grouping is one undo step.
//! The selection itself is editor state, not part of the document, and is not undoable.

pub mod canvas_edit;
pub mod crop;
pub mod drag;
pub mod image_ops;
pub mod ops;
pub mod selection;
pub mod snap;
pub mod steps;
pub mod text_edit;
pub mod tool;
