//! Pure geometry of a recording (add-screen-recording design.md D2): which buffer pixels are
//! recorded and how they fit into the encoded frame. No GStreamer, no GTK.
//!
//! Three conversions:
//! - [`region_crop`]: a region selection (overlay, logical px) to a crop of the monitor stream's
//!   buffers, at the buffer's pixel density (Q8).
//! - [`window_crop_in_buffer`] / [`window_content`]: a window stream's crop meta (monitor-scale px,
//!   measured on GNOME 50) to the window's content rectangle in buffer pixels, with the alpha rule
//!   of `capture::screencast::window_rect` as the fallback. Kept self-contained so it can later be
//!   swapped for the window-still fix's own helper in `src/capture`.
//! - [`letterbox`]: a content rectangle into the fixed frame of a window recording (D2 confirmed
//!   2026-11-23): scaled to fit, aspect kept, centred, never stretched.

use crate::capture::geometry::IRect;
use crate::capture::screencast::{window_rect, Pixels};

/// A size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

impl Size {
    pub const fn new(w: i32, h: i32) -> Self {
        Self { w, h }
    }

    fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// A monitor stream as the ScreenCast portal describes it: `position` and `size` in logical px
/// (measured P2 on GNOME 50 at scale 1.25).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorStream {
    pub position: (i32, i32),
    pub size: Size,
}

impl MonitorStream {
    fn logical_rect(&self) -> IRect {
        IRect::new(self.position.0, self.position.1, self.size.w, self.size.h)
    }
}

/// Why a region cannot be recorded from a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionError {
    /// The stream does not contain the whole selection ("Region not on the chosen screen").
    NotOnScreen,
    /// The selection, the stream or its buffers have no area.
    Empty,
}

/// Whether the monitor stream contains the whole selection (both logical px). Known before the
/// stream starts, so a recording that would fail is refused before anything is written.
pub fn region_on_stream(selection: IRect, stream: MonitorStream) -> Result<(), RegionError> {
    if selection.is_empty() || stream.size.is_empty() {
        return Err(RegionError::Empty);
    }
    let area = stream.logical_rect();
    let inside = selection.x >= area.x
        && selection.y >= area.y
        && selection.right() <= area.right()
        && selection.bottom() <= area.bottom();
    if inside {
        Ok(())
    } else {
        Err(RegionError::NotOnScreen)
    }
}

/// The buffer crop of a region recording: (selection − stream position) × (buffer size ÷ stream
/// size), per axis, edges rounded, clamped to the buffer and trimmed to even width and height for
/// x264.
pub fn region_crop(
    selection: IRect,
    stream: MonitorStream,
    buffer: Size,
) -> Result<IRect, RegionError> {
    region_on_stream(selection, stream)?;
    if buffer.is_empty() {
        return Err(RegionError::Empty);
    }
    let fx = f64::from(buffer.w) / f64::from(stream.size.w);
    let fy = f64::from(buffer.h) / f64::from(stream.size.h);
    let edge = |logical: i32, origin: i32, factor: f64, limit: i32| {
        ((f64::from(logical - origin) * factor).round() as i32).clamp(0, limit)
    };
    let x0 = edge(selection.x, stream.position.0, fx, buffer.w);
    let x1 = edge(selection.right(), stream.position.0, fx, buffer.w);
    let y0 = edge(selection.y, stream.position.1, fy, buffer.h);
    let y1 = edge(selection.bottom(), stream.position.1, fy, buffer.h);
    let rect = IRect::new(x0, y0, even(x1 - x0), even(y1 - y0));
    if rect.is_empty() {
        Err(RegionError::Empty)
    } else {
        Ok(rect)
    }
}

/// The largest even number not above `n` (x264 needs even frame sizes).
pub fn even(n: i32) -> i32 {
    n - n.rem_euclid(2)
}

/// `rect` trimmed at its right and bottom edge to even width and height.
pub fn even_rect(rect: IRect) -> IRect {
    IRect::new(rect.x, rect.y, even(rect.w), even(rect.h))
}

