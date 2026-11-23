//! The window's content rectangle per buffer of a window stream (task 3.3, design.md D2).
//!
//! Each buffer carries a crop meta in monitor-scale px; [`geometry::window_content`] converts it
//! to buffer pixels and trims the client-side shadow by alpha. The alpha scan of a 4K buffer is
//! too costly for every frame, so it runs on the first buffer and again only when the crop meta
//! (or the buffer size) changes; buffers in between reuse the last rectangle.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;

use super::geometry::{self, Size};
use super::pipeline::{ContentRect, VideoArea};
use crate::capture::geometry::IRect;
use crate::capture::screencast::Pixels;

/// The pixel format whose byte order the alpha rule reads (`screencast::window_rect`).
#[cfg(target_endian = "little")]
const ALPHA_FORMAT: gst_video::VideoFormat = gst_video::VideoFormat::Bgra;
#[cfg(target_endian = "big")]
const ALPHA_FORMAT: gst_video::VideoFormat = gst_video::VideoFormat::Argb;

/// The last computed rectangle and what it was computed from.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    crop_meta: Option<IRect>,
    buffer: Size,
}

pub struct WindowContent {
    stream_size: Size,
    monitor_scale: Option<f64>,
    last: Mutex<Option<(Key, Option<IRect>)>>,
    scans: AtomicUsize,
}

impl WindowContent {
    /// `stream_size`: the stream's `size`; `monitor_scale`: the scale of the monitor it is on
    /// (`source::monitor_scale`), `None` for the alpha rule alone.
    pub fn new(stream_size: Size, monitor_scale: Option<f64>) -> Self {
        Self {
            stream_size,
            monitor_scale,
            last: Mutex::new(None),
            scans: AtomicUsize::new(0),
        }
    }

    /// The recording's [`VideoArea`] for this window stream.
    pub fn into_area(self) -> VideoArea {
        let content = Arc::new(self);
        let rect: ContentRect = Arc::new(move |buffer, info| content.rect(buffer, info));
        VideoArea::Window(rect)
    }

    /// How many buffers were scanned for alpha so far.
    pub fn scans(&self) -> usize {
        self.scans.load(Ordering::Relaxed)
    }

    /// The content rectangle of `buffer` in buffer pixels; `None` keeps the previous one.
    pub fn rect(&self, buffer: &gst::BufferRef, info: &gst_video::VideoInfo) -> Option<IRect> {
        let size = Size::new(
            i32::try_from(info.width()).ok()?,
            i32::try_from(info.height()).ok()?,
        );
        let crop_meta = buffer.meta::<gst_video::VideoCropMeta>().and_then(|meta| {
            let (x, y, w, h) = meta.rect();
            Some(IRect::new(
                i32::try_from(x).ok()?,
                i32::try_from(y).ok()?,
                i32::try_from(w).ok()?,
                i32::try_from(h).ok()?,
            ))
        });
        let key = Key {
            crop_meta,
            buffer: size,
        };
        let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((known, rect)) = *last {
            if known == key {
                return rect;
            }
        }
        let rect = self.compute(buffer, info, key);
        // A buffer without a visible pixel says nothing new: keep the previous rectangle.
        if rect.is_some() || last.is_none() {
            *last = Some((key, rect));
        }
        rect
    }

