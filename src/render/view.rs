//! Zoom and scroll arithmetic for the canvas (spec editor-canvas: View control), kept free of
//! widgets so it can be tested.
//!
//! The canvas scrolls over a content area of [`content_size`]: the zoomed canvas plus a
//! [`FIT_MARGIN`] on every side (at least the view). When the view is larger than that, the
//! canvas is centred. The widget itself is only as large as the view; [`view_origin`] maps
//! document space into it for a scroll position.

use crate::document::geometry::{IRect, Point};

pub const MIN_ZOOM: f64 = 0.10;
pub const MAX_ZOOM: f64 = 16.0;
/// Zoom levels of the zoom popover (mockup 07); Ctrl++ / Ctrl+− step through them.
pub const PRESETS: [f64; 7] = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0];
/// Space around the canvas in screen pixels; mockup 10 fits a 2000 px canvas into the
/// 928 px canvas area at 42 %.
pub const FIT_MARGIN: f64 = 44.0;
/// Zoom factor per Ctrl+scroll step.
pub const SCROLL_FACTOR: f64 = 1.1;

pub fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_finite() {
        zoom.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

/// Zoom at which the whole canvas fits the view with margins; never above 100 %
/// (spec "Fit": the whole image is visible and centred).
pub fn fit_zoom(canvas: IRect, view: (f64, f64)) -> f64 {
    let w = f64::from(canvas.width.max(1));
    let h = f64::from(canvas.height.max(1));
    let zx = (view.0 - 2.0 * FIT_MARGIN).max(1.0) / w;
    let zy = (view.1 - 2.0 * FIT_MARGIN).max(1.0) / h;
    clamp_zoom(zx.min(zy).min(1.0))
}

/// Zoom at which the canvas *width* fits the view with margins ("Fit Width" in the zoom
/// popover, mockup 07); like [`fit_zoom`] never above 100 %. The height may then scroll.
pub fn fit_width_zoom(canvas: IRect, view: (f64, f64)) -> f64 {
    let w = f64::from(canvas.width.max(1));
    clamp_zoom(((view.0 - 2.0 * FIT_MARGIN).max(1.0) / w).min(1.0))
}

/// The preset `zoom` is at, if any (the check mark in the zoom popover).
pub fn preset(zoom: f64) -> Option<f64> {
    PRESETS
        .iter()
        .copied()
        .find(|p| (p - zoom).abs() < 1e-6 * p.max(1.0))
}

/// Next preset above `zoom` (Ctrl++).
pub fn zoom_in(zoom: f64) -> f64 {
    PRESETS
        .iter()
        .copied()
        .find(|&p| p > zoom * 1.0001)
        .unwrap_or(MAX_ZOOM)
}

/// Next preset below `zoom` (Ctrl+−); 10 % below the smallest preset.
pub fn zoom_out(zoom: f64) -> f64 {
    PRESETS
        .iter()
        .rev()
        .copied()
        .find(|&p| p < zoom * 0.9999)
        .unwrap_or(MIN_ZOOM)
}

/// Size of the scrollable content at `zoom`.
pub fn content_size(canvas: IRect, zoom: f64) -> (f64, f64) {
    (
        f64::from(canvas.width) * zoom + 2.0 * FIT_MARGIN,
        f64::from(canvas.height) * zoom + 2.0 * FIT_MARGIN,
    )
}

/// Where document (0, 0) lands in content coordinates, for content allocated at `area`
/// (at least the view size): the canvas is centred when it is smaller than the area.
pub fn origin(canvas: IRect, zoom: f64, area: (f64, f64)) -> Point {
    let axis = |size: i32, start: i32, avail: f64| {
        let scaled = f64::from(size) * zoom;
        let left = ((avail - scaled) / 2.0).max(FIT_MARGIN);
        (left - f64::from(start) * zoom).round()
    };
    Point::new(
        axis(canvas.width, canvas.x, area.0),
        axis(canvas.height, canvas.y, area.1),
    )
}

/// Where document (0, 0) lands in the *view* (the visible, viewport-sized widget) for a view
/// of `view` pixels scrolled to `scroll` (content coordinates). Rounded to whole pixels so that
/// 100 % shows the exported pixels exactly.
pub fn view_origin(canvas: IRect, zoom: f64, view: (f64, f64), scroll: (f64, f64)) -> Point {
    let (cw, ch) = content_size(canvas, zoom);
    let o = origin(canvas, zoom, (cw.max(view.0), ch.max(view.1)));
    Point::new((o.x - scroll.0).round(), (o.y - scroll.1).round())
}

/// Scroll position after zooming from `old_zoom` to `new_zoom` so that the document point
/// under `anchor` (view coordinates) stays under it (Ctrl+scroll at the pointer).
pub fn scroll_for_zoom(
    canvas: IRect,
    view: (f64, f64),
    old_zoom: f64,
    old_scroll: (f64, f64),
    new_zoom: f64,
    anchor: (f64, f64),
) -> (f64, f64) {
    let area = |zoom: f64| {
        let (cw, ch) = content_size(canvas, zoom);
        (cw.max(view.0), ch.max(view.1))
    };
    let old_origin = origin(canvas, old_zoom, area(old_zoom));
    let new_area = area(new_zoom);
    let new_origin = origin(canvas, new_zoom, new_area);
    let doc_x = (old_scroll.0 + anchor.0 - old_origin.x) / old_zoom;
    let doc_y = (old_scroll.1 + anchor.1 - old_origin.y) / old_zoom;
    let sx = doc_x * new_zoom + new_origin.x - anchor.0;
    let sy = doc_y * new_zoom + new_origin.y - anchor.1;
    (
        sx.clamp(0.0, (new_area.0 - view.0).max(0.0)),
        sy.clamp(0.0, (new_area.1 - view.1).max(0.0)),
    )
}

/// Label of the zoom level button ("Fit · 50 %", "200 %"), without the translated "Fit".
pub fn percent(zoom: f64) -> u32 {
    (zoom * 100.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_matches_the_mockup() {
        // Mockup 10: 2000 × 1000 canvas in the 928 × 646 canvas area → "Fit · 42 %".
        let z = fit_zoom(IRect::new(0, 0, 2000, 1000), (928.0, 646.0));
        assert_eq!(percent(z), 42);
        // Small images are not enlarged.
        assert_eq!(fit_zoom(IRect::new(0, 0, 200, 100), (928.0, 646.0)), 1.0);
        // Tiny views stay in range.
        assert_eq!(
            fit_zoom(IRect::new(0, 0, 30_000, 30_000), (100.0, 100.0)),
            MIN_ZOOM
        );
    }

    #[test]
    fn fitted_canvas_is_visible_and_centred() {
        // Spec "Fit": the whole image is visible and centred.
        let canvas = IRect::new(0, 0, 1600, 1000);
        let view = (928.0, 646.0);
        let z = fit_zoom(canvas, view);
        let (cw, ch) = content_size(canvas, z);
        assert!(cw <= view.0 + 0.5 && ch <= view.1 + 0.5);
        let o = origin(canvas, z, view);
        let right = view.0 - (o.x + 1600.0 * z);
        let bottom = view.1 - (o.y + 1000.0 * z);
        assert!(o.x >= 0.0 && o.y >= 0.0 && right >= -0.5 && bottom >= -0.5);
        assert!((o.x - right).abs() <= 1.0, "{o:?} {right}");
        assert!((o.y - bottom).abs() <= 1.0, "{o:?} {bottom}");
    }

    #[test]
    fn extended_canvas_origin_accounts_for_negative_start() {
        // Canvas extended 200 px to the left: document (0,0) sits 200 px right of its edge.
        let canvas = IRect::new(-200, 0, 1200, 500);
        let o = origin(canvas, 1.0, (100.0, 100.0));
        assert_eq!(o, Point::new(FIT_MARGIN + 200.0, FIT_MARGIN));
    }

    #[test]
    fn fit_width_fills_the_width_only() {
        // 1600 × 1000 in a 928 × 400 view: Fit is limited by the height, Fit Width is not.
        let canvas = IRect::new(0, 0, 1600, 1000);
        let view = (928.0, 400.0);
        let zw = fit_width_zoom(canvas, view);
        assert!((zw - (928.0 - 2.0 * FIT_MARGIN) / 1600.0).abs() < 1e-12);
        assert!(zw > fit_zoom(canvas, view));
        // Never enlarged, like Fit.
        assert_eq!(fit_width_zoom(IRect::new(0, 0, 200, 100), view), 1.0);
    }

    #[test]
    fn presets_are_recognised() {
        assert_eq!(preset(2.0), Some(2.0));
        assert_eq!(preset(0.25), Some(0.25));
        assert_eq!(preset(0.42), None);
        assert_eq!(preset(MIN_ZOOM), None);
    }

    #[test]
    fn zoom_steps_and_range() {
        assert_eq!(zoom_in(0.5), 1.0);
        assert_eq!(zoom_in(0.42), 0.5);
        assert_eq!(zoom_in(16.0), MAX_ZOOM);
        assert_eq!(zoom_out(1.0), 0.5);
        assert_eq!(zoom_out(0.25), MIN_ZOOM);
        assert_eq!(zoom_out(MIN_ZOOM), MIN_ZOOM);
        assert_eq!(clamp_zoom(100.0), MAX_ZOOM);
        assert_eq!(clamp_zoom(0.01), MIN_ZOOM);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn view_origin_follows_the_scroll_position() {
        let canvas = IRect::new(0, 0, 4000, 3000);
        let view = (1000.0, 700.0);
        // Zoomed in: the content is larger than the view, scrolling moves the canvas.
        let o = view_origin(canvas, 16.0, view, (0.0, 0.0));
        assert_eq!(o, Point::new(FIT_MARGIN, FIT_MARGIN));
        let o = view_origin(canvas, 16.0, view, (30_000.0, 2_000.4));
        assert_eq!(o, Point::new(FIT_MARGIN - 30_000.0, FIT_MARGIN - 2_000.0));
        // The far edge of the content at the far end of the scroll range.
        let (cw, _) = content_size(canvas, 16.0);
        let o = view_origin(canvas, 16.0, view, (cw - view.0, 0.0));
        assert_eq!(o.x + 4000.0 * 16.0 + FIT_MARGIN, view.0);
        // Fitted: centred in the view whatever its size.
        for v in [(928.0, 646.0), (900.0, 564.0), (600.0, 400.0)] {
            let z = fit_zoom(canvas, v);
            let o = view_origin(canvas, z, v, (0.0, 0.0));
            let right = v.0 - (o.x + 4000.0 * z);
            let bottom = v.1 - (o.y + 3000.0 * z);
            assert!(
                (o.x - right).abs() <= 1.0 && (o.y - bottom).abs() <= 1.0,
                "{v:?}"
            );
            assert!(
                o.x >= 0.0 && o.y >= 0.0 && right >= -0.5 && bottom >= -0.5,
                "{v:?}"
            );
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer() {
        let canvas = IRect::new(0, 0, 4000, 3000);
        let view = (1000.0, 700.0);
        let anchor = (300.0, 200.0);
        let old_scroll = (1500.0, 900.0);
        let (old_zoom, new_zoom) = (1.0, 2.0);
        let s = scroll_for_zoom(canvas, view, old_zoom, old_scroll, new_zoom, anchor);
        let area = |z: f64| {
            let (w, h) = content_size(canvas, z);
            (w.max(view.0), h.max(view.1))
        };
        let o1 = origin(canvas, old_zoom, area(old_zoom));
        let o2 = origin(canvas, new_zoom, area(new_zoom));
        let before = (
            (old_scroll.0 + anchor.0 - o1.x) / old_zoom,
            (old_scroll.1 + anchor.1 - o1.y) / old_zoom,
        );
        let after = (
            (s.0 + anchor.0 - o2.x) / new_zoom,
            (s.1 + anchor.1 - o2.y) / new_zoom,
        );
        assert!((before.0 - after.0).abs() < 1e-6 && (before.1 - after.1).abs() < 1e-6);
    }
}
