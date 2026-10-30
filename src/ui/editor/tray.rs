//! The recent captures tray (task 4.2; spec editor-canvas: Recent captures tray; redline
//! "Window and shell", mockup 11).
//!
//! **Data source.** "Open" lists the documents open in the editor window; clicking one switches
//! to it. "Recent" is meant to come from the capture library's index (add-capture-library,
//! not built yet). Until then it lists the newest [`RECENT_LIMIT`] `.annota` and `.png` files of
//! the save folder (the `save-folder` preference — where quick-saved captures go today and
//! where Ctrl+S will put new documents), excluding files that are open; clicking one opens it.
//! Thumbnails come from the file's own `thumbnail.png` (`.annota`) or the scaled-down image
//! (`.png`) and are decoded off the main thread. Dragging any tile onto the canvas inserts its
//! image as an object (spec "Picture in picture").

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use annota::document::archive;
use annota::document::bitmap::Bitmap;

/// Spec: "the last 50 captures".
pub const RECENT_LIMIT: usize = 50;
/// Redline: thumbnails 100 × 62.
pub const THUMB_WIDTH: i32 = 100;
pub const THUMB_HEIGHT: i32 = 62;
/// Drag image (redline "Canvas": 240 × 150).
const DRAG_WIDTH: i32 = 240;
const DRAG_HEIGHT: i32 = 150;
/// Half the drag image: it is centred on the pointer.
pub fn drag_half_size() -> (f64, f64) {
    (f64::from(DRAG_WIDTH) / 2.0, f64::from(DRAG_HEIGHT) / 2.0)
}

/// Thumbnails are decoded at twice the tile size for HiDPI screens.
const DECODE_SCALE: i32 = 2;

/// Prefix of the drag payload of a tray tile (a string, so it stays inside Annota).
const PAYLOAD: &str = "annota-tray:";

/// What a tray tile stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayKey {
    /// An open document, by session uid.
    Open(u64),
    /// A file in the save folder.
    Recent(PathBuf),
}

impl TrayKey {
    pub fn to_payload(&self) -> String {
        match self {
            TrayKey::Open(uid) => format!("{PAYLOAD}open:{uid}"),
            TrayKey::Recent(path) => format!("{PAYLOAD}recent:{}", path.display()),
        }
    }

    pub fn from_payload(text: &str) -> Option<Self> {
        let rest = text.strip_prefix(PAYLOAD)?;
        if let Some(uid) = rest.strip_prefix("open:") {
            return uid.parse().ok().map(TrayKey::Open);
        }
        let path = rest.strip_prefix("recent:")?;
        (!path.is_empty()).then(|| TrayKey::Recent(PathBuf::from(path)))
    }
}

/// A file for the "Recent" group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentFile {
    pub path: PathBuf,
    pub modified: SystemTime,
}

fn is_recent_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("annota") || e.eq_ignore_ascii_case("png"))
}

/// The newest `limit` `.annota` / `.png` files directly in `dir`, newest first, without the
/// files in `open` (canonical paths, see `session::canonical`) — whatever their type, so an
/// open image is not offered again. A folder that does not exist has none.
pub fn recent_files(dir: &Path, open: &[PathBuf], limit: usize) -> Vec<RecentFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<RecentFile> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let meta = entry.metadata().ok()?;
            let visible = !entry.file_name().to_string_lossy().starts_with('.');
            let is_open = || open.contains(&super::session::canonical(&path));
            (visible && meta.is_file() && is_recent_candidate(&path) && !is_open()).then(|| {
                RecentFile {
                    path,
                    modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                }
            })
        })
        .collect();
    files.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)));
    files.truncate(limit);
    files
}

/// Thumbnail pixels made off the main thread (cairo `ARGB32`).
pub struct Pixels {
    width: i32,
    height: i32,
    stride: usize,
    data: Vec<u8>,
}

/// Scales `bitmap` so that it covers `width` × `height` (never enlarged) and returns its pixels.
pub fn scaled_pixels(bitmap: &Bitmap, width: i32, height: i32) -> Option<Pixels> {
    let (bw, bh) = (f64::from(bitmap.width()), f64::from(bitmap.height()));
    let scale = (f64::from(width) / bw).max(f64::from(height) / bh).min(1.0);
    let w = (bw * scale).round().max(1.0) as i32;
    let h = (bh * scale).round().max(1.0) as i32;
    let mut surface = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, w, h).ok()?;
    {
        let cr = gtk::cairo::Context::new(&surface).ok()?;
        cr.scale(scale, scale);
        cr.set_source_surface(bitmap.surface(), 0.0, 0.0).ok()?;
        cr.source().set_filter(gtk::cairo::Filter::Good);
        cr.paint().ok()?;
    }
    surface.flush();
    let stride = usize::try_from(surface.stride()).ok()?;
    let data = surface.data().ok()?.to_vec();
    Some(Pixels {
        width: w,
        height: h,
        stride,
        data,
    })
}

