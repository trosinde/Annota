//! Capability `style-system`: quick styles, favourites and themes (add-style-system).
//!
//! * [`value`] — what a quick style is: a tool's style properties, the same
//!   `properties`/`set_property` values the sidebar edits, and their JSON form;
//! * [`library`] — the user's styles, favourites and current theme (`styles.json`);
//! * [`theme`] — themes and the `.annota-theme` format;
//! * [`generate`] — the eight theme styles of every tool;
//! * [`store`] — where all of it lives on disk, the system theme directory, import/export;
//! * [`tile`] — the picture of a style in the sidebar's grid.
//!
//! Applying a style to objects is an ordinary property edit (`editing::ops::set_properties`):
//! one `Command`, one undo step. Nothing here touches a document.

pub mod generate;
pub mod library;
pub mod store;
pub mod theme;
pub mod tile;
pub mod value;
