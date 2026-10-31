//! Pixel effects of redactions (spec annotation-tools: Redaction): the original image's pixels
//! under a redaction, blurred or pixelated.
//!
//! The effect is computed in document pixels from the base image (`RenderContext::document`),
//! never from what the canvas shows, so the canvas at any zoom and the export show the same
//! pixels (scaled). Results are cached per image, area and effect (a small LRU per thread), so
//! redrawing the canvas does not blur again; moving or resizing a redaction recomputes it.

use std::cell::RefCell;

use super::blur::blur_argb32;
use crate::document::bitmap::Bitmap;
use crate::document::geometry::IRect;

/// What happens to the pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// Gaussian blur with this standard deviation (document pixels).
    Blur { sigma: f64 },
    /// Blocks of this many pixels square, each the average of its pixels.
    Pixelate { block: i32 },
}

/// The processed pixels and the document rectangle they cover.
#[derive(Clone, Debug)]
pub struct Processed {
    pub surface: cairo::ImageSurface,
    pub rect: IRect,
}

struct Entry {
    image: Bitmap,
    region: IRect,
    effect: Effect,
    result: Processed,
}

/// Cached results per thread, newest last.
const CACHE_SIZE: usize = 16;

thread_local! {
    static CACHE: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

/// The pixels of `image` within `region` (document pixels, the image at the origin) with
/// `effect` applied; `None` where `region` does not overlap the image.
pub fn processed(image: &Bitmap, region: IRect, effect: Effect) -> Option<Processed> {
    let hit = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let i = cache
            .iter()
            .position(|e| e.image.ptr_eq(image) && e.region == region && e.effect == effect)?;
        let entry = cache.remove(i);
        let result = entry.result.clone();
        cache.push(entry);
        Some(result)
    });
    if hit.is_some() {
        return hit;
    }
    let result = compute(image, region, effect)?;
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_SIZE {
            cache.remove(0);
        }
        cache.push(Entry {
            image: image.clone(),
            region,
            effect,
            result: result.clone(),
        });
    });
    Some(result)
}

fn compute(image: &Bitmap, region: IRect, effect: Effect) -> Option<Processed> {
    let bounds = IRect::new(0, 0, image.width(), image.height());
    region.intersection(&bounds)?;
    // A blur reads 3 σ around what it shows, or its edges would fade; beyond the image's own
    // edges its edge pixels are repeated (callers clip what they draw to the image).
    let margin = match effect {
        Effect::Blur { sigma } => (3.0 * sigma.max(0.0)).ceil() as i32,
        Effect::Pixelate { .. } => 0,
    };
    let grow = |r: IRect, by: i32| {
        IRect::new(
            r.x.saturating_sub(by),
            r.y.saturating_sub(by),
            r.width.saturating_add(2 * by),
            r.height.saturating_add(2 * by),
        )
    };
    let source = grow(region, margin).intersection(&grow(bounds, margin))?;
    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, source.width, source.height).ok()?;
    {
        let cr = cairo::Context::new(&surface).ok()?;
        let pattern = cairo::SurfacePattern::create(image.surface());
        pattern.set_extend(cairo::Extend::Pad);
        pattern.set_filter(cairo::Filter::Nearest);
        pattern.set_matrix(cairo::Matrix::new(
            1.0,
            0.0,
            0.0,
            1.0,
            f64::from(source.x),
            f64::from(source.y),
        ));
        cr.set_source(&pattern).ok()?;
        cr.set_operator(cairo::Operator::Source);
        cr.paint().ok()?;
    }
    surface.flush();
    let (w, h) = (source.width as usize, source.height as usize);
    let stride = surface.stride() as usize;
    {
        let mut data = surface.data().ok()?;
        match effect {
            Effect::Blur { sigma } => blur_argb32(&mut data, w, h, stride, sigma),
            Effect::Pixelate { block } => pixelate(&mut data, w, h, stride, block.max(1) as usize),
        }
    }
    surface.mark_dirty();
    Some(Processed {
        surface,
        rect: source,
    })
}

/// Replaces every `block` × `block` square (from the top-left) with its average (premultiplied
/// channels, so transparent pixels do not darken it).
fn pixelate(data: &mut [u8], width: usize, height: usize, stride: usize, block: usize) {
    for by in (0..height).step_by(block) {
        for bx in (0..width).step_by(block) {
            let (y1, x1) = ((by + block).min(height), (bx + block).min(width));
            let mut sum = [0u64; 4];
            let mut n = 0u64;
            for y in by..y1 {
                for x in bx..x1 {
                    let o = y * stride + x * 4;
                    if let Some(px) = data.get(o..o + 4) {
                        for (s, v) in sum.iter_mut().zip(px) {
                            *s += u64::from(*v);
                        }
                        n += 1;
                    }
                }
            }
            if n == 0 {
                continue;
            }
            let avg = sum.map(|s| ((s + n / 2) / n) as u8);
            for y in by..y1 {
                for x in bx..x1 {
                    let o = y * stride + x * 4;
                    if let Some(px) = data.get_mut(o..o + 4) {
                        px.copy_from_slice(&avg);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::color::{palette, Rgba};

    fn stripes() -> Bitmap {
        Bitmap::draw(64, 32, |cr| {
            for x in 0..64 {
                let c = if x % 2 == 0 {
                    palette::INK
                } else {
                    palette::PAPER
                };
                c.set_source(cr);
                cr.rectangle(f64::from(x), 0.0, 1.0, 32.0);
                cr.fill()?;
            }
            Ok(())
        })
        .unwrap()
    }

    fn pixel(p: &Processed, x: i32, y: i32) -> Rgba {
        let bmp = Bitmap::draw(p.rect.width, p.rect.height, |cr| {
            cr.set_source_surface(&p.surface, 0.0, 0.0)?;
            cr.paint()
        })
        .unwrap();
        bmp.pixel(x - p.rect.x, y - p.rect.y).unwrap()
    }

    #[test]
    fn pixelate_averages_blocks() {
        let p = processed(
            &stripes(),
            IRect::new(8, 8, 16, 16),
            Effect::Pixelate { block: 8 },
        )
        .unwrap();
        assert_eq!(p.rect, IRect::new(8, 8, 16, 16));
        // Ink and paper stripes average to one grey for the whole block.
        let a = pixel(&p, 8, 8);
        let b = pixel(&p, 15, 15);
        assert_eq!(a, b);
        assert!(a.r > 100 && a.r < 160, "{a:?}");
    }

    #[test]
    fn blur_reads_beyond_the_area_and_is_cached() {
        let image = stripes();
        let p = processed(
            &image,
            IRect::new(20, 4, 10, 10),
            Effect::Blur { sigma: 2.0 },
        )
        .unwrap();
        // 6 px of margin around (the image's edge pixels repeated above it).
        assert_eq!(p.rect, IRect::new(14, -2, 22, 22));
        let mid = pixel(&p, 25, 8);
        assert!(mid.r > 100 && mid.r < 160 && mid.a == 255, "{mid:?}");
        let again = processed(
            &image,
            IRect::new(20, 4, 10, 10),
            Effect::Blur { sigma: 2.0 },
        )
        .unwrap();
        assert_eq!(
            again.surface.to_raw_none(),
            p.surface.to_raw_none(),
            "cached"
        );
        assert!(processed(
            &image,
            IRect::new(100, 100, 5, 5),
            Effect::Blur { sigma: 2.0 }
        )
        .is_none());
    }
}
