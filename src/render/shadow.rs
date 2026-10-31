//! Soft drop shadows for any shape, used by object types and the canvas chrome.

use super::blur::blur_a8;
use crate::document::color::Rgba;

/// Paints the blurred silhouette of the path drawn by `shape` in `color` × `opacity`.
///
/// The context's current transformation defines where the shape is; apply any shadow offset
/// before calling. `blur` is the Gaussian standard deviation in *user* units, so the shadow
/// scales with zoom. The blur runs on an offscreen mask at device resolution, limited to the
/// visible clip area, so huge zoom levels do not allocate huge surfaces.
pub fn paint(
    cr: &cairo::Context,
    color: Rgba,
    opacity: f64,
    blur: f64,
    shape: impl Fn(&cairo::Context),
) -> Result<(), cairo::Error> {
    // Shape extents in user space.
    cr.save()?;
    cr.new_path();
    shape(cr);
    let extents = cr.fill_extents()?;
    cr.new_path();
    cr.restore()?;
    paint_mask(cr, color, opacity, blur, extents, |mcr| {
        shape(mcr);
        mcr.set_source_rgba(0.0, 0.0, 0.0, 1.0);
        mcr.fill()
    })
}

/// The drop shadow of `shadow` for whatever `draw` paints (its alpha, like CSS
/// `drop-shadow()`): strokes, text outlines and several shapes at once. `bounds` (user space)
/// must cover everything `draw` paints. Paint the content itself afterwards.
pub fn drop_shadow(
    cr: &cairo::Context,
    shadow: &crate::document::style::Shadow,
    bounds: crate::document::geometry::Rect,
    draw: impl Fn(&cairo::Context) -> Result<(), cairo::Error>,
) -> Result<(), cairo::Error> {
    if !shadow.is_visible() || bounds.is_none() {
        return Ok(());
    }
    let (dx, dy) = shadow.offset();
    cr.save()?;
    cr.translate(dx, dy);
    let painted = paint_mask(
        cr,
        shadow.color,
        shadow.opacity,
        shadow.blur,
        (bounds.x, bounds.y, bounds.right(), bounds.bottom()),
        draw,
    );
    cr.restore()?;
    painted
}

/// Blurs the alpha of what `fill_mask` paints within `extents` (user space) and paints
/// `color` × `opacity` through it.
fn paint_mask(
    cr: &cairo::Context,
    color: Rgba,
    opacity: f64,
    blur: f64,
    extents: (f64, f64, f64, f64),
    fill_mask: impl FnOnce(&cairo::Context) -> Result<(), cairo::Error>,
) -> Result<(), cairo::Error> {
    let alpha = f64::from(color.a) / 255.0 * opacity.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return Ok(());
    }
    let m = cr.matrix();
    let user_scale = (m.xx() * m.yy() - m.xy() * m.yx()).abs().sqrt();
    let (dsx, dsy) = cr.target().device_scale();
    let sigma = blur.max(0.0) * user_scale;
    let (x1, y1, x2, y2) = extents;
    let (mut dx0, mut dy0, mut dx1, mut dy1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (x, y) in [(x1, y1), (x2, y1), (x2, y2), (x1, y2)] {
        let (dx, dy) = cr.user_to_device(x, y);
        dx0 = dx0.min(dx);
        dy0 = dy0.min(dy);
        dx1 = dx1.max(dx);
        dy1 = dy1.max(dy);
    }
    let margin = (3.0 * sigma).ceil() + 1.0;
    // Visible area in the same space.
    cr.save()?;
    cr.identity_matrix();
    let (cx0, cy0, cx1, cy1) = cr.clip_extents()?;
    cr.restore()?;
    let x0 = (dx0 - margin).max(cx0 - margin).floor();
    let y0 = (dy0 - margin).max(cy0 - margin).floor();
    let x1 = (dx1 + margin).min(cx1 + margin).ceil();
    let y1 = (dy1 + margin).min(cy1 + margin).ceil();
    if x1 <= x0 || y1 <= y0 {
        return Ok(());
    }
    let width = ((x1 - x0) * dsx).ceil() as i32;
    let height = ((y1 - y0) * dsy).ceil() as i32;
    if width <= 0 || height <= 0 || width > 32_767 || height > 32_767 {
        return Ok(());
    }

    let mut mask = cairo::ImageSurface::create(cairo::Format::A8, width, height)?;
    mask.set_device_scale(dsx, dsy);
    {
        let mcr = cairo::Context::new(&mask)?;
        mcr.set_matrix(cairo::Matrix::new(
            m.xx(),
            m.yx(),
            m.xy(),
            m.yy(),
            m.x0() - x0,
            m.y0() - y0,
        ));
        fill_mask(&mcr)?;
    }
    mask.flush();
    let stride = mask.stride() as usize;
    if let Ok(mut data) = mask.data() {
        blur_a8(
            &mut data,
            width as usize,
            height as usize,
            stride,
            sigma * dsx.max(dsy),
        );
    }
    mask.mark_dirty();

    cr.save()?;
    cr.identity_matrix();
    cr.set_source_rgba(
        f64::from(color.r) / 255.0,
        f64::from(color.g) / 255.0,
        f64::from(color.b) / 255.0,
        alpha,
    );
    cr.mask_surface(&mask, x0, y0)?;
    cr.restore()
}

