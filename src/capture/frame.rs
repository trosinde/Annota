//! A captured still image held in memory.
//!
//! Pixels are stored in cairo's `ARGB32` layout (premultiplied alpha, one native-endian `u32`
//! per pixel), which is also what `gdk::Texture::download` produces on little-endian machines.
//! This module is free of GTK so that it can be unit-tested without a display; conversion to
//! and from `gdk::Texture` lives in [`super::texture`].

use gtk::cairo;

use super::geometry::IRect;

/// Bytes per pixel of the `ARGB32` layout.
const BPP: usize = 4;

#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    width: i32,
    height: i32,
    stride: usize,
    data: Vec<u8>,
}

impl std::fmt::Debug for Frame {
    // Never dump pixel data into logs (CLAUDE.md §7: capture contents stay private).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// An opaque, un-premultiplied colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Rec. 601 luma, 0–255.
    pub fn luma(self) -> u8 {
        ((299 * u32::from(self.r) + 587 * u32::from(self.g) + 114 * u32::from(self.b)) / 1000) as u8
    }
}

impl Frame {
    /// Wraps raw `ARGB32` rows. Returns `None` if the buffer is too small for the geometry.
    pub fn from_argb32(width: i32, height: i32, stride: usize, data: Vec<u8>) -> Option<Self> {
        if width <= 0 || height <= 0 || stride < width as usize * BPP {
            return None;
        }
        if data.len() < stride * height as usize {
            return None;
        }
        Some(Self {
            width,
            height,
            stride,
            data,
        })
    }

    /// A frame filled with one opaque colour.
    #[cfg(test)]
    pub fn filled(width: i32, height: i32, color: Rgb) -> Option<Self> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let pixel = 0xFF00_0000u32
            | (u32::from(color.r) << 16)
            | (u32::from(color.g) << 8)
            | u32::from(color.b);
        let stride = width as usize * BPP;
        let mut data = Vec::with_capacity(stride * height as usize);
        for _ in 0..(width as usize * height as usize) {
            data.extend_from_slice(&pixel.to_ne_bytes());
        }
        Self::from_argb32(width, height, stride, data)
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn bounds(&self) -> IRect {
        IRect::new(0, 0, self.width, self.height)
    }

    /// The colour at `(x, y)`, un-premultiplied; `None` outside the frame.
    pub fn pixel(&self, x: i32, y: i32) -> Option<Rgb> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return None;
        }
        let offset = y as usize * self.stride + x as usize * BPP;
        let bytes: [u8; 4] = self.data.get(offset..offset + BPP)?.try_into().ok()?;
        let argb = u32::from_ne_bytes(bytes);
        let a = (argb >> 24) & 0xFF;
        let unpremultiply = |c: u32| -> u8 {
            if a == 0 {
                0
            } else {
                ((c * 255 + a / 2) / a).min(255) as u8
            }
        };
        Some(Rgb {
            r: unpremultiply((argb >> 16) & 0xFF),
            g: unpremultiply((argb >> 8) & 0xFF),
            b: unpremultiply(argb & 0xFF),
        })
    }

    /// Luma at `(x, y)`, 0 outside the frame.
    pub fn luma(&self, x: i32, y: i32) -> u8 {
        self.pixel(x, y).map(Rgb::luma).unwrap_or(0)
    }

    /// Copies the part of the frame inside `rect` (clipped to the frame).
    pub fn crop(&self, rect: IRect) -> Option<Frame> {
        let rect = rect.intersect(self.bounds())?;
        let row_len = rect.w as usize * BPP;
        let mut data = Vec::with_capacity(row_len * rect.h as usize);
        for y in rect.y..rect.bottom() {
            let start = y as usize * self.stride + rect.x as usize * BPP;
            data.extend_from_slice(self.data.get(start..start + row_len)?);
        }
        Frame::from_argb32(rect.w, rect.h, row_len, data)
    }

    /// A cairo surface holding a copy of the pixels, for drawing the frame on screen.
    pub fn to_surface(&self) -> Result<cairo::ImageSurface, cairo::Error> {
        cairo::ImageSurface::create_for_data(
            self.data.clone(),
            cairo::Format::ARgb32,
            self.width,
            self.height,
            self.stride as i32,
        )
    }

    /// Runs `draw` on a cairo context targeting this frame and keeps the result.
    pub fn draw_with(
        &mut self,
        draw: impl FnOnce(&cairo::Context) -> Result<(), cairo::Error>,
    ) -> Result<(), cairo::Error> {
        let surface = cairo::ImageSurface::create_for_data(
            std::mem::take(&mut self.data),
            cairo::Format::ARgb32,
            self.width,
            self.height,
            self.stride as i32,
        )?;
        let result = {
            let cr = cairo::Context::new(&surface)?;
            draw(&cr)
        };
        surface.flush();
        surface
            .with_data(|bytes| self.data = bytes.to_vec())
            .map_err(|_| cairo::Error::SurfaceFinished)?;
        result
    }

    /// Paints a mouse pointer with its hot spot at `(x, y)` (frame pixels). `scale` is the
    /// device scale of the captured monitor so the pointer has its on-screen size.
    ///
    /// The Screenshot portal has no "include pointer" option for non-interactive grabs, so
    /// Annota composites a standard arrow pointer itself (spec: Cursor inclusion).
    pub fn draw_pointer(&mut self, x: f64, y: f64, scale: f64) -> Result<(), cairo::Error> {
        self.draw_with(|cr| {
            cr.translate(x, y);
            cr.scale(scale, scale);
            pointer_path(cr);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.fill_preserve()?;
            cr.set_source_rgb(0.0, 0.0, 0.0);
            cr.set_line_width(1.2);
            cr.set_line_join(cairo::LineJoin::Round);
            cr.stroke()
        })
    }
}

