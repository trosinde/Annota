//! Capabilities `document-format` and the model half of `editor-canvas`: the document (original
//! image, canvas, annotation objects), command-based undo/redo, `.annota` files and autosave.
//!
//! A [`Document`] can only be changed through [`command::Command`]s: its mutators are
//! crate-private, and the UI (another crate) submits commands to a [`command::History`].

pub mod annotation;
pub mod archive;
pub mod autosave;
pub mod bitmap;
pub mod color;
pub mod command;
pub mod geometry;
pub mod registry;
pub(crate) mod residual;
pub mod scene;
pub mod style;

use std::collections::BTreeMap;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

use bitmap::Bitmap;
use color::Rgba;
use geometry::IRect;
use residual::Residual;
use scene::Scene;

/// What fills the canvas outside (and under transparent parts of) the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Background {
    /// Exported as transparent pixels; shown as a checkerboard on screen (mockup 10).
    Transparent,
    /// Palette swatch, Paper (white), custom colour or eyedropper — all store the colour.
    Color { color: Rgba },
    /// A background type from a newer minor format version (e.g. a future `"gradient"`).
    /// It is drawn like [`Background::Transparent`]; its JSON is kept by the document's
    /// residual (`residual.rs`) and written back unchanged as long as the user does not pick
    /// another background. This version never creates it.
    #[serde(other)]
    Unknown,
}

/// `{"type": "transparent"}`, `{"type": "color", "color": "#RRGGBBAA"}`; an unknown
/// background writes nothing of its own (`{}`) so its original keys, `"type"` included, come
/// back from the residual.
impl Serialize for Background {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Background::Transparent => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("type", "transparent")?;
                map.end()
            }
            Background::Color { color } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "color")?;
                map.serialize_entry("color", color)?;
                map.end()
            }
            Background::Unknown => serializer.serialize_map(Some(0))?.end(),
        }
    }
}

/// The canvas: the exported area, in document pixels. It starts as the image bounds and grows
/// (negative x/y included) when the user extends it (spec: Canvas extension).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Canvas {
    pub rect: IRect,
    pub background: Background,
}

/// The original image (`original.png`), always at document position (0, 0).
#[derive(Clone, Debug)]
pub struct BaseImage {
    pub bitmap: Bitmap,
    /// Rounded corners of the image (sidebar: Image · Corner radius), document pixels.
    pub corner_radius: f64,
}

impl PartialEq for BaseImage {
    fn eq(&self, other: &Self) -> bool {
        self.bitmap.ptr_eq(&other.bitmap) && self.corner_radius == other.corner_radius
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    /// Document name shown as the window title ("Checkout flow").
    pub title: Option<String>,
    /// Device scale of the screen the capture came from (add-screen-capture keeps captures at
    /// device pixels).
    pub scale_factor: f64,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            title: None,
            scale_factor: 1.0,
        }
    }
}

/// Embedded images (inserted picture-in-picture objects), stored as `images/<n>.png`.
#[derive(Clone, Debug, Default)]
pub struct Resources {
    map: BTreeMap<String, Bitmap>,
}

