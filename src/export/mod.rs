//! Capability `export-share`, requirement "Export formats": the document as PNG, JPG, WebP or
//! PDF, and as an editable `.annota` file.
//!
//! Free of GTK like the rest of the library: the export dialog, quick save, batch export,
//! clipboard and drag-and-drop (later phases of add-export-share) all call into here.
//!
//! * Raster formats ([`raster`]) re-encode the bitmap of [`crate::render::flatten`] (the canvas
//!   renderer at the export scale), so at 100 % they are pixel-identical to the canvas
//!   (scenario "WYSIWYG"). The encoders are compiled into the binary (design.md, "Decided
//!   during apply"), so no format can be missing on a particular system.
//! * PDF ([`pdf`]) draws the objects through the same renderer onto a cairo PDF surface, so
//!   text and shapes stay vectors; what must not stay a vector (redacted content) is burnt into
//!   the page's one bitmap.

pub mod pdf;
pub mod raster;

use std::path::Path;

use crate::document::archive::{self, ArchiveError};
use crate::document::bitmap::BitmapError;
use crate::document::Document;

pub use pdf::{encode_pdf, PageSize, PdfOptions};
pub use raster::{encode_jpeg, encode_png, encode_webp};

/// JPG quality the export dialog starts with (redline "Export dialog": default 85).
pub const DEFAULT_JPEG_QUALITY: u8 = 85;
/// Lossy WebP quality the export dialog starts with (redline: default 80).
pub const DEFAULT_WEBP_QUALITY: u8 = 80;

/// WebP compression (redline: "Compression" Lossless | Lossy).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebPCompression {
    Lossless,
    /// Quality 1–100.
    Lossy {
        quality: u8,
    },
}

/// An export format with its encoder settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Png,
    /// Quality 1–100.
    Jpeg {
        quality: u8,
    },
    WebP {
        compression: WebPCompression,
    },
    Pdf(PdfOptions),
}

impl Format {
    /// File name extension, without the dot (the dialog shows it after the name).
    pub fn extension(&self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg { .. } => "jpg",
            Format::WebP { .. } => "webp",
            Format::Pdf(_) => "pdf",
        }
    }

    /// The format's short name (redline: toggle group PNG | JPG | WebP | PDF).
    pub fn name(&self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg { .. } => "JPG",
            Format::WebP { .. } => "WebP",
            Format::Pdf(_) => "PDF",
        }
    }
}

/// Why an export failed. Messages are English diagnostics; the export dialog shows its own
/// translated text per variant (mockup 04: banner, e.g. for [`ExportError::io_kind`]
/// `PermissionDenied`).
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The size factor is not a finite number above zero.
    #[error("export scale {0} is not supported")]
    Scale(f64),
    /// JPG and lossy WebP quality must be 1–100.
    #[error("quality {0} is outside 1–100")]
    Quality(u8),
    /// The image is larger than the format allows (WebP: 16383 px per side).
    #[error(
        "{format} images can be at most {max} px wide and high; this one is {width} × {height}"
    )]
    TooLarge {
        format: &'static str,
        max: i32,
        width: i32,
        height: i32,
    },
    /// Drawing the document failed (size limits of the renderer, out of memory).
    #[error("could not draw the document: {0}")]
    Render(#[from] BitmapError),
    #[error("cairo error: {0}")]
    Cairo(#[from] cairo::Error),
    /// The encoder refused the image.
    #[error("{format} encoder failed: {reason}")]
    Encoder {
        format: &'static str,
        reason: String,
    },
    /// Writing the file failed (folder not writable, disk full …).
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Saving a `.annota` file failed.
    #[error("{0}")]
    Archive(#[from] ArchiveError),
}

impl ExportError {
    /// The kind of a file system error, from an export or a `.annota` save alike (mockup 04's
    /// "No permission to write to this folder" is `PermissionDenied`).
    pub fn io_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            ExportError::Io(err) | ExportError::Archive(ArchiveError::Io(err)) => Some(err.kind()),
            _ => None,
        }
    }
}

/// Checks a size factor: finite and above zero (redline "Size": 200 %, 100 %, 50 %, 25 %, custom).
pub(crate) fn check_scale(scale: f64) -> Result<f64, ExportError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(scale)
    } else {
        Err(ExportError::Scale(scale))
    }
}

/// Checks a JPG or WebP quality: 1–100.
pub(crate) fn check_quality(quality: u8) -> Result<u8, ExportError> {
    if (1..=100).contains(&quality) {
        Ok(quality)
    } else {
        Err(ExportError::Quality(quality))
    }
}

/// The file content of `doc` in `format`, `scale` output pixels per document pixel (1.0 is
/// 100 %, pixel-identical to the canvas).
pub fn encode(doc: &Document, format: &Format, scale: f64) -> Result<Vec<u8>, ExportError> {
    let scale = check_scale(scale)?;
    // Refuse bad settings before drawing anything.
    match format {
        Format::Jpeg { quality }
        | Format::WebP {
            compression: WebPCompression::Lossy { quality },
        } => {
            check_quality(*quality)?;
        }
        Format::Png | Format::WebP { .. } | Format::Pdf(_) => {}
    }
    let flatten = || crate::render::flatten(doc, scale);
    match format {
        Format::Png => encode_png(&flatten()?),
        Format::Jpeg { quality } => encode_jpeg(&flatten()?, *quality),
        Format::WebP { compression } => encode_webp(&flatten()?, *compression),
        Format::Pdf(options) => encode_pdf(doc, scale, options),
    }
}

/// Exports `doc` to `path`. The file is replaced only once the new one is complete (a failed
/// export leaves an existing file as it was).
pub fn export(doc: &Document, path: &Path, format: &Format, scale: f64) -> Result<(), ExportError> {
    let bytes = encode(doc, format, scale)?;
    archive::write_atomically(path, &bytes)?;
    Ok(())
}

/// Saves `doc` as an editable `.annota` file at `path` (the document format's own save, the
/// same one autosave uses, here for Save As and quick save).
pub fn save_annota(doc: &Document, path: &Path) -> Result<(), ExportError> {
    archive::save(doc, path)?;
    Ok(())
}

/// Save and Save As (task 2.2): like [`save_annota`], but the file's document title is the
/// file's name, so a saved document is called what its file is called — in the window title
/// right after saving (mockup 06) and whenever the file is opened again. `doc` itself is not
/// changed (a title is not an edit with an undo step); only the written copy is renamed.
pub fn save_annota_named(doc: &Document, path: &Path) -> Result<(), ExportError> {
    let title = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty());
    if title.is_none() || doc.metadata().title == title {
        return save_annota(doc, path);
    }
    let mut copy = doc.clone();
    copy.replace_metadata(crate::document::Metadata {
        title,
        ..doc.metadata().clone()
    });
    save_annota(&copy, path)
}

#[cfg(test)]
mod tests;