/// Adds a rounded rectangle to the path (radius clamped to half the shorter side).
pub fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, radius: f64) {
    let r = radius.max(0.0).min(w.abs() / 2.0).min(h.abs() / 2.0);
    if r <= 0.0 {
        cr.rectangle(x, y, w, h);
        return;
    }
    use std::f64::consts::{FRAC_PI_2, PI};
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(surface: &cairo::ImageSurface, x: i32, y: i32) -> u8 {
        let mut a = 0;
        let stride = surface.stride() as usize;
        surface
            .with_data(|d| {
                let i = y as usize * stride + x as usize * 4;
                a = (u32::from_ne_bytes([d[i], d[i + 1], d[i + 2], d[i + 3]]) >> 24) as u8;
            })
            .unwrap();
        a
    }

    #[test]
    fn shadow_is_soft_and_offset() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 100, 100).unwrap();
        {
            let cr = cairo::Context::new(&surface).unwrap();
            cr.translate(10.0, 10.0); // the offset
            paint(&cr, Rgba::opaque(0, 0, 0), 1.0, 3.0, |cr| {
                cr.rectangle(30.0, 30.0, 30.0, 30.0)
            })
            .unwrap();
        }
        surface.flush();
        // Inside the offset square: solid.
        assert!(alpha_at(&surface, 55, 55) > 250);
        // On the edge: half.
        let edge = alpha_at(&surface, 40, 55);
        assert!((100..160).contains(&edge), "edge {edge}");
        // Outside, beyond 3 sigma: nothing.
        assert_eq!(alpha_at(&surface, 25, 55), 0);
        // Not drawn at the un-offset place's top-left.
        assert_eq!(alpha_at(&surface, 31, 31), 0);
    }

    #[test]
    fn shadow_respects_zoom() {
        // At 2× zoom the blur is twice as wide in device pixels.
        let width_at = |zoom: f64| {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 200, 60).unwrap();
            {
                let cr = cairo::Context::new(&surface).unwrap();
                cr.scale(zoom, zoom);
                paint(&cr, Rgba::opaque(0, 0, 0), 1.0, 4.0, |cr| {
                    cr.rectangle(40.0 / zoom, 0.0, 200.0, 60.0)
                })
                .unwrap();
            }
            surface.flush();
            (0..40).filter(|&x| alpha_at(&surface, x, 30) > 0).count()
        };
        assert!(width_at(2.0) > width_at(1.0));
    }
}