/// Decodes the thumbnail of a recent file (runs on a worker thread).
fn decode_thumbnail(path: &Path) -> Option<Pixels> {
    let png = if super::session::is_annota(path) {
        archive::read_summary(path).ok()?.thumbnail_png?
    } else {
        std::fs::read(path).ok()?
    };
    let bitmap = Bitmap::from_png(png).ok()?;
    scaled_pixels(
        &bitmap,
        THUMB_WIDTH * DECODE_SCALE,
        THUMB_HEIGHT * DECODE_SCALE,
    )
}

pub fn texture(pixels: Pixels) -> gdk::Texture {
    let format = if cfg!(target_endian = "little") {
        gdk::MemoryFormat::B8g8r8a8Premultiplied
    } else {
        gdk::MemoryFormat::A8r8g8b8Premultiplied
    };
    gdk::MemoryTexture::new(
        pixels.width,
        pixels.height,
        format,
        &glib::Bytes::from_owned(pixels.data),
        pixels.stride,
    )
    .upcast()
}

/// A tray thumbnail of an open document.
pub fn bitmap_texture(bitmap: &Bitmap) -> Option<gdk::Texture> {
    scaled_pixels(
        bitmap,
        THUMB_WIDTH * DECODE_SCALE,
        THUMB_HEIGHT * DECODE_SCALE,
    )
    .map(texture)
}

/// One tile of the "Open" group.
pub struct OpenTile {
    pub uid: u64,
    pub name: String,
    pub thumbnail: Option<gdk::Texture>,
    pub current: bool,
    pub unsaved: bool,
}

pub type Activate = Rc<dyn Fn(TrayKey)>;

/// The tray's widgets (from `editor-window.ui`) and the "Recent" state.
pub struct Tray {
    pub revealer: gtk::Revealer,
    open_section: gtk::Box,
    open_separator: gtk::Separator,
    open_items: gtk::Box,
    recent_section: gtk::Box,
    recent_items: gtk::Box,
    activate: RefCell<Option<Activate>>,
    /// The "Open" tiles by session uid; updated in place, so a tile being dragged survives.
    open_tiles: RefCell<Vec<(u64, Tile)>>,
    /// The listing the "Recent" tiles show.
    recent: RefCell<Vec<RecentFile>>,
    /// Decoded thumbnails by file and modification time.
    cache: Rc<RefCell<HashMap<PathBuf, (SystemTime, gdk::Texture)>>>,
    /// Bumped on every refresh, so a slow thumbnail of an older listing is dropped.
    generation: Rc<Cell<u64>>,
}

impl Tray {
    pub fn new(builder: &gtk::Builder) -> Option<Self> {
        Some(Self {
            revealer: builder.object("tray_revealer")?,
            open_section: builder.object("open_section")?,
            open_separator: builder.object("open_separator")?,
            open_items: builder.object("open_items")?,
            recent_section: builder.object("recent_section")?,
            recent_items: builder.object("recent_items")?,
            activate: RefCell::new(None),
            open_tiles: RefCell::new(Vec::new()),
            recent: RefCell::new(Vec::new()),
            cache: Rc::default(),
            generation: Rc::default(),
        })
    }

    /// What clicking a tile does.
    pub fn connect_activate(&self, f: impl Fn(TrayKey) + 'static) {
        self.activate.replace(Some(Rc::new(f)));
    }

    fn activate(&self) -> Activate {
        self.activate
            .borrow()
            .clone()
            .unwrap_or_else(|| Rc::new(|_| {}))
    }