impl Resources {
    pub fn get(&self, id: &str) -> Option<&Bitmap> {
        self.map.get(id)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// An id not used yet, for `Command::Insert { resources, .. }`.
    pub fn fresh_id(&self) -> String {
        let next = self
            .map
            .keys()
            .filter_map(|k| parse_resource_id(k))
            .max()
            .map_or(1, |n| n + 1);
        format!("images/{next}.png")
    }

    pub fn contains(&self, id: &str) -> bool {
        self.map.contains_key(id)
    }

    pub(crate) fn insert(&mut self, id: String, bitmap: Bitmap) {
        self.map.insert(id, bitmap);
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Bitmap> {
        self.map.remove(id)
    }
}

/// `images/<n>.png` → n. Resource ids are ZIP entry names, so only this exact form is valid.
pub fn parse_resource_id(id: &str) -> Option<u32> {
    let n = id.strip_prefix("images/")?.strip_suffix(".png")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || n.starts_with('0') {
        return None;
    }
    n.parse().ok()
}

#[derive(Clone, Debug)]
pub struct Document {
    image: BaseImage,
    canvas: Canvas,
    metadata: Metadata,
    scene: Scene,
    resources: Resources,
    /// What this version did not understand of `document.json` outside `objects` (unknown
    /// keys at any depth, unknown background types), written back on save.
    pub(crate) residual: Option<Residual>,
    /// Minor format version of the file this came from (written back if newer than ours).
    pub(crate) format_minor: u32,
}

impl Document {
    /// A new document for a capture: canvas = image bounds, transparent background.
    pub fn new(bitmap: Bitmap) -> Self {
        let rect = IRect::new(0, 0, bitmap.width(), bitmap.height());
        Self {
            image: BaseImage {
                bitmap,
                corner_radius: 0.0,
            },
            canvas: Canvas {
                rect,
                background: Background::Transparent,
            },
            metadata: Metadata::default(),
            scene: Scene::new(),
            resources: Resources::default(),
            residual: None,
            format_minor: 0,
        }
    }

    /// Same as [`Document::new`] with a title (no history entry: the document is new).
    pub fn with_title(bitmap: Bitmap, title: impl Into<String>) -> Self {
        let mut doc = Self::new(bitmap);
        doc.metadata.title = Some(title.into());
        doc
    }

    pub fn image(&self) -> &BaseImage {
        &self.image
    }

    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn resources(&self) -> &Resources {
        &self.resources
    }

    /// Number of editable objects (sidebar: "N editable objects"; recovery rows).
    pub fn object_count(&self) -> usize {
        self.scene.len()
    }

    pub(crate) fn scene_mut(&mut self) -> &mut Scene {
        &mut self.scene
    }

    pub(crate) fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    pub(crate) fn replace_canvas(&mut self, canvas: Canvas) -> Canvas {
        std::mem::replace(&mut self.canvas, canvas)
    }

    pub(crate) fn replace_image(&mut self, image: BaseImage) -> BaseImage {
        std::mem::replace(&mut self.image, image)
    }

    pub(crate) fn replace_metadata(&mut self, metadata: Metadata) -> Metadata {
        std::mem::replace(&mut self.metadata, metadata)
    }

    pub(crate) fn from_parts(
        image: BaseImage,
        canvas: Canvas,
        metadata: Metadata,
        scene: Scene,
        resources: Resources,
    ) -> Self {
        Self {
            image,
            canvas,
            metadata,
            scene,
            resources,
            residual: None,
            format_minor: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_canvas_is_the_image() {
        let doc = Document::new(Bitmap::filled(64, 48, Rgba::WHITE).unwrap());
        assert_eq!(doc.canvas().rect, IRect::new(0, 0, 64, 48));
        assert_eq!(doc.canvas().background, Background::Transparent);
        assert_eq!(doc.object_count(), 0);
        assert_eq!(doc.resources().fresh_id(), "images/1.png");
    }

    #[test]
    fn resource_ids_are_strict() {
        assert_eq!(parse_resource_id("images/12.png"), Some(12));
        for bad in [
            "images/.png",
            "images/01.png",
            "images/../x.png",
            "original.png",
            "images/1.png/x",
            "images/1.jpg",
        ] {
            assert_eq!(parse_resource_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn background_json_shape() {
        assert_eq!(
            serde_json::to_value(Background::Transparent).unwrap(),
            serde_json::json!({"type": "transparent"})
        );
        assert_eq!(
            serde_json::to_value(Background::Color {
                color: color::palette::PAPER
            })
            .unwrap(),
            serde_json::json!({"type": "color", "color": "#FFFFFFFF"})
        );
        // Unknown types from a newer minor version read as `Unknown` and write nothing.
        let unknown: Background =
            serde_json::from_value(serde_json::json!({"type": "gradient", "stops": []})).unwrap();
        assert_eq!(unknown, Background::Unknown);
        assert_eq!(
            serde_json::to_value(unknown).unwrap(),
            serde_json::json!({})
        );
        // A malformed known type is still an error.
        assert!(serde_json::from_value::<Background>(
            serde_json::json!({"type": "color", "color": 7})
        )
        .is_err());
    }
}