/// A window stream's crop meta in buffer pixels (design.md D2): the meta is in monitor-scale px,
/// the buffer at its own scale, so the factor is (buffer size ÷ stream size) ÷ monitor scale per
/// axis. `None` when the factor cannot be computed (no scale, empty sizes); the caller then
/// finds the window by alpha ([`window_content`]). The result is clamped to the buffer.
pub fn window_crop_in_buffer(
    crop: IRect,
    stream_size: Size,
    buffer: Size,
    monitor_scale: Option<f64>,
) -> Option<IRect> {
    let scale = monitor_scale.filter(|s| s.is_finite() && *s > 0.0)?;
    if stream_size.is_empty() || buffer.is_empty() || crop.is_empty() {
        return None;
    }
    let fx = f64::from(buffer.w) / f64::from(stream_size.w) / scale;
    let fy = f64::from(buffer.h) / f64::from(stream_size.h) / scale;
    let x0 = (f64::from(crop.x) * fx).round() as i32;
    let y0 = (f64::from(crop.y) * fy).round() as i32;
    let x1 = (f64::from(crop.right()) * fx).round() as i32;
    let y1 = (f64::from(crop.bottom()) * fy).round() as i32;
    IRect::from_corners((x0, y0), (x1, y1)).intersect(IRect::new(0, 0, buffer.w, buffer.h))
}

/// The monitor scale for [`window_crop_in_buffer`]: the scale of the one monitor (or of all
/// monitors) the stream matched; where none or several of different scales matched, the one
/// scale every monitor has, else 1. Never "unknown": with any scale the converted crop meta
/// bounds the window, while the alpha rule over the whole buffer would take opaque undefined
/// pixels outside the window in (design.md D2).
pub fn fallback_scale(matching: &[f64], all: &[f64]) -> f64 {
    let single = |scales: &[f64]| {
        let first = *scales.first()?;
        (first.is_finite() && first > 0.0 && scales.iter().all(|s| *s == first)).then_some(first)
    };
    single(matching).or_else(|| single(all)).unwrap_or(1.0)
}

/// The window's content rectangle in one buffer: the crop meta converted to buffer pixels
/// ([`window_crop_in_buffer`]), then trimmed to the window without its shadow by
/// `capture::screencast::window_rect`'s alpha rule. Where the factor cannot be computed, the
/// alpha rule alone over the whole buffer. `None`: no visible pixel.
pub fn window_content(
    pixels: &Pixels,
    crop_meta: Option<IRect>,
    stream_size: Size,
    monitor_scale: Option<f64>,
) -> Option<IRect> {
    let buffer = Size::new(
        i32::try_from(pixels.width).ok()?,
        i32::try_from(pixels.height).ok()?,
    );
    let area = crop_meta
        .and_then(|crop| window_crop_in_buffer(crop, stream_size, buffer, monitor_scale))
        .and_then(|r| {
            Some((
                u32::try_from(r.x).ok()?,
                u32::try_from(r.y).ok()?,
                u32::try_from(r.w).ok()?,
                u32::try_from(r.h).ok()?,
            ))
        });
    window_rect(pixels, area)
}

