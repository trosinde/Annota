//! Pure geometry of the capture overlay: the selection rectangle and its handles, keyboard
//! nudging, fixed sizes, placement of the overlay's floating chips, and the mapping between
//! monitors (logical layout) and the pixels of the portal screenshot.
//!
//! Selection coordinates are pixels of the captured monitor image ("image px"); the overlay
//! converts them to widget coordinates by dividing by the monitor's capture scale.

/// Gap between pointer and loupe (redline 02).
pub const LOUPE_OFFSET: f64 = 26.0;
/// Loupe diameter (redline 02).
pub const LOUPE_DIAMETER: f64 = 136.0;
/// Loupe zoom factor: 17 × 17 image px fill the 136 px lens.
pub const LOUPE_ZOOM: f64 = 8.0;
/// Gap between selection and size chip (redline 02).
pub const CHIP_GAP: f64 = 6.0;
/// Gap between selection and toolbar (redline 03).
pub const TOOLBAR_GAP: f64 = 14.0;
/// Handle diameter (redline 02).
pub const HANDLE_DIAMETER: f64 = 12.0;
/// Space kept free at the bottom edge for the hint bar.
pub const HINT_RESERVE: f64 = 80.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// Normalised rectangle spanned by two corner points.
    pub fn from_corners(a: (i32, i32), b: (i32, i32)) -> Self {
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// Where a pointer at `logical` (this monitor's logical px) is drawn inside this rectangle,
    /// in image px relative to its origin — or `None` when its hot spot lies outside.
    ///
    /// The hot spot alone decides. The arrow extends right and down from it, so a hot spot just
    /// inside the lower-right corner still draws and is almost entirely clipped; requiring the
    /// whole arrow to fit would suppress legible pointers, and nudging it inwards would draw it
    /// where it never was.
    ///
    /// The capture (`capture::region`) and the overlay's preview both go through this, so what
    /// the preview promises and what the image contains cannot disagree.
    pub fn pointer_inside(&self, logical: (f64, f64), scale: f64) -> Option<(f64, f64)> {
        let (x, y) = (logical.0 * scale, logical.1 * scale);
        self.contains(x as i32, y as i32)
            .then(|| (x - f64::from(self.x), y - f64::from(self.y)))
    }

    pub fn intersect(&self, other: IRect) -> Option<IRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        (x1 > x0 && y1 > y0).then(|| IRect::new(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn union(&self, other: IRect) -> IRect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        IRect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Moves (and if necessary shrinks) the rectangle so it lies inside `bounds`.
    pub fn fit_inside(&self, bounds: IRect) -> IRect {
        let w = self.w.min(bounds.w);
        let h = self.h.min(bounds.h);
        let x = self.x.clamp(bounds.x, bounds.right() - w);
        let y = self.y.clamp(bounds.y, bounds.bottom() - h);
        IRect::new(x, y, w, h)
    }
}

/// The eight resize handles shown after the mouse button is released (redline 02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::NorthWest,
        Handle::North,
        Handle::NorthEast,
        Handle::East,
        Handle::SouthEast,
        Handle::South,
        Handle::SouthWest,
        Handle::West,
    ];

    /// Centre of the handle on `rect`, in the same units as `rect`.
    pub fn position(self, rect: IRect) -> (i32, i32) {
        let cx = rect.x + rect.w / 2;
        let cy = rect.y + rect.h / 2;
        match self {
            Handle::NorthWest => (rect.x, rect.y),
            Handle::North => (cx, rect.y),
            Handle::NorthEast => (rect.right(), rect.y),
            Handle::East => (rect.right(), cy),
            Handle::SouthEast => (rect.right(), rect.bottom()),
            Handle::South => (cx, rect.bottom()),
            Handle::SouthWest => (rect.x, rect.bottom()),
            Handle::West => (rect.x, cy),
        }
    }

    /// CSS cursor name for hovering this handle.
    pub fn cursor_name(self) -> &'static str {
        match self {
            Handle::NorthWest => "nw-resize",
            Handle::North => "n-resize",
            Handle::NorthEast => "ne-resize",
            Handle::East => "e-resize",
            Handle::SouthEast => "se-resize",
            Handle::South => "s-resize",
            Handle::SouthWest => "sw-resize",
            Handle::West => "w-resize",
        }
    }
}