/// Outline of a classic arrow pointer, hot spot at the origin, 22 logical px tall.
fn pointer_path(cr: &cairo::Context) {
    let points = [
        (0.0, 0.0),
        (0.0, 17.0),
        (4.2, 13.2),
        (7.0, 19.5),
        (9.8, 18.3),
        (7.1, 12.2),
        (12.6, 12.2),
    ];
    cr.new_path();
    for (i, (px, py)) in points.iter().enumerate() {
        if i == 0 {
            cr.move_to(*px, *py);
        } else {
            cr.line_to(*px, *py);
        }
    }
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREY: Rgb = Rgb {
        r: 0x80,
        g: 0x80,
        b: 0x80,
    };

    #[test]
    fn filled_frame_reports_its_colour() {
        let frame = Frame::filled(4, 3, GREY).unwrap();
        assert_eq!(frame.pixel(0, 0), Some(GREY));
        assert_eq!(frame.pixel(3, 2), Some(GREY));
        assert_eq!(frame.pixel(4, 0), None);
        assert_eq!(frame.pixel(-1, 0), None);
        assert_eq!(GREY.hex(), "#808080");
    }

    #[test]
    fn crop_has_exact_size_and_content() {
        let mut frame = Frame::filled(1000, 800, GREY).unwrap();
        frame
            .draw_with(|cr| {
                cr.set_source_rgb(1.0, 0.0, 0.0);
                cr.rectangle(100.0, 50.0, 1.0, 1.0);
                cr.fill()
            })
            .unwrap();
        let crop = frame.crop(IRect::new(100, 50, 400, 300)).unwrap();
        assert_eq!((crop.width(), crop.height()), (400, 300));
        assert_eq!(crop.pixel(0, 0), Some(Rgb { r: 255, g: 0, b: 0 }));
        assert_eq!(crop.pixel(1, 0), Some(GREY));
    }

    #[test]
    fn crop_is_clipped_to_the_frame() {
        let frame = Frame::filled(100, 100, GREY).unwrap();
        let crop = frame.crop(IRect::new(80, 90, 50, 50)).unwrap();
        assert_eq!((crop.width(), crop.height()), (20, 10));
        assert!(frame.crop(IRect::new(200, 200, 10, 10)).is_none());
    }

    #[test]
    fn pointer_is_visible_after_drawing() {
        // Scenario "Include cursor": the pointer is visible in the image.
        let mut frame = Frame::filled(200, 200, GREY).unwrap();
        frame.draw_pointer(50.0, 60.0, 1.0).unwrap();
        // Inside the arrow body: white fill.
        assert_eq!(
            frame.pixel(53, 70),
            Some(Rgb {
                r: 255,
                g: 255,
                b: 255
            })
        );
        // Far from the pointer: untouched.
        assert_eq!(frame.pixel(150, 150), Some(GREY));
    }

    #[test]
    fn frame_without_pointer_is_untouched() {
        let frame = Frame::filled(200, 200, GREY).unwrap();
        assert_eq!(frame.pixel(53, 70), Some(GREY));
    }

    #[test]
    fn rejects_short_buffers() {
        assert!(Frame::from_argb32(10, 10, 40, vec![0; 39]).is_none());
        assert!(Frame::from_argb32(10, 10, 30, vec![0; 400]).is_none());
        assert!(Frame::from_argb32(0, 10, 40, vec![0; 400]).is_none());
    }
}
