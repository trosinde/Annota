//! Annotation object types, one module each (CLAUDE.md §3). Each implements
//! [`crate::document::annotation::Annotation`] and is registered in
//! [`crate::document::registry`].
//!
//! add-core-annotation-tools adds [`arrow`] (arrows and lines), [`pen`], [`shape`], [`text`],
//! [`callout`], [`step`], [`highlighter`] and [`redaction`]; the tools that draw them are in
//! [`crate::editing::tool`]. [`image`] has no tool button of its own: image objects come from
//! the recent-captures tray and from pasting. [`group`] has no tool either: groups come from
//! Group (Ctrl+G).

pub mod arrow;
pub mod callout;
pub mod common;
pub mod group;
pub mod highlighter;
pub mod image;
pub mod pen;
pub mod redaction;
pub mod shape;
pub mod step;
pub mod text;