    /// Shows the "Open" group: rebuilt when documents open or close, otherwise updated.
    pub fn set_open(&self, tiles: &[OpenTile]) {
        let same = {
            let current = self.open_tiles.borrow();
            current.len() == tiles.len()
                && current.iter().zip(tiles).all(|((uid, _), t)| *uid == t.uid)
        };
        if !same {
            clear(&self.open_items);
            let activate = self.activate();
            let built: Vec<(u64, Tile)> = tiles
                .iter()
                .map(|t| {
                    (
                        t.uid,
                        Tile::new(None, &t.name, TrayKey::Open(t.uid), &activate),
                    )
                })
                .collect();
            for (_, tile) in &built {
                self.open_items.append(&tile.button);
            }
            self.open_tiles.replace(built);
        }
        for ((_, tile), t) in self.open_tiles.borrow().iter().zip(tiles) {
            tile.picture.set_paintable(t.thumbnail.as_ref());
            tile.button.set_tooltip_text(Some(&t.name));
            tile.dot.set_visible(t.unsaved);
            if t.current {
                tile.button.add_css_class("current");
            } else {
                tile.button.remove_css_class("current");
            }
        }
        self.open_section.set_visible(!tiles.is_empty());
        self.open_separator
            .set_visible(!tiles.is_empty() && self.recent_section.is_visible());
    }

    /// Shows the "Recent" group for `dir`, leaving out `open` files. Does nothing if the
    /// listing is unchanged.
    pub fn refresh_recent(&self, dir: &Path, open: &[PathBuf]) {
        let files = recent_files(dir, open, RECENT_LIMIT);
        if *self.recent.borrow() == files {
            return;
        }
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        clear(&self.recent_items);
        let activate = self.activate();
        let mut missing = Vec::new();
        for file in &files {
            let cached = self
                .cache
                .borrow()
                .get(&file.path)
                .filter(|(modified, _)| *modified == file.modified)
                .map(|(_, texture)| texture.clone());
            let name = super::session::file_stem(&file.path);
            let tile = Tile::new(
                cached.as_ref(),
                &name,
                TrayKey::Recent(file.path.clone()),
                &activate,
            );
            if cached.is_none() {
                missing.push((file.clone(), tile.picture.downgrade()));
            }
            self.recent_items.append(&tile.button);
        }
        self.recent_section.set_visible(!files.is_empty());
        self.open_separator
            .set_visible(self.open_section.is_visible() && !files.is_empty());
        self.recent.replace(files);
        if missing.is_empty() {
            return;
        }
        // One file at a time on a worker thread; the tiles fill in as they arrive.
        let cache = self.cache.clone();
        let current = self.generation.clone();
        glib::spawn_future_local(async move {
            for (file, picture) in missing {
                if current.get() != generation {
                    return;
                }
                let path = file.path.clone();
                let Ok(Some(pixels)) = gio::spawn_blocking(move || decode_thumbnail(&path)).await
                else {
                    continue;
                };
                let texture = texture(pixels);
                cache
                    .borrow_mut()
                    .insert(file.path.clone(), (file.modified, texture.clone()));
                if let Some(picture) = picture.upgrade() {
                    picture.set_paintable(Some(&texture));
                }
            }
        });
    }

    #[cfg(test)]
    pub fn counts(&self) -> (usize, usize) {
        let count = |b: &gtk::Box| {
            let mut n = 0;
            let mut child = b.first_child();
            while let Some(c) = child {
                n += 1;
                child = c.next_sibling();
            }
            n
        };
        (count(&self.open_items), count(&self.recent_items))
    }
}

