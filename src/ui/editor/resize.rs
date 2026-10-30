//! "Resize Image" (mockup 09; spec editor-canvas: Image operations): width and height in
//! pixels or percent, linked while "Keep aspect ratio" is on, and "Scale annotations".
//!
//! [`Form`] holds the values without widgets; [`present`] binds it to `resize-dialog.ui`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use annota::document::bitmap::MAX_SIDE;

const UI_RESIZE: &str = "/io/github/trosinde/Annota/ui/resize-dialog.ui";
/// Largest percentage offered.
const MAX_PERCENT: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Pixels,
    Percent,
}

/// The dialog's values: the new size in pixels, shown in `unit`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Form {
    /// The image's size now.
    pub original: (i32, i32),
    /// The new size in pixels (fractional while linked, rounded when used).
    pub size: (f64, f64),
    pub keep_ratio: bool,
    pub unit: Unit,
}

impl Form {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            original: (width.max(1), height.max(1)),
            size: (f64::from(width.max(1)), f64::from(height.max(1))),
            keep_ratio: true,
            unit: Unit::Pixels,
        }
    }

    fn ratio(&self) -> f64 {
        f64::from(self.original.0) / f64::from(self.original.1)
    }

    /// A value in the current unit for the width (`horizontal`) or the height.
    fn shown(&self, horizontal: bool) -> f64 {
        let (px, original) = if horizontal {
            (self.size.0, self.original.0)
        } else {
            (self.size.1, self.original.1)
        };
        match self.unit {
            Unit::Pixels => px.round(),
            Unit::Percent => (px / f64::from(original) * 100.0).round(),
        }
    }

    /// The width and height spins' values in the current unit.
    pub fn values(&self) -> (f64, f64) {
        (self.shown(true), self.shown(false))
    }

    /// The spins' range in the current unit.
    pub fn range(&self, horizontal: bool) -> (f64, f64) {
        match self.unit {
            Unit::Pixels => (1.0, f64::from(MAX_SIDE)),
            Unit::Percent => {
                let original = if horizontal {
                    self.original.0
                } else {
                    self.original.1
                };
                let min = (100.0 / f64::from(original)).ceil().max(1.0);
                let max = (f64::from(MAX_SIDE) / f64::from(original) * 100.0)
                    .floor()
                    .min(MAX_PERCENT);
                (min, max.max(min))
            }
        }
    }

    /// The width (`horizontal`) or height spin was set to `value` (current unit); with the
    /// ratio kept, the other side follows.
    pub fn set(&mut self, horizontal: bool, value: f64) {
        if !value.is_finite() {
            return;
        }
        let original = if horizontal {
            self.original.0
        } else {
            self.original.1
        };
        let px = match self.unit {
            Unit::Pixels => value,
            Unit::Percent => value / 100.0 * f64::from(original),
        }
        .clamp(1.0, f64::from(MAX_SIDE));
        if horizontal {
            self.size.0 = px;
            if self.keep_ratio {
                self.size.1 = (px / self.ratio()).clamp(1.0, f64::from(MAX_SIDE));
            }
        } else {
            self.size.1 = px;
            if self.keep_ratio {
                self.size.0 = (px * self.ratio()).clamp(1.0, f64::from(MAX_SIDE));
            }
        }
    }

    /// Turning "Keep aspect ratio" on matches the height to the width.
    pub fn set_keep_ratio(&mut self, keep: bool) {
        self.keep_ratio = keep;
        if keep {
            let width = self.shown(true);
            self.set(true, width);
        }
    }

    /// The size to resize to, in whole pixels.
    pub fn pixels(&self) -> (i32, i32) {
        let clamp = |v: f64| (v.round() as i32).clamp(1, MAX_SIDE);
        (clamp(self.size.0), clamp(self.size.1))
    }
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: resize-dialog.ui is missing object '{}'", $name);
                return;
            }
        }
    };
}

