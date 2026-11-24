//! Pure geometry of a recording (add-screen-recording design.md D2): which buffer pixels are
//! recorded and how they fit into the encoded frame. No GStreamer, no GTK.
//!
//! Three conversions:
//! - [`region_crop`]: a region selection (overlay, logical px) to a crop of the monitor stream's
//!   buffers, at the buffer's pixel density (Q8).
//! - [`window_frame_size`]: a window recording's frame size, the window's size on screen in
//!   monitor pixels ([`screen_factor`]: monitor scale ÷ buffer scale, maintainer decision
//!   2026-11-24). Where the window is in each buffer is `capture::screencast::window_rect`'s
//!   job (`window.rs`).
//! - [`letterbox`]: a content rectangle into the fixed frame of a window recording (D2 confirmed
//!   2026-11-23): scaled to fit, aspect kept, centred, never stretched.

use crate::capture::geometry::IRect;

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

/// What is known about the scales of a window stream (design.md D2, maintainer decision
/// 2026-11-24). Mirrors `screencast::Scales` of the window-still fix
/// (`fix/window-capture-screen-size`, not merged yet), so the recording can switch to it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scales {
    /// The stream's `size` in logical pixels: on GNOME 50.1 the monitor's logical size.
    pub stream_size: Option<(i32, i32)>,
    /// GDK's scale of the monitor that shows the window, where it can be told.
    pub monitor: Option<f64>,
}

/// From this factor up, the buffer counts as at the monitor's scale already: kept as it is.
const SAME_SCALE: f64 = 0.999;

/// Monitor pixels per buffer pixel of a window stream, `None` to keep the buffer's pixels
/// (same semantics as the window-still fix's `screen_size`): the monitor's scale ÷ the buffer's
/// scale, where the buffer's scale is the buffer's width ÷ the stream's logical width (6144 ÷
/// 3072 = 2, so 1.25 ÷ 2 = 0.625 on GNOME 50.1 at 125 %). Without both, a crop meta that cut
/// the window (it was at the monitor's scale) gives the factor directly: meta ÷ window, when
/// both directions agree within 2 %. Nothing is enlarged.
pub fn screen_factor(
    rect: IRect,
    buffer_width: i32,
    crop: Option<IRect>,
    scales: Scales,
) -> Option<f64> {
    let factor = match (scales.stream_size, scales.monitor) {
        (Some((logical, _)), Some(monitor)) if logical > 0 && buffer_width > 0 => {
            monitor * f64::from(logical) / f64::from(buffer_width)
        }
        _ => meta_factor(rect, crop)?,
    };
    (factor.is_finite() && factor > 0.0 && factor < SAME_SCALE).then_some(factor)
}

/// Meta ÷ window, when the crop meta is smaller than the window in both directions by the same
/// ratio (within 2 %): a meta at the monitor's scale that cut the window.
fn meta_factor(rect: IRect, crop: Option<IRect>) -> Option<f64> {
    let crop = crop?;
    if rect.is_empty() {
        return None;
    }
    let fx = f64::from(crop.w) / f64::from(rect.w);
    let fy = f64::from(crop.h) / f64::from(rect.h);
    (fx < SAME_SCALE && fy < SAME_SCALE && (fx - fy).abs() <= 0.02 * fx.max(fy))
        .then_some((fx + fy) / 2.0)
}

/// The encoded frame size of a window recording (D2, maintainer decision 2026-11-24): the
/// first content rectangle (`start`, buffer px, even) at the window's size on screen in monitor
/// pixels, i.e. times [`screen_factor`], rounded and trimmed to even sides for x264.
pub fn window_frame_size(start: Size, factor: Option<f64>) -> Size {
    let side = |n: i32| match factor {
        Some(f) => even((f64::from(n) * f).round() as i32),
        None => even(n),
    };
    Size::new(side(start.w).max(2), side(start.h).max(2))
}

