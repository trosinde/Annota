//! Conversion between [`Frame`] and `gdk::Texture` (needs GTK, so kept out of `frame.rs`).

use gtk::{gdk, glib, prelude::*};

use super::frame::Frame;

/// `gdk::MemoryFormat` matching cairo's native-endian `ARGB32`.
fn argb32_format() -> gdk::MemoryFormat {
    if cfg!(target_endian = "little") {
        gdk::MemoryFormat::B8g8r8a8Premultiplied
    } else {
        gdk::MemoryFormat::A8r8g8b8Premultiplied
    }
}

pub fn frame_from_texture(texture: &impl IsA<gdk::Texture>) -> Option<Frame> {
    let width = texture.width();
    let height = texture.height();
    if width <= 0 || height <= 0 {
        return None;
    }
    let stride = width as usize * 4;
    let mut data = vec![0u8; stride * height as usize];
    texture.download(&mut data, stride);
    Frame::from_argb32(width, height, stride, data)
}

pub fn texture_from_frame(frame: &Frame) -> gdk::Texture {
    let bytes = glib::Bytes::from(frame.data());
    gdk::MemoryTexture::new(
        frame.width(),
        frame.height(),
        argb32_format(),
        &bytes,
        frame.stride(),
    )
    .upcast()
}