/// Shows "Resize Image" for an image of `width` × `height` over `parent`. Resize calls
/// `resize(width, height, scale_annotations)` with the new size in pixels.
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    width: i32,
    height: i32,
    resize: impl Fn(i32, i32, bool) + 'static,
) {
    let builder = gtk::Builder::from_resource(UI_RESIZE);
    let dialog: adw::Dialog = obj!(builder, "resize_dialog");
    let width_row: adw::SpinRow = obj!(builder, "width_row");
    let height_row: adw::SpinRow = obj!(builder, "height_row");
    let keep_row: adw::SwitchRow = obj!(builder, "keep_ratio_row");
    let px_button: gtk::ToggleButton = obj!(builder, "unit_px_button");
    let scale_row: adw::SwitchRow = obj!(builder, "scale_row");
    let cancel_button: gtk::Button = obj!(builder, "cancel_button");
    let resize_button: gtk::Button = obj!(builder, "resize_button");

    let form = Rc::new(RefCell::new(Form::new(width, height)));
    let updating = Rc::new(Cell::new(false));
    // Shows the form in the spins without reporting it back.
    let show = {
        let (form, updating) = (form.clone(), updating.clone());
        let (width_row, height_row) = (width_row.clone(), height_row.clone());
        move || {
            updating.set(true);
            let f = *form.borrow();
            for (row, horizontal, value) in [
                (&width_row, true, f.values().0),
                (&height_row, false, f.values().1),
            ] {
                let (lower, upper) = f.range(horizontal);
                let adjustment = row.adjustment();
                adjustment.set_lower(lower);
                adjustment.set_upper(upper);
                row.set_value(value.clamp(lower, upper));
            }
            updating.set(false);
        }
    };
    show();
    for (row, horizontal) in [(&width_row, true), (&height_row, false)] {
        let (form, updating, show) = (form.clone(), updating.clone(), show.clone());
        row.connect_value_notify(move |row| {
            if updating.get() {
                return;
            }
            form.borrow_mut().set(horizontal, row.value());
            show();
        });
    }
    {
        let (form, show) = (form.clone(), show.clone());
        keep_row.connect_active_notify(move |row| {
            form.borrow_mut().set_keep_ratio(row.is_active());
            show();
        });
    }
    {
        let (form, show) = (form.clone(), show.clone());
        px_button.connect_toggled(move |button| {
            form.borrow_mut().unit = if button.is_active() {
                Unit::Pixels
            } else {
                Unit::Percent
            };
            show();
        });
    }
    cancel_button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    resize_button.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        #[weak]
        scale_row,
        move |_| {
            // A value typed but not confirmed yet counts.
            width_row.update();
            height_row.update();
            let (w, h) = form.borrow().pixels();
            dialog.close();
            resize(w, h, scale_row.is_active());
        }
    ));
    dialog.set_default_widget(Some(&resize_button));
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_sizes_keep_the_ratio() {
        // Mockup 09: 960 × 800, ratio kept.
        let mut form = Form::new(960, 800);
        form.set(true, 480.0);
        assert_eq!(form.pixels(), (480, 400));
        form.set(false, 1000.0);
        assert_eq!(form.pixels(), (1200, 1000));
        // Unlinked: only the edited side changes.
        form.set_keep_ratio(false);
        form.set(true, 500.0);
        assert_eq!(form.pixels(), (500, 1000));
        // Linking again fits the height to the width.
        form.set_keep_ratio(true);
        assert_eq!(form.pixels(), (500, 417));
    }

    #[test]
    fn percent_unit() {
        let mut form = Form::new(960, 800);
        form.unit = Unit::Percent;
        assert_eq!(form.values(), (100.0, 100.0));
        form.set(true, 50.0);
        assert_eq!(form.pixels(), (480, 400));
        assert_eq!(form.values(), (50.0, 50.0));
        // Never below one pixel or above cairo's limit.
        assert_eq!(form.range(true), (1.0, 1000.0));
        form.unit = Unit::Pixels;
        form.set(true, 0.0);
        assert_eq!(form.pixels().0, 1);
        form.set(true, 1e9);
        assert_eq!(form.pixels().0, MAX_SIDE);
    }
}
