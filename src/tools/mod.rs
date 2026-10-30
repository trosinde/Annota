//! Annotation object types, one module each (CLAUDE.md §3). Each implements
//! [`crate::document::annotation::Annotation`] and is registered in
//! [`crate::document::registry`].
//!
//! `add-core-annotation-tools` adds arrow, line, pen, shape, text, callout, step, highlighter
//! and redaction here. [`image`] has no tool button of its own: image objects come from the
//! recent-captures tray and from pasting. [`group`] has no tool either: groups come from
//! Group (Ctrl+G).

pub mod group;
pub mod image;
