//! Crop mode (tool `C`; spec editor-canvas: Image operations — "Crop"; mockup 08, redline
//! "Canvas"). Crop is a mode, not a dialog: a frame over the canvas with corner and edge grips,
//! dimmed outside, a rule-of-thirds grid inside and a size chip below; the sidebar shows its
//! X/Y/W/H, an aspect-ratio choice, Cancel (Esc) and Crop (Enter).
//!
//! [`Crop`] is that mode's state, in whole document pixels: the frame always stays inside the
//! canvas, at least one pixel large. Applying it is [`super::image_ops::crop`].

use crate::document::annotation::Compass;
use crate::document::geometry::{IRect, Point};
use crate::document::Document;
use crate::render::Viewport;

use super::image_ops;

/// Grab distance around a grip or a frame edge, in screen pixels.
pub const GRIP_REACH: f64 = 10.0;

/// The aspect-ratio choice of the crop sidebar (redline: Free, Original, 1:1, 4:3, 16:9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Aspect {
    #[default]
    Free,
    /// The image's own ratio.
    Original,
    Square,
    FourThree,
    SixteenNine,
}

impl Aspect {
    pub const ALL: [Aspect; 5] = [
        Aspect::Free,
        Aspect::Original,
        Aspect::Square,
        Aspect::FourThree,
        Aspect::SixteenNine,
    ];

    /// Width ÷ height, `None` for Free.
    pub fn ratio(self, doc: &Document) -> Option<f64> {
        match self {
            Aspect::Free => None,
            Aspect::Original => {
                let r = image_ops::image_rect(doc);
                Some(f64::from(r.width) / f64::from(r.height.max(1)))
            }
            Aspect::Square => Some(1.0),
            Aspect::FourThree => Some(4.0 / 3.0),
            Aspect::SixteenNine => Some(16.0 / 9.0),
        }
    }
}

/// What a press in crop mode grabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grip {
    /// A corner or an edge (N, E, S, W).
    Handle(Compass),
    /// Inside the frame: moves it.
    Inside,
}

#[derive(Clone, Copy, Debug)]
struct Drag {
    grip: Grip,
    start: IRect,
    start_doc: Point,
}

#[derive(Clone, Debug)]
pub struct Crop {
    /// The canvas: the frame stays inside it.
    bounds: IRect,
    frame: IRect,
    aspect: Aspect,
    ratio: Option<f64>,
    drag: Option<Drag>,
}

impl Crop {
    /// Crop mode on `doc`: the frame starts as the whole canvas.
    pub fn new(doc: &Document) -> Self {
        let bounds = doc.canvas().rect;
        Self {
            bounds,
            frame: bounds,
            aspect: Aspect::Free,
            ratio: None,
            drag: None,
        }
    }

    pub fn frame(&self) -> IRect {
        self.frame
    }

