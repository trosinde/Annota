//! The window's content rectangle per buffer of a window stream (task 3.3, design.md D2).
//!
//! Each buffer carries a crop meta in monitor-scale px; [`geometry::window_content`] converts it
//! to buffer pixels and trims the client-side shadow by alpha, on every buffer. Measured live
//! (GNOME 50, 2026-11-24): while GNOME animates a maximize or unmaximize, the first buffers
//! with the new crop meta show the window mid-animation (shrunk into a corner, or the old size
//! stretched over the new crop), and the settled window only comes with the window's next
//! redraw, under the same crop meta. A rectangle cached per crop meta kept the animation's
//! rectangle for as long as the window stayed maximized.

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

/// How far (buffer px, per edge) a new rectangle may lie from the current one and still count
/// as the same: the anti-aliased 1 px outline's alpha hovers around the threshold from frame
/// to frame (measured: 1284 then 1283 px wide), which must not move the crop.
pub const SAME_RECT: i32 = 2;

pub struct WindowContent {
    stream_size: Size,
    monitor_scale: Option<f64>,
    current: Mutex<Option<IRect>>,
}

impl WindowContent {
    /// `stream_size`: the stream's `size`; `monitor_scale`: the scale of the monitor it is on
    /// (`source::monitor_scale`), `None` for the alpha rule alone.
    pub fn new(stream_size: Size, monitor_scale: Option<f64>) -> Self {
        Self {
            stream_size,
            monitor_scale,
            current: Mutex::new(None),
        }
    }

    /// The recording's [`VideoArea`] for this window stream.
    pub fn into_area(self) -> VideoArea {
        let content = Arc::new(self);
        let rect: ContentRect = Arc::new(move |buffer, info| content.rect(buffer, info));
        VideoArea::Window(rect)
    }

    /// The content rectangle of `buffer` in buffer pixels; `None` keeps the previous one (a
    /// buffer without a visible pixel). A rectangle within [`SAME_RECT`] of the current one
    /// keeps the current one.
    pub fn rect(&self, buffer: &gst::BufferRef, info: &gst_video::VideoInfo) -> Option<IRect> {
        let fresh = self.compute(buffer, info)?;
        let mut current = self.current.lock().unwrap_or_else(|e| e.into_inner());
        let rect = match *current {
            Some(known) if same(known, fresh) => known,
            _ => fresh,
        };
        *current = Some(rect);
        Some(rect)
    }

    fn compute(&self, buffer: &gst::BufferRef, info: &gst_video::VideoInfo) -> Option<IRect> {
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
        if info.format() != ALPHA_FORMAT {
            // No alpha to trim the shadow by: the crop meta alone, else the whole buffer.
            let converted = crop_meta.and_then(|crop| {
                geometry::window_crop_in_buffer(crop, self.stream_size, size, self.monitor_scale)
            });
            return Some(converted.unwrap_or_else(|| IRect::new(0, 0, size.w, size.h)));
        }
        let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, info).ok()?;
        let stride = usize::try_from(*frame.plane_stride().first()?).ok()?;
        let pixels = Pixels {
            width: frame.width(),
            height: frame.height(),
            stride,
            data: frame.plane_data(0).ok()?,
        };
        geometry::window_content(&pixels, crop_meta, self.stream_size, self.monitor_scale)
    }
}