fn clear(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

/// A thumbnail button: picture, unsaved dot, click to activate, drag onto the canvas.
struct Tile {
    button: gtk::Button,
    picture: gtk::Picture,
    dot: gtk::Box,
}

impl Tile {
    fn new(texture: Option<&gdk::Texture>, name: &str, key: TrayKey, activate: &Activate) -> Self {
        let picture = gtk::Picture::builder()
            .can_shrink(true)
            .content_fit(gtk::ContentFit::Cover)
            .width_request(THUMB_WIDTH)
            .height_request(THUMB_HEIGHT)
            .overflow(gtk::Overflow::Hidden)
            .css_classes(["annota-tray-thumb"])
            .build();
        picture.set_paintable(texture);
        let dot = gtk::Box::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .visible(false)
            .can_target(false)
            .css_classes(["annota-tray-dot"])
            .build();
        let overlay = gtk::Overlay::builder().child(&picture).build();
        overlay.add_overlay(&dot);
        let button = gtk::Button::builder()
            .child(&overlay)
            .tooltip_text(name)
            .css_classes(["flat", "annota-tray-item"])
            .build();
        button.update_property(&[gtk::accessible::Property::Label(name)]);
        let on_click = activate.clone();
        let clicked = key.clone();
        button.connect_clicked(move |_| on_click(clicked.clone()));

        let source = gtk::DragSource::new();
        source.set_actions(gdk::DragAction::COPY);
        let payload = key.to_payload();
        source.connect_prepare(move |_, _, _| {
            Some(gdk::ContentProvider::for_value(&payload.to_value()))
        });
        source.connect_drag_begin(glib::clone!(
            #[weak]
            button,
            #[weak]
            picture,
            move |_, drag| {
                // Redline "Canvas": drag image 240 × 150 with a 2 px accent ring, source at 45 %.
                let icon = gtk::Picture::builder()
                    .can_shrink(true)
                    .content_fit(gtk::ContentFit::Cover)
                    .width_request(DRAG_WIDTH)
                    .height_request(DRAG_HEIGHT)
                    .overflow(gtk::Overflow::Hidden)
                    .css_classes(["annota-drag-icon"])
                    .build();
                icon.set_paintable(picture.paintable().as_ref());
                gtk::DragIcon::for_drag(drag).set_child(Some(&icon));
                drag.set_hotspot(DRAG_WIDTH / 2, DRAG_HEIGHT / 2);
                button.set_opacity(0.45);
            }
        ));
        source.connect_drag_end(glib::clone!(
            #[weak]
            button,
            move |_, _, _| button.set_opacity(1.0)
        ));
        button.add_controller(source);
        Self {
            button,
            picture,
            dot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::color::Rgba;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("annota-tray-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn payload_round_trip() {
        for key in [
            TrayKey::Open(7),
            TrayKey::Recent(PathBuf::from(
                "/home/u/Pictures/Annota/Checkout flow.annota",
            )),
        ] {
            assert_eq!(TrayKey::from_payload(&key.to_payload()), Some(key));
        }
        assert_eq!(TrayKey::from_payload("hello"), None);
        assert_eq!(TrayKey::from_payload("annota-tray:open:x"), None);
        assert_eq!(TrayKey::from_payload("annota-tray:recent:"), None);
    }

    #[test]
    fn recent_lists_newest_captures_without_open_ones() {
        let tmp = TempDir::new("recent");
        let names = [
            "a.png",
            "b.annota",
            "c.PNG",
            "d.jpg",
            "notes.txt",
            ".hidden.png",
        ];
        for (k, name) in names.iter().enumerate() {
            let path = tmp.0.join(name);
            std::fs::write(&path, b"x").unwrap();
            let time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000 + k as u64);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(time)
                .unwrap();
        }
        std::fs::create_dir(tmp.0.join("folder.png")).unwrap();
        let names_of = |files: Vec<RecentFile>| -> Vec<String> {
            files
                .iter()
                .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            names_of(recent_files(&tmp.0, &[], RECENT_LIMIT)),
            ["c.PNG", "b.annota", "a.png"]
        );
        assert_eq!(
            names_of(recent_files(&tmp.0, &[tmp.0.join("b.annota")], 2)),
            ["c.PNG", "a.png"]
        );
        assert!(recent_files(&tmp.0.join("missing"), &[], 50).is_empty());
        // Bug fix: an open image (not only an open `.annota` file) is left out, also when the
        // folder is named differently.
        let detour = tmp.0.join(".");
        let open_png = super::super::session::canonical(&tmp.0.join("a.png"));
        assert_eq!(
            names_of(recent_files(&detour, &[open_png], RECENT_LIMIT)),
            ["c.PNG", "b.annota"]
        );
    }

    #[test]
    fn thumbnails_cover_the_tile_and_are_never_enlarged() {
        let wide = Bitmap::filled(1600, 1000, Rgba::WHITE).unwrap();
        let p = scaled_pixels(&wide, 200, 124).unwrap();
        // Covers 200 × 124: the width decides (1600 → 200), the height overflows a little.
        assert_eq!((p.width, p.height), (200, 125));
        let p = scaled_pixels(&Bitmap::filled(40, 30, Rgba::WHITE).unwrap(), 200, 124).unwrap();
        assert_eq!((p.width, p.height), (40, 30));
        assert_eq!(p.data.len(), p.stride * 30);
    }

    #[test]
    fn annota_thumbnails_come_from_the_file() {
        let tmp = TempDir::new("thumb");
        let doc = annota::document::Document::new(Bitmap::filled(800, 500, Rgba::WHITE).unwrap());
        let path = tmp.0.join("x.annota");
        archive::save(&doc, &path).unwrap();
        let p = decode_thumbnail(&path).unwrap();
        assert_eq!((p.width, p.height), (200, 125));
        std::fs::write(tmp.0.join("broken.annota"), b"no zip").unwrap();
        assert!(decode_thumbnail(&tmp.0.join("broken.annota")).is_none());
    }
}
