//! The window's content rectangle per buffer of a window stream (task 3.3, design.md D2).
//!
//! The rectangle is `capture::screencast::window_rect`'s: the window found by its pixels at
//! whatever scale the crop meta is in (PR #12), without its shadow. Measured live (GNOME 50,
//! 2026-11-24): while GNOME animates a maximize or unmaximize, the first buffers with the new
//! crop meta show the window mid-animation (shrunk into a corner, or the old size stretched
//! over the new crop), and the settled window only comes with the window's next redraw, under
//! the same crop meta. So every buffer gets its own rectangle; `window_rect` itself (a scan of
//! the crop meta's area, up to the whole buffer) runs only when the crop meta or the buffer
//! size changed or the previous rectangle no longer bounds the window, which a check of its
//! edge lines tells cheaply ([`bounds_window`], design.md "Decided during apply").

use std::sync::{Arc, Mutex};

use gstreamer as gst;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;

use super::geometry::Scales;
use super::pipeline::{ContentRect, VideoArea};
use crate::capture::geometry::IRect;
use crate::capture::screencast::{window_rect, Pixels, OPAQUE_ALPHA};

/// The pixel format whose byte order the alpha rule reads (`screencast::window_rect`).
#[cfg(target_endian = "little")]
const ALPHA_FORMAT: gst_video::VideoFormat = gst_video::VideoFormat::Bgra;
#[cfg(target_endian = "big")]
const ALPHA_FORMAT: gst_video::VideoFormat = gst_video::VideoFormat::Argb;

/// The byte of a pixel that holds its alpha in [`ALPHA_FORMAT`].
#[cfg(target_endian = "little")]
const ALPHA_BYTE: usize = 3;
#[cfg(target_endian = "big")]
const ALPHA_BYTE: usize = 0;

/// How far (buffer px, per edge) a new rectangle may lie from the current one and still count
/// as the same: the anti-aliased 1 px outline's alpha hovers around the threshold from frame
/// to frame (measured: 1284 then 1283 px wide), which must not move the crop.
pub const SAME_RECT: i32 = 2;

/// A buffer's crop meta (x, y, w, h) as `pipewiresrc` hands it over.
type CropMeta = (u32, u32, u32, u32);

#[derive(Default)]
struct State {
    /// The crop meta and buffer size `found` was searched under.
    key: Option<(Option<CropMeta>, u32, u32)>,
    /// `window_rect`'s last answer, while it is one [`bounds_window`] can check.
    found: Option<IRect>,
    /// The rectangle handed out, within [`SAME_RECT`] of the found ones.
    current: Option<IRect>,
}

#[derive(Default)]
pub struct WindowContent {
    state: Mutex<State>,
}

impl WindowContent {
    pub fn new() -> Self {
        Self::default()
    }

    /// The recording's [`VideoArea`] for this window stream; `scales` set its frame size.
    pub fn into_area(self, scales: Scales) -> VideoArea {
        let content = Arc::new(self);
        let rect: ContentRect = Arc::new(move |buffer, info| content.rect(buffer, info));
        VideoArea::Window {
            content: rect,
            scales,
        }
    }

    /// The content rectangle of `buffer` in buffer pixels; `None` keeps the previous one (a
    /// buffer without a visible pixel). A rectangle within [`SAME_RECT`] of the current one
    /// keeps the current one.
    pub fn rect(&self, buffer: &gst::BufferRef, info: &gst_video::VideoInfo) -> Option<IRect> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = find(&mut state, buffer, info)?;
        let rect = match state.current {
            Some(known) if same(known, fresh) => known,
            _ => fresh,
        };
        state.current = Some(rect);
        Some(rect)
    }
}

/// `window_rect` for `buffer`, or the previous answer where the crop meta and the buffer size
/// are the same and [`bounds_window`] says it still holds.
fn find(state: &mut State, buffer: &gst::BufferRef, info: &gst_video::VideoInfo) -> Option<IRect> {
    let crop = buffer
        .meta::<gst_video::VideoCropMeta>()
        .map(|meta| meta.rect());
    if info.format() != ALPHA_FORMAT {
        // No alpha to find the window by (never seen from GNOME): the crop meta as it is, else
        // the whole buffer.
        let whole = IRect::new(
            0,
            0,
            i32::try_from(info.width()).ok()?,
            i32::try_from(info.height()).ok()?,
        );
        let meta = crop.and_then(|(x, y, w, h)| {
            IRect::new(
                i32::try_from(x).ok()?,
                i32::try_from(y).ok()?,
                i32::try_from(w).ok()?,
                i32::try_from(h).ok()?,
            )
            .intersect(whole)
        });
        return Some(meta.unwrap_or(whole));
    }
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, info).ok()?;
    let stride = usize::try_from(*frame.plane_stride().first()?).ok()?;
    let pixels = Pixels {
        width: frame.width(),
        height: frame.height(),
        stride,
        data: frame.plane_data(0).ok()?,
    };
    find_in(state, &pixels, crop)
}

