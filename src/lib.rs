//! Library half of Annota: the document model, the `.annota` format and the cairo renderer.
//!
//! Everything here is free of GTK widgets so it can be unit- and golden-image-tested without a
//! display (`tests/render/`). The application binary (`src/main.rs`) builds the UI on top.

pub mod document;
pub mod render;
pub mod tools;
