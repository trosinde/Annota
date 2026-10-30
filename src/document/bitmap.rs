//! Immutable, shared raster images: the document's original image and embedded images.
//!
//! A [`Bitmap`] wraps a cairo image surface (drawn directly by the renderer, no per-frame copy)
//! and caches its PNG encoding, so saving and autosaving do not re-encode unchanged pixels.
//! Cloning is cheap (reference counted), which lets undo history and image objects share one
//! copy of the pixels. Bitmaps are never drawn into after creation.

use std::cell::OnceCell;
use std::fmt;
use std::io::Cursor;
use std::rc::Rc;

use super::color::Rgba;

/// cairo's limit for image surfaces in either direction.
pub const MAX_SIDE: i32 = 32_767;
/// Refuse images above this many pixels (1 GiB of ARGB32) before decoding them.
pub const MAX_PIXELS: u64 = 1 << 28;

#[derive(Debug, thiserror::Error)]
pub enum BitmapError {
    #[error("image size {0} × {1} is not supported")]
    Size(i64, i64),
    #[error("pixel buffer is too small for the image size")]
    ShortBuffer,
    #[error("not a PNG image")]
    NotPng,
    #[error("PNG error: {0}")]
    Png(String),
    #[error("cairo error: {0}")]
    Cairo(#[from] cairo::Error),
}

#[derive(Clone)]
pub struct Bitmap(Rc<Inner>);

struct Inner {
    surface: cairo::ImageSurface,
    png: OnceCell<Rc<[u8]>>,
}

impl fmt::Debug for Bitmap {
    // Never dump pixel data (CLAUDE.md §7: capture contents stay private).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bitmap")
            .field("width", &self.width())
            .field("height", &self.height())
            .finish_non_exhaustive()
    }
}

fn check_size(width: i64, height: i64) -> Result<(), BitmapError> {
    let max = i64::from(MAX_SIDE);
    if width <= 0 || height <= 0 || width > max || height > max {
        return Err(BitmapError::Size(width, height));
    }
    if (width as u64) * (height as u64) > MAX_PIXELS {
        return Err(BitmapError::Size(width, height));
    }
    Ok(())
}

impl Bitmap {
    /// Takes over rows in cairo's `ARGB32` layout (premultiplied, native-endian `u32`), the
    /// layout of `capture::frame::Frame`.
    pub fn from_argb32(
        width: i32,
        height: i32,
        stride: usize,
        data: Vec<u8>,
    ) -> Result<Self, BitmapError> {
        check_size(i64::from(width), i64::from(height))?;
        if stride < width as usize * 4 || data.len() < stride * height as usize {
            return Err(BitmapError::ShortBuffer);
        }
        let surface = cairo::ImageSurface::create_for_data(
            data,
            cairo::Format::ARgb32,
            width,
            height,
            stride as i32,
        )?;
        Ok(Self::wrap(surface, None))
    }

    /// A bitmap of one colour (tests, placeholders).
    pub fn filled(width: i32, height: i32, color: Rgba) -> Result<Self, BitmapError> {
        check_size(i64::from(width), i64::from(height))?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
        {
            let cr = cairo::Context::new(&surface)?;
            cr.set_operator(cairo::Operator::Source);
            color.set_source(&cr);
            cr.paint()?;
        }
        surface.flush();
        Ok(Self::wrap(surface, None))
    }

    /// Draws a new bitmap with `draw` (tests, renderer output).
    pub fn draw(
        width: i32,
        height: i32,
        draw: impl FnOnce(&cairo::Context) -> Result<(), cairo::Error>,
    ) -> Result<Self, BitmapError> {
        check_size(i64::from(width), i64::from(height))?;
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;
        {
            let cr = cairo::Context::new(&surface)?;
            draw(&cr)?;
        }
        surface.flush();
        Ok(Self::wrap(surface, None))
    }

    /// Decodes PNG data and keeps it as the cached encoding.
    pub fn from_png(bytes: Vec<u8>) -> Result<Self, BitmapError> {
        let (width, height) = png_size(&bytes).ok_or(BitmapError::NotPng)?;
        check_size(i64::from(width), i64::from(height))?;
        let surface = cairo::ImageSurface::create_from_png(&mut Cursor::new(&bytes))
            .map_err(|err| BitmapError::Png(err.to_string()))?;
        Ok(Self::wrap(surface, Some(Rc::from(bytes))))
    }

    fn wrap(surface: cairo::ImageSurface, png: Option<Rc<[u8]>>) -> Self {
        let cell = OnceCell::new();
        if let Some(png) = png {
            let _ = cell.set(png);
        }
        Self(Rc::new(Inner { surface, png: cell }))
    }

    pub fn width(&self) -> i32 {
        self.0.surface.width()
    }

    pub fn height(&self) -> i32 {
        self.0.surface.height()
    }