/// Arrow-key direction for nudging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Nudge step: 1 px, 10 px with Shift (spec: Region capture).
pub fn nudge_step(shift: bool) -> i32 {
    if shift {
        10
    } else {
        1
    }
}

/// The region being selected on one monitor, in image px.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    bounds: IRect,
    rect: Option<IRect>,
    anchor: (i32, i32),
}

impl Selection {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            bounds: IRect::new(0, 0, width.max(1), height.max(1)),
            rect: None,
            anchor: (0, 0),
        }
    }

    /// The current selection, if it has a non-zero area.
    pub fn rect(&self) -> Option<IRect> {
        self.rect.filter(|r| !r.is_empty())
    }

    pub fn clear(&mut self) {
        self.rect = None;
    }

    fn clamp_point(&self, p: (i32, i32)) -> (i32, i32) {
        (
            p.0.clamp(self.bounds.x, self.bounds.right()),
            p.1.clamp(self.bounds.y, self.bounds.bottom()),
        )
    }

    /// Starts a new rubber-band selection at `p`.
    pub fn begin(&mut self, p: (i32, i32)) {
        self.anchor = self.clamp_point(p);
        self.rect = Some(IRect::new(self.anchor.0, self.anchor.1, 0, 0));
    }

    /// Updates the rubber band to end at `p`. With `square`, the shorter side follows the
    /// longer one (Shift while dragging).
    pub fn drag_to(&mut self, p: (i32, i32), square: bool) {
        let mut p = self.clamp_point(p);
        if square {
            let dx = p.0 - self.anchor.0;
            let dy = p.1 - self.anchor.1;
            let side = dx.abs().max(dy.abs());
            let max_x = if dx < 0 {
                self.anchor.0 - self.bounds.x
            } else {
                self.bounds.right() - self.anchor.0
            };
            let max_y = if dy < 0 {
                self.anchor.1 - self.bounds.y
            } else {
                self.bounds.bottom() - self.anchor.1
            };
            let side = side.min(max_x).min(max_y);
            let sign = |d: i32| if d < 0 { -1 } else { 1 };
            p = (
                self.anchor.0 + side * sign(dx),
                self.anchor.1 + side * sign(dy),
            );
        }
        self.rect = Some(IRect::from_corners(self.anchor, p));
    }

    /// Moves the whole selection (and the rubber-band anchor) by a delta, staying on screen.
    pub fn move_by(&mut self, dx: i32, dy: i32) {
        if let Some(rect) = self.rect {
            let moved =
                IRect::new(rect.x + dx, rect.y + dy, rect.w, rect.h).fit_inside(self.bounds);
            self.anchor.0 += moved.x - rect.x;
            self.anchor.1 += moved.y - rect.y;
            self.rect = Some(moved);
        }
    }

    /// Arrow-key nudging: 1 px, or 10 px with Shift.
    pub fn nudge(&mut self, direction: Direction, shift: bool) {
        let step = nudge_step(shift);
        match direction {
            Direction::Left => self.move_by(-step, 0),
            Direction::Right => self.move_by(step, 0),
            Direction::Up => self.move_by(0, -step),
            Direction::Down => self.move_by(0, step),
        }
    }

    /// Drags `handle` of the current selection to `p`.
    pub fn resize(&mut self, handle: Handle, p: (i32, i32)) {
        let Some(rect) = self.rect else {
            return;
        };
        let p = self.clamp_point(p);
        let (mut l, mut t, mut r, mut b) = (rect.x, rect.y, rect.right(), rect.bottom());
        match handle {
            Handle::NorthWest => (l, t) = p,
            Handle::North => t = p.1,
            Handle::NorthEast => (r, t) = p,
            Handle::East => r = p.0,
            Handle::SouthEast => (r, b) = p,
            Handle::South => b = p.1,
            Handle::SouthWest => (l, b) = p,
            Handle::West => l = p.0,
        }
        self.rect = Some(IRect::from_corners((l, t), (r, b)));
    }

    /// Sets the selection to exactly `w × h`, keeping the top-left corner where possible
    /// (spec: Fixed size). Sizes larger than the monitor are limited to the monitor.
    pub fn set_size(&mut self, w: i32, h: i32) {
        let (x, y) = self
            .rect
            .map(|r| (r.x, r.y))
            .unwrap_or((self.bounds.x, self.bounds.y));
        let rect = IRect::new(x, y, w.max(1), h.max(1)).fit_inside(self.bounds);
        self.anchor = (rect.x, rect.y);
        self.rect = Some(rect);
    }

    pub fn set_rect(&mut self, rect: IRect) {
        let rect = rect.fit_inside(self.bounds);
        self.anchor = (rect.x, rect.y);
        self.rect = Some(rect);
    }

    /// Selects the whole monitor.
    pub fn select_all(&mut self) {
        self.set_rect(self.bounds);
    }

    /// The handle under `p`, within `radius` image px of its centre.
    pub fn handle_at(&self, p: (i32, i32), radius: i32) -> Option<Handle> {
        let rect = self.rect()?;
        Handle::ALL.into_iter().find(|h| {
            let (hx, hy) = h.position(rect);
            (hx - p.0).abs() <= radius && (hy - p.1).abs() <= radius
        })
    }

    pub fn contains(&self, p: (i32, i32)) -> bool {
        self.rect().is_some_and(|r| r.contains(p.0, p.1))
    }
}

