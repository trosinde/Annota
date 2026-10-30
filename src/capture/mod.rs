//! Capability `capture`: region, window and screen captures through xdg-desktop-portal,
//! delayed captures, global shortcuts and new captures from the clipboard or a file.
//!
//! This first step holds the display-independent core: the in-memory image, selection
//! geometry, window-edge snapping and the Screenshot portal wrapper.

pub mod frame;
pub mod geometry;
pub mod portal;
pub mod snap;
pub mod texture;