/// The one scale of the monitors that may show a stream (same rule as the window-still fix's
/// `pick_monitor_scale`): those matching the stream's geometry if any, else all; `None` when
/// there is none or they differ.
pub fn single_scale(matching: &[f64], all: &[f64]) -> Option<f64> {
    let one = |scales: &[f64]| {
        let first = *scales.first()?;
        (first.is_finite()
            && first > 0.0
            && scales.iter().all(|scale| (scale - first).abs() < 1e-6))
        .then_some(first)
    };
    if matching.is_empty() {
        one(all)
    } else {
        one(matching)
    }
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

    /// GNOME 50.1 at 125 % (2026-11-23/24): stream `size` 3072×1728, buffers 6144×3456 (scale 2).
    const AT_125: Scales = Scales {
        stream_size: Some((3072, 1728)),
        monitor: Some(1.25),
    };

    /// The frame size of a window recording from its first content rectangle in buffer px.
    fn frame_of(start: Size, crop: Option<IRect>, scales: Scales) -> Size {
        let rect = IRect::new(0, 0, start.w, start.h);
        window_frame_size(start, screen_factor(rect, 6144, crop, scales))
    }

    // spec: screen-recording / Recording source / Window recorded at its on-screen size
    #[test]
    fn a_window_frame_has_the_window_size_in_monitor_pixels() {
        // 640×400 logical at 1.25 = 800×500 monitor px, 1280×800 in the scale-2 buffer.
        assert_eq!(
            screen_factor(IRect::new(0, 0, 1280, 800), 6144, None, AT_125),
            Some(0.625)
        );
        assert_eq!(
            frame_of(Size::new(1280, 800), None, AT_125),
            Size::new(800, 500)
        );
        // With its outline (2 buffer px per side, measured live): 1284×804 × 0.625 = 802.5 ×
        // 502.5, rounded to 803 × 503 and trimmed to even for x264.
        assert_eq!(
            frame_of(Size::new(1284, 804), None, AT_125),
            Size::new(802, 502)
        );
        // Maximised under a 60 px top bar: 6144×3360 → 3840×2100.
        assert_eq!(
            frame_of(Size::new(6144, 3360), None, AT_125),
            Size::new(3840, 2100)
        );
    }

    // spec: screen-recording / Recording source / Window recorded at its on-screen size
    #[test]
    fn a_window_frame_keeps_the_buffer_pixels_where_the_scales_agree_or_are_unknown() {
        // Buffers at the monitor's own scale (scale 1, or 2 on a 200 % monitor): unchanged.
        let at_1 = Scales {
            stream_size: Some((6144, 3456)),
            monitor: Some(1.0),
        };
        assert_eq!(
            frame_of(Size::new(1284, 804), None, at_1),
            Size::new(1284, 804)
        );
        let at_2 = Scales {
            stream_size: Some((3072, 1728)),
            monitor: Some(2.0),
        };
        assert_eq!(
            frame_of(Size::new(1283, 805), None, at_2),
            Size::new(1282, 804)
        );
        // Never enlarged: a buffer below the monitor's scale stays as it is.
        let below = Scales {
            stream_size: Some((6144, 3456)),
            monitor: Some(1.5),
        };
        assert_eq!(
            frame_of(Size::new(1280, 800), None, below),
            Size::new(1280, 800)
        );
        // Monitor scale unknown: a crop meta that cut the window at 1/1.6 gives the factor...
        let unknown = Scales {
            stream_size: Some((3072, 1728)),
            monitor: None,
        };
        let meta = Some(IRect::new(0, 0, 800, 500));
        assert_eq!(
            frame_of(Size::new(1280, 800), meta, unknown),
            Size::new(800, 500)
        );
        // ...a meta with the shadow margin in it (not one ratio) or none keeps the buffer's
        // pixels, as the window-still fix does.
        let margin = Some(IRect::new(0, 0, 928, 628));
        assert_eq!(
            frame_of(Size::new(1284, 804), margin, unknown),
            Size::new(1284, 804)
        );
        assert_eq!(
            frame_of(Size::new(1284, 804), None, unknown),
            Size::new(1284, 804)
        );
        assert_eq!(frame_of(Size::new(1, 1), None, unknown), Size::new(2, 2));
    }

    // spec: screen-recording / Recording source / Window recorded at its on-screen size
    #[test]
    fn the_monitor_scale_is_the_one_scale_of_the_matching_monitors() {
        assert_eq!(single_scale(&[1.25], &[1.0, 1.25]), Some(1.25));
        assert_eq!(single_scale(&[], &[1.25, 1.25]), Some(1.25));
        assert_eq!(single_scale(&[1.0, 2.0], &[1.0, 2.0]), None);
        assert_eq!(single_scale(&[], &[1.0, 1.5]), None);
        assert_eq!(single_scale(&[], &[]), None);
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