    /// The surface to paint from. Never draw into it: bitmaps are shared and immutable.
    pub fn surface(&self) -> &cairo::ImageSurface {
        &self.0.surface
    }

    /// The PNG encoding, computed once.
    pub fn png(&self) -> Result<Rc<[u8]>, BitmapError> {
        if let Some(png) = self.0.png.get() {
            return Ok(png.clone());
        }
        let mut out = Vec::new();
        self.0
            .surface
            .write_to_png(&mut out)
            .map_err(|err| BitmapError::Png(err.to_string()))?;
        let png: Rc<[u8]> = Rc::from(out);
        let _ = self.0.png.set(png.clone());
        Ok(png)
    }

    /// The straight (un-premultiplied) colour at `(x, y)`; `None` outside the bitmap.
    pub fn pixel(&self, x: i32, y: i32) -> Option<Rgba> {
        if x < 0 || y < 0 || x >= self.width() || y >= self.height() {
            return None;
        }
        let surface = &self.0.surface;
        let opaque = surface.format() == cairo::Format::Rgb24;
        let offset = y as usize * surface.stride() as usize + x as usize * 4;
        let mut argb = None;
        surface
            .with_data(|data| {
                argb = data
                    .get(offset..offset + 4)
                    .and_then(|b| <[u8; 4]>::try_from(b).ok())
                    .map(u32::from_ne_bytes);
            })
            .ok()?;
        let argb = argb?;
        let a = if opaque { 255 } else { (argb >> 24) & 0xFF };
        let un = |c: u32| -> u8 {
            if a == 0 {
                0
            } else {
                ((c * 255 + a / 2) / a).min(255) as u8
            }
        };
        Some(Rgba::new(
            un((argb >> 16) & 0xFF),
            un((argb >> 8) & 0xFF),
            un(argb & 0xFF),
            a as u8,
        ))
    }

    /// True if both bitmaps share the same pixels (not a pixel comparison).
    pub fn ptr_eq(&self, other: &Bitmap) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// Width and height from a PNG's IHDR chunk, read before decoding so oversized images are
/// refused without allocating them.
pub fn png_size(bytes: &[u8]) -> Option<(i32, i32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.get(0..8)? != SIGNATURE || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let read = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    let width = i32::try_from(read(16)?).ok()?;
    let height = i32::try_from(read(20)?).ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip_keeps_pixels() {
        let red = Rgba::opaque(255, 0, 0);
        let bitmap = Bitmap::draw(20, 10, |cr| {
            cr.set_source_rgb(0.0, 0.0, 1.0);
            cr.paint()?;
            red.set_source(cr);
            cr.rectangle(0.0, 0.0, 5.0, 5.0);
            cr.fill()
        })
        .unwrap();
        let png = bitmap.png().unwrap();
        assert_eq!(png_size(&png), Some((20, 10)));
        let back = Bitmap::from_png(png.to_vec()).unwrap();
        assert_eq!((back.width(), back.height()), (20, 10));
        assert_eq!(back.pixel(1, 1), Some(red));
        assert_eq!(back.pixel(10, 8), Some(Rgba::opaque(0, 0, 255)));
        assert_eq!(back.pixel(20, 0), None);
        // The decoded bitmap keeps the original bytes instead of re-encoding.
        assert_eq!(&*back.png().unwrap(), &*png);
    }

    #[test]
    fn translucent_pixels_are_unpremultiplied() {
        let c = Rgba::new(200, 100, 50, 128);
        let bitmap = Bitmap::filled(2, 2, c).unwrap();
        let p = bitmap.pixel(0, 0).unwrap();
        assert_eq!(p.a, 128);
        assert!((i32::from(p.r) - 200).abs() <= 1);
        assert!((i32::from(p.g) - 100).abs() <= 1);
    }

    #[test]
    fn argb32_buffers_are_validated() {
        assert!(Bitmap::from_argb32(2, 2, 8, vec![0; 16]).is_ok());
        assert!(matches!(
            Bitmap::from_argb32(2, 2, 8, vec![0; 15]),
            Err(BitmapError::ShortBuffer)
        ));
        assert!(matches!(
            Bitmap::from_argb32(0, 2, 8, vec![0; 16]),
            Err(BitmapError::Size(..))
        ));
    }

    #[test]
    fn oversized_png_is_refused_before_decoding() {
        // A forged header claiming 40000 × 40000 px; no pixel data follows.
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&40_000u32.to_be_bytes());
        bytes.extend_from_slice(&40_000u32.to_be_bytes());
        assert!(matches!(
            Bitmap::from_png(bytes),
            Err(BitmapError::Size(40_000, 40_000))
        ));
        assert!(matches!(
            Bitmap::from_png(b"GIF89a".to_vec()),
            Err(BitmapError::NotPng)
        ));
    }
}