    fn compute(
        &self,
        buffer: &gst::BufferRef,
        info: &gst_video::VideoInfo,
        key: Key,
    ) -> Option<IRect> {
        let converted = key.crop_meta.and_then(|crop| {
            geometry::window_crop_in_buffer(crop, self.stream_size, key.buffer, self.monitor_scale)
        });
        if info.format() != ALPHA_FORMAT {
            // No alpha to trim the shadow by: the crop meta alone, else the whole buffer.
            return Some(converted.unwrap_or_else(|| IRect::new(0, 0, key.buffer.w, key.buffer.h)));
        }
        self.scans.fetch_add(1, Ordering::Relaxed);
        let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, info).ok()?;
        let stride = usize::try_from(*frame.plane_stride().first()?).ok()?;
        let pixels = Pixels {
            width: frame.width(),
            height: frame.height(),
            stride,
            data: frame.plane_data(0).ok()?,
        };
        geometry::window_content(&pixels, key.crop_meta, self.stream_size, self.monitor_scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A BGRA (little-endian byte order) buffer: transparent, a faint shadow of alpha 30 around
    /// `window`, the window itself opaque.
    fn buffer(w: i32, h: i32, window: IRect, crop_meta: Option<IRect>) -> gst::Buffer {
        let shadow = IRect::new(window.x - 8, window.y - 8, window.w + 16, window.h + 16);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let alpha = if window.contains(x, y) {
                    255
                } else if shadow.contains(x, y) {
                    30
                } else {
                    0
                };
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[40, 80, 120, alpha]);
            }
        }
        let mut buffer = gst::Buffer::from_slice(data);
        if let Some(crop) = crop_meta {
            let b = buffer.get_mut().unwrap();
            gst_video::VideoCropMeta::add(
                b,
                (crop.x as u32, crop.y as u32, crop.w as u32, crop.h as u32),
            );
        }
        buffer
    }

    fn info(w: u32, h: u32) -> gst_video::VideoInfo {
        gst_video::VideoInfo::builder(ALPHA_FORMAT, w, h)
            .build()
            .unwrap()
    }

    /// P6 scaled down by 8: buffers 768×432 for a stream of size 384×216 at monitor scale 1.25
    /// (factor 1.6). The crop meta (0, 0, 100, 62) holds the window and its shadow.
    // spec: screen-recording / Recording source / Recorded window resized
    #[test]
    fn the_shadow_is_trimmed_and_scanned_only_when_the_crop_meta_changes() {
        gst::init().unwrap();
        let content = WindowContent::new(Size::new(384, 216), Some(1.25));
        let info = info(768, 432);
        let normal_meta = IRect::new(0, 0, 100, 62);
        // In buffer px the meta is (0, 0, 160, 99); the window sits inside its shadow margin.
        let normal = IRect::new(10, 10, 140, 79);
        let first = buffer(768, 432, normal, Some(normal_meta));
        assert_eq!(content.rect(&first, &info), Some(normal));
        assert_eq!(content.scans(), 1);
        // The same crop meta: no scan, the same rectangle.
        for _ in 0..5 {
            let again = buffer(768, 432, normal, Some(normal_meta));
            assert_eq!(content.rect(&again, &info), Some(normal));
        }
        assert_eq!(
            content.scans(),
            1,
            "no alpha scan while the crop meta stays"
        );
        // Maximised: a new crop meta, a new scan, the larger rectangle.
        let max_meta = IRect::new(0, 0, 480, 262);
        let max = IRect::new(0, 0, 768, 419);
        let larger = buffer(768, 432, max, Some(max_meta));
        assert_eq!(content.rect(&larger, &info), Some(max));
        assert_eq!(content.scans(), 2);
        // And back.
        let back = buffer(768, 432, normal, Some(normal_meta));
        assert_eq!(content.rect(&back, &info), Some(normal));
        assert_eq!(content.scans(), 3);
        // Pixels outside the converted crop meta never count, even if opaque.
        let stray = buffer(768, 432, IRect::new(300, 300, 50, 50), Some(normal_meta));
        let mut data = stray.map_readable().unwrap().to_vec();
        for y in 20..60 {
            for x in 20..100 {
                data[((y * 768 + x) * 4 + 3) as usize] = 255;
            }
        }
        let mut stray = gst::Buffer::from_slice(data);
        gst_video::VideoCropMeta::add(stray.get_mut().unwrap(), (0, 0, 101, 62));
        assert_eq!(
            content.rect(&stray, &info),
            Some(IRect::new(20, 20, 80, 40))
        );
    }
}