/// A rectangle in widget coordinates (floating point, logical px).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl FRect {
    pub fn from_image(rect: IRect, scale: f64) -> Self {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        Self {
            x: f64::from(rect.x) / scale,
            y: f64::from(rect.y) / scale,
            w: f64::from(rect.w) / scale,
            h: f64::from(rect.h) / scale,
        }
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

fn clamp_span(pos: f64, len: f64, area: f64) -> f64 {
    pos.min(area - len).max(0.0)
}

/// Size chip: above the selection's top-left corner with a 6 px gap; inside the selection
/// when there is no room above.
pub fn size_chip_position(sel: FRect, chip: (f64, f64), area: (f64, f64)) -> (f64, f64) {
    let above = sel.y - CHIP_GAP - chip.1;
    let y = if above >= 0.0 {
        above
    } else {
        sel.y + CHIP_GAP
    };
    (
        clamp_span(sel.x, chip.0, area.0),
        clamp_span(y, chip.1, area.1),
    )
}

/// Floating toolbar: 14 px below the selection, left-aligned with it; above the selection
/// when it would cover the hint bar, and inside the selection if neither fits.
pub fn toolbar_position(sel: FRect, toolbar: (f64, f64), area: (f64, f64)) -> (f64, f64) {
    let below = sel.bottom() + TOOLBAR_GAP;
    let above = sel.y - TOOLBAR_GAP - toolbar.1;
    let y = if below + toolbar.1 <= area.1 - HINT_RESERVE {
        below
    } else if above >= 0.0 {
        above
    } else {
        sel.bottom() - TOOLBAR_GAP - toolbar.1
    };
    (
        clamp_span(sel.x, toolbar.0, area.0),
        clamp_span(y, toolbar.1, area.1),
    )
}

/// Top-left of the loupe: 26 px right of and below the pointer, flipping to the other side
/// of the pointer near the right or bottom edge. `extra_below` reserves room for the colour
/// chip under the lens.
pub fn loupe_position(pointer: (f64, f64), area: (f64, f64), extra_below: f64) -> (f64, f64) {
    let mut x = pointer.0 + LOUPE_OFFSET;
    if x + LOUPE_DIAMETER > area.0 {
        x = pointer.0 - LOUPE_OFFSET - LOUPE_DIAMETER;
    }
    let mut y = pointer.1 + LOUPE_OFFSET;
    if y + LOUPE_DIAMETER + extra_below > area.1 {
        y = pointer.1 - LOUPE_OFFSET - LOUPE_DIAMETER - extra_below;
    }
    (x.max(0.0), y.max(0.0))
}

/// Gap between the pointer's hot spot and the preview chip (redlines, "Pointer chip").
pub const POINTER_CHIP_GAP: f64 = 30.0;

/// The pointer preview chip: `POINTER_CHIP_GAP` right of the hot spot, vertically centred on it,
/// flipped to the left when it would leave the monitor.
///
/// Centring on the hot spot is what keeps it clear of the loupe, which starts at
/// `LOUPE_OFFSET` below and right of the same point: the chip's band always ends above it.
pub fn pointer_chip_position(
    pointer: (f64, f64),
    size: (f64, f64),
    area: (f64, f64),
) -> (f64, f64) {
    let right = pointer.0 + POINTER_CHIP_GAP;
    let x = if right + size.0 <= area.0 {
        right
    } else {
        pointer.0 - POINTER_CHIP_GAP - size.0
    };
    (
        x.clamp(0.0, (area.0 - size.0).max(0.0)),
        (pointer.1 - size.1 / 2.0).clamp(0.0, (area.1 - size.1).max(0.0)),
    )
}

/// Bounding box of all monitors in the logical layout.
pub fn bounding_box(monitors: &[IRect]) -> Option<IRect> {
    monitors.iter().copied().reduce(|a, b| a.union(b))
}

/// Ratio between screenshot pixels and logical layout pixels (2.0 on a HiDPI desktop).
pub fn image_scale(image_width: i32, layout: IRect) -> f64 {
    if layout.w <= 0 || image_width <= 0 {
        1.0
    } else {
        f64::from(image_width) / f64::from(layout.w)
    }
}

/// The part of the full-desktop screenshot that shows `monitor`.
pub fn monitor_crop(monitor: IRect, layout: IRect, scale: f64) -> IRect {
    let px = |v: i32| (f64::from(v) * scale).round() as i32;
    let x0 = px(monitor.x - layout.x);
    let y0 = px(monitor.y - layout.y);
    let x1 = px(monitor.right() - layout.x);
    let y1 = px(monitor.bottom() - layout.y);
    IRect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Screen mode (spec: Full-screen capture): the part of the full-desktop image to keep —
/// the monitor under the pointer, the first monitor if the pointer position is unknown, or
/// everything with `capture-all-monitors`.
pub fn screen_capture_rect(
    crops: &[IRect],
    pointer_monitor: Option<usize>,
    all_monitors: bool,
    full: IRect,
) -> IRect {
    if all_monitors {
        return full;
    }
    pointer_monitor
        .and_then(|index| crops.get(index))
        .or_else(|| crops.first())
        .copied()
        .unwrap_or(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointer_chip_sits_beside_the_hot_spot_and_clears_the_loupe() {
        // Redline: 30 px right of the hot spot, vertically centred on it. The loupe starts at
        // +26 / +26 from the same point, so the chip's band always ends above it.
        let area = (1920.0, 1080.0);
        let size = (180.0, 26.0);
        let (x, y) = pointer_chip_position((400.0, 500.0), size, area);
        assert_eq!((x, y), (430.0, 487.0));
        let (_, loupe_y) = loupe_position((400.0, 500.0), area, 0.0);
        assert!(
            y + size.1 <= loupe_y,
            "chip bottom {} vs loupe top {loupe_y}",
            y + size.1
        );
    }

    #[test]
    fn the_pointer_chip_flips_at_the_right_edge() {
        let area = (1920.0, 1080.0);
        let size = (180.0, 26.0);
        // Fits on the right.
        assert_eq!(pointer_chip_position((1700.0, 500.0), size, area).0, 1730.0);
        // Would leave the monitor: flips to the left of the hot spot.
        assert_eq!(pointer_chip_position((1750.0, 500.0), size, area).0, 1540.0);
        // Vertically clamped into the monitor at the very top and bottom.
        assert_eq!(pointer_chip_position((400.0, 2.0), size, area).1, 0.0);
        assert_eq!(pointer_chip_position((400.0, 1078.0), size, area).1, 1054.0);
    }

    #[test]
    fn the_pointer_is_drawn_only_inside_the_selection() {
        // Scenario "Pointer inside the selection" / "Pointer outside the selection".
        let rect = IRect::new(100, 200, 400, 300); // x 100..500, y 200..500
                                                   // Inside, scale 1: relative to the crop's origin.
        assert_eq!(rect.pointer_inside((150.0, 250.0), 1.0), Some((50.0, 50.0)));
        // Outside on every side.
        assert_eq!(rect.pointer_inside((99.0, 250.0), 1.0), None);
        assert_eq!(rect.pointer_inside((150.0, 199.0), 1.0), None);
        assert_eq!(rect.pointer_inside((500.0, 250.0), 1.0), None);
        assert_eq!(rect.pointer_inside((150.0, 500.0), 1.0), None);
    }

    #[test]
    fn the_hot_spot_decides_not_the_whole_arrow() {
        // Redline: a hot spot one pixel inside the lower-right corner still draws, even though
        // the arrow body (right and down, 22 x 15 px) is then almost entirely clipped.
        let rect = IRect::new(0, 0, 800, 600);
        assert_eq!(
            rect.pointer_inside((799.0, 599.0), 1.0),
            Some((799.0, 599.0))
        );
        // Scenario "Pointer on the selection edge": released exactly on the corner, nothing.
        assert_eq!(rect.pointer_inside((800.0, 600.0), 1.0), None);
        assert_eq!(rect.pointer_inside((800.0, 300.0), 1.0), None);
    }

    #[test]
    fn the_pointer_is_scaled_into_image_pixels() {
        // The overlay reports logical px; the crop is image px. At 125 % a logical 2084.16
        // lands on image 2605 (the maintainer's 4K monitor, measured 2026-11-10).
        let rect = IRect::new(0, 0, 3840, 2160);
        let (x, y) = rect.pointer_inside((2084.15625, 415.4375), 1.25).unwrap();
        assert!((x - 2605.1953125).abs() < 1e-9, "{x}");
        assert!((y - 519.296875).abs() < 1e-9, "{y}");
        // A logical position inside the monitor but outside a small selection is rejected
        // after scaling, not before.
        let small = IRect::new(0, 0, 100, 100);
        assert_eq!(
            small.pointer_inside((80.0, 80.0), 1.25),
            None,
            "80 x 1.25 = 100"
        );
        assert_eq!(
            small.pointer_inside((79.0, 79.0), 1.25),
            Some((98.75, 98.75))
        );
    }

    #[test]
    fn drag_400_by_300() {
        // Scenario "Select and confirm": a 400×300 drag yields a 400×300 selection.
        let mut sel = Selection::new(1920, 1080);
        sel.begin((140, 236));
        sel.drag_to((540, 536), false);
        assert_eq!(sel.rect(), Some(IRect::new(140, 236, 400, 300)));
    }

    #[test]
    fn drag_backwards_normalises() {
        let mut sel = Selection::new(1920, 1080);
        sel.begin((540, 536));
        sel.drag_to((140, 236), false);
        assert_eq!(sel.rect(), Some(IRect::new(140, 236, 400, 300)));
    }

    #[test]
    fn drag_is_clamped_to_monitor() {
        let mut sel = Selection::new(1920, 1080);
        sel.begin((1800, 1000));
        sel.drag_to((2500, 1500), false);
        assert_eq!(sel.rect(), Some(IRect::new(1800, 1000, 120, 80)));
    }

    #[test]
    fn shift_drag_is_square() {
        let mut sel = Selection::new(1920, 1080);
        sel.begin((100, 100));
        sel.drag_to((400, 250), true);
        assert_eq!(sel.rect(), Some(IRect::new(100, 100, 300, 300)));
        sel.drag_to((20, 80), true);
        assert_eq!(sel.rect(), Some(IRect::new(20, 20, 80, 80)));
    }

    #[test]
    fn click_without_drag_has_no_selection() {
        let mut sel = Selection::new(1920, 1080);
        sel.begin((10, 10));
        sel.drag_to((10, 10), false);
        assert_eq!(sel.rect(), None);
    }

    #[test]
    fn arrow_keys_nudge_one_or_ten_px() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(100, 100, 400, 300));
        sel.nudge(Direction::Right, false);
        assert_eq!(sel.rect(), Some(IRect::new(101, 100, 400, 300)));
        sel.nudge(Direction::Down, true);
        assert_eq!(sel.rect(), Some(IRect::new(101, 110, 400, 300)));
        sel.nudge(Direction::Left, true);
        sel.nudge(Direction::Up, false);
        assert_eq!(sel.rect(), Some(IRect::new(91, 109, 400, 300)));
    }

    #[test]
    fn nudge_stops_at_monitor_edge() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(3, 0, 400, 300));
        sel.nudge(Direction::Left, true);
        assert_eq!(sel.rect(), Some(IRect::new(0, 0, 400, 300)));
        sel.nudge(Direction::Up, false);
        assert_eq!(sel.rect(), Some(IRect::new(0, 0, 400, 300)));
    }

    #[test]
    fn fixed_size_sets_exact_size() {
        // Scenario "Fixed size".
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(140, 236, 400, 300));
        sel.set_size(800, 600);
        assert_eq!(sel.rect(), Some(IRect::new(140, 236, 800, 600)));
    }

    #[test]
    fn fixed_size_without_selection_starts_top_left() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_size(640, 480);
        assert_eq!(sel.rect(), Some(IRect::new(0, 0, 640, 480)));
    }

    #[test]
    fn fixed_size_moves_selection_to_fit() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(1500, 900, 100, 100));
        sel.set_size(800, 600);
        assert_eq!(sel.rect(), Some(IRect::new(1120, 480, 800, 600)));
        sel.set_size(5000, 5000);
        assert_eq!(sel.rect(), Some(IRect::new(0, 0, 1920, 1080)));
    }

    #[test]
    fn handles_resize_their_edges() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(100, 100, 400, 300));
        sel.resize(Handle::East, (600, 999));
        assert_eq!(sel.rect(), Some(IRect::new(100, 100, 500, 300)));
        sel.resize(Handle::NorthWest, (50, 80));
        assert_eq!(sel.rect(), Some(IRect::new(50, 80, 550, 320)));
        sel.resize(Handle::South, (0, 300));
        assert_eq!(sel.rect(), Some(IRect::new(50, 80, 550, 220)));
    }

    #[test]
    fn handle_hit_testing() {
        let mut sel = Selection::new(1920, 1080);
        sel.set_rect(IRect::new(100, 100, 400, 300));
        assert_eq!(sel.handle_at((102, 98), 6), Some(Handle::NorthWest));
        assert_eq!(sel.handle_at((300, 400), 6), Some(Handle::South));
        assert_eq!(sel.handle_at((300, 250), 6), None);
        assert!(sel.contains((300, 250)));
    }

    #[test]
    fn eight_distinct_handles() {
        let rect = IRect::new(0, 0, 100, 50);
        let mut positions: Vec<_> = Handle::ALL.iter().map(|h| h.position(rect)).collect();
        positions.sort_unstable();
        positions.dedup();
        assert_eq!(positions.len(), 8);
    }

    #[test]
    fn size_chip_sits_above_or_inside() {
        let area = (1920.0, 1080.0);
        let sel = FRect::from_image(IRect::new(140, 296, 400, 300), 1.0);
        assert_eq!(size_chip_position(sel, (80.0, 26.0), area), (140.0, 264.0));
        let top = FRect::from_image(IRect::new(140, 10, 400, 300), 1.0);
        assert_eq!(size_chip_position(top, (80.0, 26.0), area), (140.0, 16.0));
    }

    #[test]
    fn toolbar_below_then_above() {
        let area = (1920.0, 1080.0);
        let sel = FRect::from_image(IRect::new(140, 236, 800, 600), 1.0);
        assert_eq!(toolbar_position(sel, (718.0, 48.0), area), (140.0, 850.0));
        let low = FRect::from_image(IRect::new(140, 600, 800, 400), 1.0);
        assert_eq!(toolbar_position(low, (718.0, 48.0), area), (140.0, 538.0));
    }

    #[test]
    fn loupe_flips_near_edges() {
        let area = (1920.0, 1080.0);
        assert_eq!(loupe_position((540.0, 596.0), area, 34.0), (566.0, 622.0));
        let (x, y) = loupe_position((1900.0, 1060.0), area, 34.0);
        assert_eq!(x, 1900.0 - 26.0 - 136.0);
        assert_eq!(y, 1060.0 - 26.0 - 136.0 - 34.0);
    }

    #[test]
    fn two_monitors_map_to_image_halves() {
        // Scenario "Multi-monitor": pointer on monitor 2 → only monitor 2 is captured.
        let monitors = [
            IRect::new(0, 0, 1920, 1080),
            IRect::new(1920, 0, 2560, 1440),
        ];
        let layout = bounding_box(&monitors).unwrap();
        assert_eq!(layout, IRect::new(0, 0, 4480, 1440));
        let scale = image_scale(4480, layout);
        assert_eq!(scale, 1.0);
        let crops: Vec<IRect> = monitors
            .iter()
            .map(|m| monitor_crop(*m, layout, scale))
            .collect();
        let full = IRect::new(0, 0, 4480, 1440);
        // The pointer probe reported monitor 2 (index 1).
        assert_eq!(
            screen_capture_rect(&crops, Some(1), false, full),
            IRect::new(1920, 0, 2560, 1440)
        );
        // capture-all-monitors keeps the whole desktop.
        assert_eq!(screen_capture_rect(&crops, Some(1), true, full), full);
        // Unknown pointer position: first monitor.
        assert_eq!(
            screen_capture_rect(&crops, None, false, full),
            IRect::new(0, 0, 1920, 1080)
        );
    }

    #[test]
    fn hidpi_layout_scales_crop() {
        let monitors = [IRect::new(0, 0, 1440, 900), IRect::new(1440, 0, 1920, 1080)];
        let layout = bounding_box(&monitors).unwrap();
        let scale = image_scale(6720, layout);
        assert_eq!(scale, 2.0);
        assert_eq!(
            monitor_crop(monitors[1], layout, scale),
            IRect::new(2880, 0, 3840, 2160)
        );
    }

    #[test]
    fn layout_with_negative_origin() {
        let monitors = [
            IRect::new(-1280, 0, 1280, 1024),
            IRect::new(0, 0, 1920, 1080),
        ];
        let layout = bounding_box(&monitors).unwrap();
        assert_eq!(
            monitor_crop(monitors[1], layout, 1.0),
            IRect::new(1280, 0, 1920, 1080)
        );
    }
}