/// Every edge of `b` within [`SAME_RECT`] of `a`'s.
fn same(a: IRect, b: IRect) -> bool {
    (a.x - b.x).abs() <= SAME_RECT
        && (a.y - b.y).abs() <= SAME_RECT
        && (a.right() - b.right()).abs() <= SAME_RECT
        && (a.bottom() - b.bottom()).abs() <= SAME_RECT
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// One painted layer of a test buffer: a rectangle, its B, G, R and its alpha.
    pub(crate) type Layer = (IRect, [u8; 3], u8);

    /// A BGRA (little-endian byte order) frame: transparent black, `layers` painted over it in
    /// order. Built row by row from copied spans, fast enough for 6144×3456 in a debug build.
    pub(crate) fn layered(w: i32, h: i32, layers: &[Layer]) -> Vec<u8> {
        let spans: Vec<(IRect, Vec<u8>)> = layers
            .iter()
            .filter_map(|(rect, bgr, alpha)| {
                let rect = rect.intersect(IRect::new(0, 0, w, h))?;
                let px = [bgr[0], bgr[1], bgr[2], *alpha];
                Some((rect, px.repeat(rect.w as usize)))
            })
            .collect();
        let row_bytes = (w * 4) as usize;
        let mut data = vec![0u8; row_bytes * h as usize];
        for (y, row) in data.chunks_exact_mut(row_bytes).enumerate() {
            let y = y as i32;
            for (rect, span) in &spans {
                if y >= rect.y && y < rect.bottom() {
                    let at = (rect.x * 4) as usize;
                    row[at..at + span.len()].copy_from_slice(span);
                }
            }
        }
        data
    }

    fn buffer(data: Vec<u8>, crop_meta: IRect) -> gst::Buffer {
        let mut buffer = gst::Buffer::from_slice(data);
        gst_video::VideoCropMeta::add(
            buffer.get_mut().unwrap(),
            (
                crop_meta.x as u32,
                crop_meta.y as u32,
                crop_meta.w as u32,
                crop_meta.h as u32,
            ),
        );
        buffer
    }

    const BODY: [u8; 3] = [40, 80, 120];
    const STALE: [u8; 3] = [200, 0, 200];

    /// GNOME 50's window at rest, as measured live (P6 and 2026-11-24, 640×400 logical at
    /// scale 1.25): crop meta (0, 0, 928, 628) = (0, 0, 1484, 1004) in buffer px, a CSD shadow of
    /// alpha 30 filling it, the window opaque at (100, 100) with a right outline column whose
    /// alpha passes the threshold only sometimes; opaque undefined pixels outside the crop.
    pub(crate) fn at_rest(outline: u8) -> Vec<u8> {
        layered(
            6144,
            3456,
            &[
                (IRect::new(2000, 0, 4144, 3456), STALE, 255),
                (IRect::new(0, 0, 1484, 1004), [0, 0, 0], 30),
                (IRect::new(100, 100, 1283, 804), BODY, 255),
                (IRect::new(1383, 100, 1, 804), BODY, outline),
            ],
        )
    }

    /// The window maximized and settled: crop meta (0, 0, 3840, 2100) = (0, 0, 6144, 3360) in
    /// buffer px, opaque edge to edge (no shadow); undefined pixels below it.
    pub(crate) fn maximized() -> Vec<u8> {
        layered(
            6144,
            3456,
            &[
                (IRect::new(0, 3360, 6144, 96), STALE, 255),
                (IRect::new(0, 0, 6144, 3360), BODY, 255),
            ],
        )
    }

    /// The first buffers after a maximize, measured live: GNOME's animation frame, the window
    /// squeezed into 1280×800 near the bottom-right corner of the new crop, the rest
    /// transparent; mutter sends the settled window only with the window's next redraw.
    pub(crate) fn mid_maximize() -> Vec<u8> {
        layered(
            6144,
            3456,
            &[(IRect::new(4762, 2458, 1280, 800), BODY, 255)],
        )
    }

    /// The first buffers after an unmaximize, measured live: the old (maximized) content still
    /// fills the whole new crop, shadow margin included.
    pub(crate) fn mid_unmaximize() -> Vec<u8> {
        layered(6144, 3456, &[(IRect::new(0, 0, 1484, 1004), STALE, 255)])
    }

    fn info() -> gst_video::VideoInfo {
        gst_video::VideoInfo::builder(ALPHA_FORMAT, 6144, 3456)
            .build()
            .unwrap()
    }

    pub(crate) const REST_META: IRect = IRect::new(0, 0, 928, 628);
    pub(crate) const MAX_META: IRect = IRect::new(0, 0, 3840, 2100);

    /// The live sequence of 2026-11-24 (scale 1.25, P6's buffers): at rest, the maximize
    /// animation's frame and then the settled maximized window under the same crop meta, an
    /// in-between crop meta, the unmaximize animation's frame and then the window at rest again.
    /// Each buffer gets its own rectangle: a settled buffer is never judged by the animation
    /// frame that came first under its crop meta.
    // spec: screen-recording / Recording source / Recorded window resized
    #[test]
    fn every_buffer_gets_its_own_rectangle_through_a_maximize_and_back() {
        gst::init().unwrap();
        let content = WindowContent::new(Size::new(3072, 1728), Some(1.25));
        let info = info();
        let rect = |data: Vec<u8>, meta: IRect| content.rect(&buffer(data, meta), &info);
        let window = IRect::new(100, 100, 1284, 804);
        assert_eq!(rect(at_rest(255), REST_META), Some(window));
        // The outline drops below the threshold: within SAME_RECT, the rectangle stays.
        assert_eq!(rect(at_rest(100), REST_META), Some(window));
        // Maximize: the animation's frame first, then the settled window, same crop meta.
        assert_eq!(
            rect(mid_maximize(), MAX_META),
            Some(IRect::new(4762, 2458, 1280, 800))
        );
        let whole = IRect::new(0, 0, 6144, 3360);
        assert_eq!(
            rect(maximized(), MAX_META),
            Some(whole),
            "the settled window"
        );
        assert_eq!(rect(maximized(), MAX_META), Some(whole));
        // Unmaximize: an in-between crop meta, the animation's frame, then the window at rest
        // (its outline column below the threshold this time: 1283 px, not 1284).
        assert_eq!(
            rect(mid_unmaximize(), IRect::new(0, 0, 830, 530)),
            Some(IRect::new(0, 0, 1328, 848))
        );
        assert_eq!(
            rect(mid_unmaximize(), REST_META),
            Some(IRect::new(0, 0, 1484, 1004))
        );
        let back = IRect::new(100, 100, 1283, 804);
        assert_eq!(
            rect(at_rest(100), REST_META),
            Some(back),
            "the window whole again"
        );
        assert_eq!(
            rect(at_rest(255), REST_META),
            Some(back),
            "no jitter by a pixel"
        );
        // A buffer without a visible pixel keeps the rectangle.
        assert_eq!(rect(layered(6144, 3456, &[]), REST_META), None);
        assert_eq!(rect(at_rest(255), REST_META), Some(back));
    }
}
