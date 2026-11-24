//! The recordings folder and the working file's name (requirement "Working file").

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use gtk::glib;

/// Subfolder of the XDG videos directory that holds recordings by default (design.md Q4).
const FOLDER_NAME: &str = "Annota";
/// The working file's extension: Matroska stays playable when the process dies (D3).
const EXTENSION: &str = "mkv";
/// How many ` (n)` suffixes are tried before giving up.
const MAX_SUFFIX: u32 = 999;

/// The recordings folder: the `record-folder` setting, or `~/Videos/Annota` (the XDG videos
/// directory, `~/Videos` without one) when it is empty.
pub fn recordings_folder(setting: &str) -> PathBuf {
    if !setting.trim().is_empty() {
        return PathBuf::from(setting);
    }
    glib::user_special_dir(glib::UserDirectory::Videos)
        .unwrap_or_else(|| glib::home_dir().join("Videos"))
        .join(FOLDER_NAME)
}

/// `Recording YYYY-MM-DD HH-MM-SS` for the given local time.
pub fn base_name(time: &glib::DateTime) -> String {
    let stamp = time
        .format("%Y-%m-%d %H-%M-%S")
        .map(|s| s.to_string())
        .unwrap_or_default();
    format!("Recording {stamp}")
}

/// Creates the working file `<base>.mkv` in `folder` (and the folder if missing), never
/// overwriting a file: a taken name gets ` (2)`, ` (3)`, … The file is created empty with
/// `create_new`, so the name is reserved without a race against another recording.
pub fn create_working_file(folder: &Path, base: &str) -> io::Result<(PathBuf, File)> {
    create_unique(folder, base, EXTENSION)
}

/// Creates `<base>.<extension>` in `folder` (and the folder if missing), never overwriting a
/// file: a taken name gets ` (2)`, ` (3)`, … (the working file, an exported recording).
pub fn create_unique(folder: &Path, base: &str, extension: &str) -> io::Result<(PathBuf, File)> {
    fs::create_dir_all(folder)?;
    for n in 1..=MAX_SUFFIX {
        let name = if n == 1 {
            format!("{base}.{extension}")
        } else {
            format!("{base} ({n}).{extension}")
        };
        let path = folder.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "no free file name",
    ))
}

/// The working file's name without `.mkv`: the export's default file name.
pub fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: screen-recording / Working file / Name already taken
    #[test]
    fn a_taken_name_gets_a_suffix_and_the_other_file_stays() {
        let dir = std::env::temp_dir().join(format!("annota-rec-names-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let folder = dir.join("Videos/Annota"); // created on demand
        let time = glib::DateTime::from_local(2026, 11, 23, 9, 5, 7.0).unwrap();
        let base = base_name(&time);
        assert_eq!(base, "Recording 2026-11-23 09-05-07");

        let (first, _) = create_working_file(&folder, &base).unwrap();
        assert_eq!(first, folder.join("Recording 2026-11-23 09-05-07.mkv"));
        fs::write(&first, b"earlier recording").unwrap();

        let (second, _) = create_working_file(&folder, &base).unwrap();
        assert_eq!(second, folder.join("Recording 2026-11-23 09-05-07 (2).mkv"));
        let (third, _) = create_working_file(&folder, &base).unwrap();
        assert_eq!(third, folder.join("Recording 2026-11-23 09-05-07 (3).mkv"));
        assert_eq!(fs::read(&first).unwrap(), b"earlier recording");

        assert_eq!(recordings_folder("/tmp/x"), PathBuf::from("/tmp/x"));
        assert!(recordings_folder("").ends_with("Annota"));
        let _ = fs::remove_dir_all(&dir);
    }
}