/// Where `content` goes inside a fixed `frame` (D2): scaled by one factor so it fits, centred,
/// the rest black. Never stretched: width and height scale alike (up to rounding).
pub fn letterbox(frame: Size, content: Size) -> IRect {
    if frame.is_empty() || content.is_empty() {
        return IRect::new(0, 0, frame.w.max(0), frame.h.max(0));
    }
    let scale =
        (f64::from(frame.w) / f64::from(content.w)).min(f64::from(frame.h) / f64::from(content.h));
    let w = ((f64::from(content.w) * scale).round() as i32).clamp(1, frame.w);
    let h = ((f64::from(content.h) * scale).round() as i32).clamp(1, frame.h);
    IRect::new((frame.w - w) / 2, (frame.h - h) / 2, w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALED: MonitorStream = MonitorStream {
        position: (0, 0),
        size: Size::new(3072, 1728),
    };

    // spec: screen-recording / Recording source / Record a region on a scaled monitor
    #[test]
    fn region_on_a_scaled_monitor_maps_to_buffer_pixels() {
        let crop = region_crop(
            IRect::new(100, 100, 800, 600),
            SCALED,
            Size::new(3840, 2160),
        );
        assert_eq!(crop, Ok(IRect::new(125, 125, 1000, 750)));
    }

    // spec: screen-recording / Recording source / Record a region
    #[test]
    fn region_with_buffers_of_the_stream_size_keeps_its_size() {
        let stream = MonitorStream {
            position: (0, 0),
            size: Size::new(1920, 1080),
        };
        let selection = IRect::new(300, 200, 800, 600);
        assert_eq!(
            region_crop(selection, stream, Size::new(1920, 1080)),
            Ok(selection)
        );
        // A second monitor at (1920, 0): the selection is taken relative to its position.
        let right = MonitorStream {
            position: (1920, 0),
            size: Size::new(1920, 1080),
        };
        assert_eq!(
            region_crop(IRect::new(2020, 50, 800, 600), right, Size::new(1920, 1080)),
            Ok(IRect::new(100, 50, 800, 600))
        );
        // Odd sizes are trimmed to even ones for x264.
        assert_eq!(
            region_crop(IRect::new(0, 0, 801, 599), stream, Size::new(1920, 1080)),
            Ok(IRect::new(0, 0, 800, 598))
        );
    }

    // spec: screen-recording / Recording source / Region not on the chosen screen
    #[test]
    fn region_outside_the_stream_is_detected() {
        let stream = MonitorStream {
            position: (1920, 0),
            size: Size::new(1920, 1080),
        };
        // Wholly on the other monitor, partly over the edge, and just inside.
        for (selection, expected) in [
            (
                IRect::new(100, 100, 800, 600),
                Err(RegionError::NotOnScreen),
            ),
            (
                IRect::new(1800, 100, 800, 600),
                Err(RegionError::NotOnScreen),
            ),
            (
                IRect::new(3000, 500, 841, 600),
                Err(RegionError::NotOnScreen),
            ),
            (IRect::new(3040, 480, 800, 600), Ok(())),
        ] {
            assert_eq!(
                region_on_stream(selection, stream),
                expected,
                "{selection:?}"
            );
        }
        assert_eq!(
            region_crop(
                IRect::new(100, 100, 800, 600),
                stream,
                Size::new(1920, 1080)
            ),
            Err(RegionError::NotOnScreen)
        );
        assert_eq!(
            region_on_stream(IRect::new(2000, 0, 0, 10), stream),
            Err(RegionError::Empty)
        );
    }

    // spec: screen-recording / Recording source / Window crop meta in buffer pixels
    #[test]
    fn window_crop_meta_is_converted_by_buffer_over_size_over_scale() {
        let buffer = Size::new(6144, 3456);
        let size = Size::new(3072, 1728);
        assert_eq!(
            window_crop_in_buffer(IRect::new(0, 0, 800, 500), size, buffer, Some(1.25)),
            Some(IRect::new(0, 0, 1280, 800))
        );
        // The maximised window of P6 (scenario "Recorded window resized").
        assert_eq!(
            window_crop_in_buffer(IRect::new(0, 0, 3840, 2100), size, buffer, Some(1.25)),
            Some(IRect::new(0, 0, 6144, 3360))
        );
        // No factor without a scale: the caller falls back to the alpha rule.
        assert_eq!(
            window_crop_in_buffer(IRect::new(0, 0, 800, 500), size, buffer, None),
            None
        );
        assert_eq!(
            window_crop_in_buffer(IRect::new(0, 0, 800, 500), size, buffer, Some(0.0)),
            None
        );
    }

    // spec: screen-recording / Recording source / Window crop meta in buffer pixels
    #[test]
    fn window_content_trims_by_alpha_inside_the_converted_crop() {
        // 64×32 buffer at twice the stream size, scale 1.25: factor 1.6. The window body
        // (alpha 255) is at (4, 4, 24, 16); a faint shadow (alpha 30) around it; stale opaque
        // pixels outside the crop meta must not count.
        let (w, h) = (64u32, 32u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        let mut set = |x: u32, y: u32, alpha: u8| {
            let offset = ((y * w + x) * 4) as usize;
            data[offset..offset + 4].copy_from_slice(&[10, 20, 30, alpha]);
        };
        for y in 0..h {
            for x in 0..w {
                if (40..64).contains(&x) {
                    set(x, y, 255); // undefined pixels outside the window
                } else if (4..28).contains(&x) && (4..20).contains(&y) {
                    set(x, y, 255);
                } else if x < 32 && y < 24 {
                    set(x, y, 30);
                }
            }
        }
        let pixels = Pixels {
            width: w,
            height: h,
            stride: (w * 4) as usize,
            data: &data,
        };
        // Crop meta (0, 0, 20, 15) in monitor-scale px → (0, 0, 32, 24) in the buffer.
        assert_eq!(
            window_content(
                &pixels,
                Some(IRect::new(0, 0, 20, 15)),
                Size::new(32, 16),
                Some(1.25)
            ),
            Some(IRect::new(4, 4, 24, 16))
        );
        // Without a scale the alpha rule alone decides over the whole buffer (here it also
        // finds the undefined pixels, which is why the crop meta is preferred).
        assert_eq!(
            window_content(
                &pixels,
                Some(IRect::new(0, 0, 20, 15)),
                Size::new(32, 16),
                None
            ),
            Some(IRect::new(4, 0, 60, 32))
        );
    }

    /// "Record a window" says the recording shows only that window: whichever scale the
    /// fallback finds, the content rectangle lies inside the converted crop meta, so a window
    /// frame never takes pixels from outside it.
    // spec: screen-recording / Recording source / Record a window
    #[test]
    fn without_a_matching_monitor_the_crop_meta_still_bounds_the_window() {
        // The matching monitor's scale wins; else the scale all monitors share; else 1.
        assert_eq!(fallback_scale(&[1.25], &[1.0, 1.25]), 1.25);
        assert_eq!(fallback_scale(&[], &[1.25, 1.25]), 1.25);
        assert_eq!(fallback_scale(&[1.0, 2.0], &[1.0, 2.0]), 1.0);
        assert_eq!(fallback_scale(&[], &[1.0, 1.5]), 1.0);
        assert_eq!(fallback_scale(&[], &[]), 1.0);
        // P6 scaled down: a 64×32 buffer of a 32×16 stream, crop meta (0, 0, 20, 15). With
        // scale 1 the meta maps to (0, 0, 40, 30): wider than the true 1.6 factor, but the
        // opaque undefined pixels right of it (x ≥ 40) are never taken.
        let (w, h) = (64u32, 32u32);
        let mut data = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let alpha = if x >= 40 || ((4..28).contains(&x) && (4..20).contains(&y)) {
                    255
                } else {
                    0
                };
                let offset = ((y * w + x) * 4) as usize;
                data[offset..offset + 4].copy_from_slice(&[10, 20, 30, alpha]);
            }
        }
        let pixels = Pixels {
            width: w,
            height: h,
            stride: (w * 4) as usize,
            data: &data,
        };
        let scale = fallback_scale(&[], &[1.0, 2.0]);
        assert_eq!(
            window_content(
                &pixels,
                Some(IRect::new(0, 0, 20, 15)),
                Size::new(32, 16),
                Some(scale)
            ),
            Some(IRect::new(4, 4, 24, 16))
        );
        // Every scale the fallback can give: the window's rectangle inside the converted crop
        // meta, never reaching the undefined pixels at x ≥ 40.
        let (meta, size, buffer) = (
            IRect::new(0, 0, 20, 15),
            Size::new(32, 16),
            Size::new(64, 32),
        );
        for (matching, all) in [
            (&[1.25][..], &[1.0, 1.25][..]),
            (&[], &[1.25, 1.25]),
            (&[1.0, 2.0], &[1.0, 2.0]),
            (&[], &[1.0, 1.5]),
            (&[], &[]),
        ] {
            let scale = fallback_scale(matching, all);
            let crop = window_crop_in_buffer(meta, size, buffer, Some(scale)).unwrap();
            let content = window_content(&pixels, Some(meta), size, Some(scale)).unwrap();
            assert_eq!(content.intersect(crop), Some(content), "scale {scale}");
            assert!(content.right() <= 40, "scale {scale}: {content:?}");
        }
    }

    // spec: screen-recording / Recording source / Recorded window resized
    #[test]
    fn a_larger_window_is_letterboxed_into_the_first_frame() {
        let frame = Size::new(1280, 800);
        // Same size: fills the frame.
        assert_eq!(letterbox(frame, frame), IRect::new(0, 0, 1280, 800));
        // Maximised (6144×3360): scaled by 1280/6144 in both axes, bars above and below.
        let fit = letterbox(frame, Size::new(6144, 3360));
        assert_eq!(fit, IRect::new(0, 50, 1280, 700));
        let (sx, sy) = (f64::from(fit.w) / 6144.0, f64::from(fit.h) / 3360.0);
        assert!((sx - sy).abs() < 0.001, "never stretched: {sx} vs {sy}");
        // A tall window: bars left and right.
        assert_eq!(
            letterbox(frame, Size::new(400, 800)),
            IRect::new(440, 0, 400, 800)
        );
        assert_eq!(even(1001), 1000);
        assert_eq!(even_rect(IRect::new(3, 3, 7, 9)), IRect::new(3, 3, 6, 8));
    }
}