/// [`find`] on plain pixels.
fn find_in(state: &mut State, pixels: &Pixels, crop: Option<CropMeta>) -> Option<IRect> {
    let key = Some((crop, pixels.width, pixels.height));
    if state.key == key {
        if let Some(rect) = state.found.filter(|rect| bounds_window(pixels, *rect)) {
            return Some(rect);
        }
    }
    let rect = window_rect(pixels, crop);
    state.key = key;
    // A translucent window (found by alpha > 0) or one bounded by stray pixels inside the meta
    // is not checkable by the edge lines: searched again on the next buffer.
    state.found = rect.filter(|rect| bounds_window(pixels, *rect));
    rect
}

/// Whether `rect` is still the window's box in `pixels`, by its edge lines only: each of its
/// four edge lines holds an opaque pixel (alpha ≥ [`OPAQUE_ALPHA`]) and none of the four lines
/// just outside it does (over `rect`'s span, as `window_rect` grows a box). A window that moved,
/// grew or shrank fails it; a few thousand reads instead of a scan of the window's area.
fn bounds_window(pixels: &Pixels, rect: IRect) -> bool {
    let (Ok(width), Ok(height)) = (i32::try_from(pixels.width), i32::try_from(pixels.height))
    else {
        return false;
    };
    if rect.is_empty() || rect.intersect(IRect::new(0, 0, width, height)) != Some(rect) {
        return false;
    }
    let opaque = |x: i32, y: i32| {
        (0..width).contains(&x)
            && (0..height).contains(&y)
            && pixels
                .data
                .get(y as usize * pixels.stride + x as usize * 4 + ALPHA_BYTE)
                .is_some_and(|alpha| *alpha >= OPAQUE_ALPHA)
    };
    let column = |x: i32| (rect.y..rect.bottom()).any(|y| opaque(x, y));
    let row = |y: i32| (rect.x..rect.right()).any(|x| opaque(x, y));
    column(rect.x)
        && column(rect.right() - 1)
        && row(rect.y)
        && row(rect.bottom() - 1)
        && !column(rect.x - 1)
        && !column(rect.right())
        && !row(rect.y - 1)
        && !row(rect.bottom())
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
    /// buffer px, opaque edge to edge (no shadow); transparent below it, as the window stills
    /// measured it (6144 × 3360, PR #12 live run 2026-11-23).
    pub(crate) fn maximized() -> Vec<u8> {
        layered(6144, 3456, &[(IRect::new(0, 0, 6144, 3360), BODY, 255)])
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
        info_of(6144, 3456)
    }

    pub(crate) const REST_META: IRect = IRect::new(0, 0, 928, 628);
    pub(crate) const MAX_META: IRect = IRect::new(0, 0, 3840, 2100);

    fn info_of(w: u32, h: u32) -> gst_video::VideoInfo {
        gst_video::VideoInfo::builder(ALPHA_FORMAT, w, h)
            .build()
            .unwrap()
    }

    /// The scenario's buffer: a 1280×800 window (640×400 logical at 1.25) at the origin of a
    /// 6144×3456 buffer at scale 2, its crop meta (0, 0, 800, 500) at the monitor's scale; stale
    /// opaque pixels beyond a transparent gap.
    // spec: screen-recording / Recording source / Window crop meta in buffer pixels
    #[test]
    fn a_crop_meta_at_the_monitor_scale_gives_the_whole_window() {
        gst::init().unwrap();
        let data = layered(
            6144,
            3456,
            &[
                (IRect::new(2000, 0, 4144, 3456), STALE, 255),
                (IRect::new(0, 0, 1280, 800), BODY, 255),
            ],
        );
        let content = WindowContent::new();
        let rect = content.rect(&buffer(data, IRect::new(0, 0, 800, 500)), &info());
        assert_eq!(rect, Some(IRect::new(0, 0, 1280, 800)));
        // A crop meta in buffer pixels (the terminal of 2026-11-14) gives the same window.
        let data = layered(6144, 3456, &[(IRect::new(0, 0, 1280, 800), BODY, 255)]);
        let content = WindowContent::new();
        let rect = content.rect(&buffer(data, IRect::new(0, 0, 1280, 800)), &info());
        assert_eq!(rect, Some(IRect::new(0, 0, 1280, 800)));
    }

    /// The live sequence of 2026-11-24 (scale 1.25, P6's buffers): at rest, the maximize
    /// animation's frame and then the settled maximized window under the same crop meta, an
    /// in-between crop meta, the unmaximize animation's frame and then the window at rest again.
    /// Each buffer gets its own rectangle: a settled buffer is never judged by the animation
    /// frame that came first under its crop meta.
    // spec: screen-recording / Recording source / Recorded window resized
    #[test]
    fn every_buffer_gets_its_own_rectangle_through_a_maximize_and_back() {
        gst::init().unwrap();
        let content = WindowContent::new();
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
        let stretched = IRect::new(0, 0, 1484, 1004);
        assert_eq!(
            rect(mid_unmaximize(), IRect::new(0, 0, 830, 530)),
            Some(stretched)
        );
        assert_eq!(rect(mid_unmaximize(), REST_META), Some(stretched));
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

    /// The cheap path (previous rectangle checked by its edge lines) answers exactly what
    /// `window_rect` answers, buffer after buffer: a window that moves, grows or shrinks under
    /// the same crop meta, one cut by a monitor-scale meta, a translucent one, stray pixels and
    /// an empty buffer. The one difference, on purpose: a stray pixel that appears apart from
    /// the window inside an unchanged crop meta, which `window_rect`'s box of the meta would take
    /// in, is left out.
    // spec: screen-recording / Recording source / Recorded window resized
    #[test]
    fn the_checked_rectangle_is_always_what_window_rect_finds() {
        let (w, h) = (192, 108);
        let meta = Some((0u32, 0u32, 120u32, 80u32));
        let frames: Vec<(Vec<u8>, Option<CropMeta>)> = vec![
            (
                layered(w, h, &[(IRect::new(10, 10, 80, 50), BODY, 255)]),
                meta,
            ),
            (
                layered(w, h, &[(IRect::new(10, 10, 80, 50), BODY, 255)]),
                meta,
            ),
            // Moved, grown, shrunk under the same meta.
            (
                layered(w, h, &[(IRect::new(12, 10, 80, 50), BODY, 255)]),
                meta,
            ),
            (
                layered(w, h, &[(IRect::new(12, 10, 81, 52), BODY, 255)]),
                meta,
            ),
            (
                layered(w, h, &[(IRect::new(14, 11, 60, 40), BODY, 255)]),
                meta,
            ),
            // Past the meta (cut by a monitor-scale meta), then with a stray pixel beyond a gap.
            (
                layered(w, h, &[(IRect::new(0, 0, 150, 96), BODY, 255)]),
                meta,
            ),
            (
                layered(
                    w,
                    h,
                    &[
                        (IRect::new(0, 0, 150, 96), BODY, 255),
                        (IRect::new(180, 100, 1, 1), STALE, 255),
                    ],
                ),
                meta,
            ),
            // A stray pixel inside the meta, apart from the window, appears and goes.
            (
                layered(
                    w,
                    h,
                    &[
                        (IRect::new(10, 10, 80, 50), BODY, 255),
                        (IRect::new(110, 70, 1, 1), STALE, 255),
                    ],
                ),
                meta,
            ),
            (
                layered(w, h, &[(IRect::new(10, 10, 80, 50), BODY, 255)]),
                meta,
            ),
            // Translucent, twice; nothing visible; no meta.
            (
                layered(w, h, &[(IRect::new(20, 20, 40, 30), BODY, 60)]),
                meta,
            ),
            (
                layered(w, h, &[(IRect::new(20, 20, 40, 30), BODY, 60)]),
                meta,
            ),
            (layered(w, h, &[]), meta),
            (
                layered(w, h, &[(IRect::new(30, 30, 40, 30), BODY, 255)]),
                None,
            ),
            (
                layered(w, h, &[(IRect::new(30, 31, 40, 30), BODY, 255)]),
                None,
            ),
        ];
        let mut state = State::default();
        for (n, (data, crop)) in frames.iter().enumerate() {
            let pixels = Pixels {
                width: w as u32,
                height: h as u32,
                stride: w as usize * 4,
                data,
            };
            assert_eq!(
                find_in(&mut state, &pixels, *crop),
                window_rect(&pixels, *crop),
                "buffer {n}"
            );
        }
        let window = IRect::new(10, 10, 80, 50);
        let clean = layered(w, h, &[(window, BODY, 255)]);
        let stray = layered(
            w,
            h,
            &[(window, BODY, 255), (IRect::new(110, 70, 1, 1), STALE, 255)],
        );
        let pixels = |data| Pixels {
            width: w as u32,
            height: h as u32,
            stride: w as usize * 4,
            data,
        };
        assert_eq!(find_in(&mut state, &pixels(&clean), meta), Some(window));
        assert_eq!(find_in(&mut state, &pixels(&stray), meta), Some(window));
        assert_eq!(
            window_rect(&pixels(&stray), meta),
            Some(IRect::new(10, 10, 101, 61))
        );
    }
}