    pub fn aspect(&self) -> Aspect {
        self.aspect
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Sets the frame from the sidebar's X/Y/W/H: kept inside the canvas and, with a fixed
    /// ratio, given the height that matches the width.
    pub fn set_frame(&mut self, rect: IRect) {
        let mut r = rect;
        r.width = r.width.clamp(1, self.bounds.width);
        r.height = r.height.clamp(1, self.bounds.height);
        if let Some(ratio) = self.ratio {
            r.height = (f64::from(r.width) / ratio).round() as i32;
            if r.height > self.bounds.height {
                r.height = self.bounds.height;
                r.width = (f64::from(r.height) * ratio).round() as i32;
            }
            r.height = r.height.max(1);
            r.width = r.width.max(1);
        }
        r.x = r.x.clamp(self.bounds.x, self.bounds.right() - r.width);
        r.y = r.y.clamp(self.bounds.y, self.bounds.bottom() - r.height);
        self.frame = r;
    }

    /// Picks an aspect ratio: the frame becomes the largest one of that ratio inside the
    /// current frame, centred on it.
    pub fn set_aspect(&mut self, aspect: Aspect, doc: &Document) {
        self.aspect = aspect;
        self.ratio = aspect.ratio(doc);
        let Some(ratio) = self.ratio else {
            return;
        };
        let f = self.frame;
        let (w, h) = (f64::from(f.width), f64::from(f.height));
        let (nw, nh) = if w / h > ratio {
            (h * ratio, h)
        } else {
            (w, w / ratio)
        };
        let (nw, nh) = (nw.round().max(1.0) as i32, nh.round().max(1.0) as i32);
        self.frame = IRect::new(f.x + (f.width - nw) / 2, f.y + (f.height - nh) / 2, nw, nh);
    }

    /// The grip under `screen` (view pixels): corners first, then edges, then the inside.
    pub fn grip_at(&self, viewport: &Viewport, screen: Point) -> Option<Grip> {
        let r = self.frame.to_rect();
        let a = viewport.to_device(Point::new(r.x, r.y));
        let b = viewport.to_device(Point::new(r.right(), r.bottom()));
        let near = |v: f64, edge: f64| (v - edge).abs() <= GRIP_REACH;
        let within = |v: f64, lo: f64, hi: f64| v >= lo - GRIP_REACH && v <= hi + GRIP_REACH;
        let (x, y) = (screen.x, screen.y);
        let (left, right) = (near(x, a.x), near(x, b.x));
        let (top, bottom) = (near(y, a.y), near(y, b.y));
        let compass = match (left, right, top, bottom) {
            (true, _, true, _) => Some(Compass::NW),
            (_, true, true, _) => Some(Compass::NE),
            (true, _, _, true) => Some(Compass::SW),
            (_, true, _, true) => Some(Compass::SE),
            (true, _, _, _) if within(y, a.y, b.y) => Some(Compass::W),
            (_, true, _, _) if within(y, a.y, b.y) => Some(Compass::E),
            (_, _, true, _) if within(x, a.x, b.x) => Some(Compass::N),
            (_, _, _, true) if within(x, a.x, b.x) => Some(Compass::S),
            _ => None,
        };
        match compass {
            Some(c) => Some(Grip::Handle(c)),
            None if x > a.x && x < b.x && y > a.y && y < b.y => Some(Grip::Inside),
            None => None,
        }
    }

    /// Starts dragging what is under `screen`; returns whether anything was grabbed.
    pub fn press(&mut self, viewport: &Viewport, screen: Point) -> bool {
        self.drag = self.grip_at(viewport, screen).map(|grip| Drag {
            grip,
            start: self.frame,
            start_doc: viewport.to_document(screen),
        });
        self.drag.is_some()
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    /// The pointer moved to `screen` with the button held.
    pub fn motion(&mut self, viewport: &Viewport, screen: Point) {
        let Some(drag) = self.drag else {
            return;
        };
        let at = viewport.to_document(screen);
        self.frame = match drag.grip {
            Grip::Inside => {
                let s = drag.start;
                let dx = (at.x - drag.start_doc.x).round() as i32;
                let dy = (at.y - drag.start_doc.y).round() as i32;
                IRect::new(
                    (s.x + dx).clamp(self.bounds.x, self.bounds.right() - s.width),
                    (s.y + dy).clamp(self.bounds.y, self.bounds.bottom() - s.height),
                    s.width,
                    s.height,
                )
            }
            Grip::Handle(compass) => self.resized(drag.start, compass, at),
        };
    }

    /// `start` with its `compass` grip dragged to `at`; the opposite side stays.
    fn resized(&self, start: IRect, compass: Compass, at: Point) -> IRect {
        let b = self.bounds;
        let (sx, sy) = compass.direction();
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::from(start.x),
            f64::from(start.y),
            f64::from(start.right()),
            f64::from(start.bottom()),
        );
        let (bx0, by0, bx1, by1) = (
            f64::from(b.x),
            f64::from(b.y),
            f64::from(b.right()),
            f64::from(b.bottom()),
        );
        // Free: each moving side follows the pointer, at least 1 px from the other, inside.
        if sx > 0.0 {
            x1 = at.x.clamp(x0 + 1.0, bx1);
        } else if sx < 0.0 {
            x0 = at.x.clamp(bx0, x1 - 1.0);
        }
        if sy > 0.0 {
            y1 = at.y.clamp(y0 + 1.0, by1);
        } else if sy < 0.0 {
            y0 = at.y.clamp(by0, y1 - 1.0);
        }
        if let Some(ratio) = self.ratio {
            let (w, h) = (x1 - x0, y1 - y0);
            match (sx != 0.0, sy != 0.0) {
                // Corner: the larger change wins, limited by the room from the fixed corner.
                (true, true) => {
                    let room_w = if sx > 0.0 { bx1 - x0 } else { x1 - bx0 };
                    let room_h = if sy > 0.0 { by1 - y0 } else { y1 - by0 };
                    let nw = w.max(h * ratio).min(room_w).min(room_h * ratio).max(1.0);
                    let nh = nw / ratio;
                    if sx > 0.0 {
                        x1 = x0 + nw;
                    } else {
                        x0 = x1 - nw;
                    }
                    if sy > 0.0 {
                        y1 = y0 + nh;
                    } else {
                        y0 = y1 - nh;
                    }
                }
                // Edge: the other side follows, centred on the frame's centre line.
                (true, false) => {
                    let cy = (y0 + y1) / 2.0;
                    let room = 2.0 * (cy - by0).min(by1 - cy);
                    let nw = w.min(room * ratio).max(1.0);
                    if sx > 0.0 {
                        x1 = x0 + nw;
                    } else {
                        x0 = x1 - nw;
                    }
                    let nh = nw / ratio;
                    y0 = cy - nh / 2.0;
                    y1 = cy + nh / 2.0;
                }
                _ => {
                    let cx = (x0 + x1) / 2.0;
                    let room = 2.0 * (cx - bx0).min(bx1 - cx);
                    let nh = h.min(room / ratio).max(1.0);
                    if sy > 0.0 {
                        y1 = y0 + nh;
                    } else {
                        y0 = y1 - nh;
                    }
                    let nw = nh * ratio;
                    x0 = cx - nw / 2.0;
                    x1 = cx + nw / 2.0;
                }
            }
        }
        let (rx0, ry0) = (x0.round() as i32, y0.round() as i32);
        let (rx1, ry1) = (x1.round() as i32, y1.round() as i32);
        IRect::new(
            rx0.max(b.x),
            ry0.max(b.y),
            (rx1.min(b.right()) - rx0.max(b.x)).max(1),
            (ry1.min(b.bottom()) - ry0.max(b.y)).max(1),
        )
    }

    /// Whether Crop (Enter) would do something.
    pub fn can_apply(&self, doc: &Document) -> bool {
        image_ops::can_crop(doc, self.frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::bitmap::Bitmap;
    use crate::document::color::Rgba;

    fn doc() -> Document {
        Document::new(Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap())
    }

    /// Mockup 08: 50 %, document (0, 0) at view (44, 44).
    fn vp() -> Viewport {
        Viewport::new(0.5, Point::new(44.0, 44.0))
    }

    fn drag(crop: &mut Crop, from: Point, to: Point) {
        let vp = vp();
        assert!(crop.press(&vp, vp.to_device(from)));
        crop.motion(&vp, vp.to_device(to));
        crop.release();
    }

    #[test]
    fn frame_starts_as_the_canvas_and_grips_resize_it() {
        let doc = doc();
        let mut crop = Crop::new(&doc);
        assert_eq!(crop.frame(), IRect::new(0, 0, 1600, 1000));
        assert!(!crop.can_apply(&doc), "the whole canvas crops nothing");
        // Mockup 08: drag the corners to 10, 170 – 970, 970 → 960 × 800.
        drag(&mut crop, Point::new(0.0, 0.0), Point::new(10.0, 170.0));
        drag(
            &mut crop,
            Point::new(1600.0, 1000.0),
            Point::new(970.0, 970.0),
        );
        assert_eq!(crop.frame(), IRect::new(10, 170, 960, 800));
        assert!(crop.can_apply(&doc));
        // An edge moves one side only; the frame never leaves the canvas.
        drag(
            &mut crop,
            Point::new(970.0, 570.0),
            Point::new(3000.0, 570.0),
        );
        assert_eq!(crop.frame(), IRect::new(10, 170, 1590, 800));
        // Inside: the frame moves, clamped to the canvas.
        drag(&mut crop, Point::new(500.0, 500.0), Point::new(400.0, 0.0));
        assert_eq!(crop.frame(), IRect::new(0, 0, 1590, 800));
    }

    #[test]
    fn grips_are_found_in_screen_pixels() {
        let doc = doc();
        let crop = Crop::new(&doc);
        let vp = vp();
        let at = |p: Point| crop.grip_at(&vp, vp.to_device(p));
        assert_eq!(at(Point::new(0.0, 0.0)), Some(Grip::Handle(Compass::NW)));
        assert_eq!(
            at(Point::new(1600.0, 500.0)),
            Some(Grip::Handle(Compass::E))
        );
        assert_eq!(at(Point::new(800.0, 500.0)), Some(Grip::Inside));
        assert_eq!(at(Point::new(800.0, 1200.0)), None);
    }

    #[test]
    fn aspect_ratios_fit_inside_the_frame() {
        let doc = doc();
        let mut crop = Crop::new(&doc);
        crop.set_aspect(Aspect::Square, &doc);
        assert_eq!(crop.frame(), IRect::new(300, 0, 1000, 1000));
        // A corner drag keeps the ratio.
        drag(
            &mut crop,
            Point::new(1300.0, 1000.0),
            Point::new(900.0, 950.0),
        );
        let f = crop.frame();
        assert_eq!(f.width, f.height);
        assert_eq!((f.x, f.y), (300, 0));
        // An edge drag too, centred on the other axis.
        crop.set_aspect(Aspect::SixteenNine, &doc);
        let f = crop.frame();
        assert!((f64::from(f.width) / f64::from(f.height) - 16.0 / 9.0).abs() < 0.02);
        crop.set_aspect(Aspect::Original, &doc);
        assert!(crop.aspect() == Aspect::Original);
        // Typed values keep the ratio and stay inside.
        crop.set_aspect(Aspect::FourThree, &doc);
        crop.set_frame(IRect::new(1500, 900, 400, 1));
        assert_eq!(crop.frame(), IRect::new(1200, 700, 400, 300));
        crop.set_aspect(Aspect::Free, &doc);
        crop.set_frame(IRect::new(-50, 10, 100, 20));
        assert_eq!(crop.frame(), IRect::new(0, 10, 100, 20));
    }
}
