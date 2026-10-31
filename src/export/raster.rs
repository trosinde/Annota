//! PNG, JPG and WebP encoding of a rendered [`Bitmap`].
//!
//! The bitmap is cairo `ARGB32` (premultiplied, native-endian). PNG goes through cairo's own
//! writer ([`Bitmap::png`]); JPG and WebP get straight 8-bit RGBA converted with the same
//! rounding as [`Bitmap::pixel`].

use super::{check_quality, ExportError, WebPCompression};
use crate::document::bitmap::Bitmap;

/// Largest width or height of a WebP image (libwebp's `WEBP_MAX_DIMENSION`).
pub const WEBP_MAX_SIDE: i32 = 16_383;

/// PNG: lossless, with alpha.
pub fn encode_png(bitmap: &Bitmap) -> Result<Vec<u8>, ExportError> {
    Ok(bitmap.png()?.to_vec())
}

/// JPG at `quality` 1–100. JPG has no alpha: translucent pixels are composited onto white.
/// Chroma is not subsampled (4:4:4) at any quality, so thin coloured strokes and red text on
/// screenshots keep sharp edges.
pub fn encode_jpeg(bitmap: &Bitmap, quality: u8) -> Result<Vec<u8>, ExportError> {
    let quality = check_quality(quality)?;
    let (width, height) = (bitmap.width(), bitmap.height());
    let too_large = || ExportError::TooLarge {
        format: "JPG",
        max: i32::from(u16::MAX),
        width,
        height,
    };
    let w = u16::try_from(width).map_err(|_| too_large())?;
    let h = u16::try_from(height).map_err(|_| too_large())?;
    let rgb = rgb_on_white(bitmap)?;
    let mut out = Vec::new();
    let mut encoder = jpeg_encoder::Encoder::new(&mut out, quality);
    encoder.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    encoder.set_optimized_huffman_tables(true);
    encoder
        .encode(&rgb, w, h, jpeg_encoder::ColorType::Rgb)
        .map_err(|err| ExportError::Encoder {
            format: "JPG",
            reason: err.to_string(),
        })?;
    Ok(out)
}

/// WebP, lossless or lossy, with alpha (lossy WebP keeps alpha lossless).
pub fn encode_webp(bitmap: &Bitmap, compression: WebPCompression) -> Result<Vec<u8>, ExportError> {
    if let WebPCompression::Lossy { quality } = compression {
        check_quality(quality)?;
    }
    let (width, height) = (bitmap.width(), bitmap.height());
    if width > WEBP_MAX_SIDE || height > WEBP_MAX_SIDE {
        return Err(ExportError::TooLarge {
            format: "WebP",
            max: WEBP_MAX_SIDE,
            width,
            height,
        });
    }
    let rgba = straight_rgba(bitmap)?;
    webp_encode(&rgba, width, height, compression).ok_or_else(|| ExportError::Encoder {
        format: "WebP",
        reason: "libwebp could not encode the image".into(),
    })
}

/// libwebp's one-call encoders. `None` if libwebp reports a failure (it returns no data).
fn webp_encode(
    rgba: &[u8],
    width: i32,
    height: i32,
    compression: WebPCompression,
) -> Option<Vec<u8>> {
    let stride = width.checked_mul(4)?;
    let needed = usize::try_from(stride)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?;
    if width <= 0 || height <= 0 || rgba.len() < needed {
        return None;
    }
    let mut output: *mut u8 = std::ptr::null_mut();
    // SAFETY: `rgba` holds `height` rows of `stride` bytes (checked above) and outlives the
    // call; libwebp only reads it. On success libwebp stores a buffer of the returned size in
    // `output`, which is copied and then released with `WebPFree`, its matching deallocator.
    let size = unsafe {
        match compression {
            WebPCompression::Lossless => libwebp_sys::WebPEncodeLosslessRGBA(
                rgba.as_ptr(),
                width,
                height,
                stride,
                &mut output,
            ),
            WebPCompression::Lossy { quality } => libwebp_sys::WebPEncodeRGBA(
                rgba.as_ptr(),
                width,
                height,
                stride,
                f32::from(quality),
                &mut output,
            ),
        }
    };
    if output.is_null() {
        return None;
    }
    // SAFETY: see above; `output` is non-null and points to `size` bytes owned by libwebp.
    let bytes = (size > 0).then(|| unsafe { std::slice::from_raw_parts(output, size) }.to_vec());
    // SAFETY: `output` was allocated by libwebp and is not used after this.
    unsafe { libwebp_sys::WebPFree(output.cast()) };
    bytes
}

/// Calls `f` with each pixel of `bitmap` as premultiplied `[r, g, b, a]`, row by row.
fn for_each_pixel(bitmap: &Bitmap, mut f: impl FnMut([u8; 4])) -> Result<(), ExportError> {
    let surface = bitmap.surface();
    let opaque = surface.format() == cairo::Format::Rgb24;
    let (width, height) = (bitmap.width() as usize, bitmap.height() as usize);
    let stride = surface.stride() as usize;
    let mut short = false;
    surface
        .with_data(|data| {
            for y in 0..height {
                let Some(row) = data.get(y * stride..y * stride + width * 4) else {
                    short = true;
                    return;
                };
                for px in row.chunks_exact(4) {
                    let argb = u32::from_ne_bytes([px[0], px[1], px[2], px[3]]);
                    let a = if opaque { 255 } else { (argb >> 24) as u8 };
                    f([(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, a]);
                }
            }
        })
        .map_err(|err| match err {
            cairo::BorrowError::Cairo(err) => ExportError::Cairo(err),
            other => ExportError::Encoder {
                format: "image",
                reason: other.to_string(),
            },
        })?;
    if short {
        return Err(ExportError::Render(
            crate::document::bitmap::BitmapError::ShortBuffer,
        ));
    }
    Ok(())
}

/// Straight (un-premultiplied) RGBA, 4 bytes per pixel, rows without padding.
pub fn straight_rgba(bitmap: &Bitmap) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::with_capacity(bitmap.width() as usize * bitmap.height() as usize * 4);
    for_each_pixel(bitmap, |[r, g, b, a]| {
        let a32 = u32::from(a);
        let un = |c: u8| -> u8 {
            if a32 == 0 {
                0
            } else {
                ((u32::from(c) * 255 + a32 / 2) / a32).min(255) as u8
            }
        };
        out.extend_from_slice(&[un(r), un(g), un(b), a]);
    })?;
    Ok(out)
}

/// RGB composited onto white, 3 bytes per pixel. With premultiplied channels "over white" is
/// exact: `c + (255 − a)`.
pub fn rgb_on_white(bitmap: &Bitmap) -> Result<Vec<u8>, ExportError> {
    let mut out = Vec::with_capacity(bitmap.width() as usize * bitmap.height() as usize * 3);
    for_each_pixel(bitmap, |[r, g, b, a]| {
        let white = 255 - a;
        out.extend_from_slice(&[
            r.saturating_add(white),
            g.saturating_add(white),
            b.saturating_add(white),
        ]);
    })?;
    Ok(out)
}
