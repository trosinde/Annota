//! Where styles and themes live (design.md: "Styles stored in `~/.local/share/annota/styles/`";
//! spec style-system: Theme exchange):
//!
//! * `<data>/styles.json` — the user's quick styles, favourites and current theme
//!   ([`super::library`]);
//! * `<data>/themes/*.annota-theme` — "My Themes", one file each, named after the theme;
//! * `/usr/share/annota/themes/*.annota-theme` — themes installed by an administrator for every
//!   user, read-only ("Installed by administrator"); files that are not valid themes are
//!   skipped (and reported on stderr), they never stop the others from loading.
//!
//! Files are written to a temporary file next to the target and renamed over it, so a crash
//! never leaves half a file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::library::{self, Library, Unreadable};
use super::theme::{Origin, Theme, ThemeError, EXTENSION};

/// The system-wide theme directory for managed deployments.
pub const SYSTEM_THEMES: &str = "/usr/share/annota/themes";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StylePaths {
    /// `~/.local/share/annota/styles`.
    pub data: PathBuf,
    /// `/usr/share/annota/themes`.
    pub system_themes: PathBuf,
}

/// Why importing a theme failed.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("cannot read the file: {0}")]
    Read(io::Error),
    #[error("not a valid theme: {0}")]
    Invalid(ThemeError),
    #[error("cannot save the theme: {0}")]
    Write(io::Error),
}

impl StylePaths {
    /// The standard places, for a user data directory such as `~/.local/share`.
    pub fn new(user_data_dir: &Path) -> Self {
        StylePaths {
            data: user_data_dir.join("annota").join("styles"),
            system_themes: PathBuf::from(SYSTEM_THEMES),
        }
    }

    pub fn library_file(&self) -> PathBuf {
        self.data.join(library::FILE_NAME)
    }

    pub fn user_themes(&self) -> PathBuf {
        self.data.join("themes")
    }

    /// Reads the library; none yet is an empty one. A file of a newer major version gives a
    /// read-only library; a file that is not a library is moved aside to
    /// `styles.json.broken` (and replaced by an empty library) instead of being overwritten.
    pub fn load_library(&self) -> Library {
        let path = self.library_file();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Library::default(),
            Err(err) => {
                eprintln!("annota: cannot read {}: {err}", path.display());
                return Library::read_only();
            }
        };
        match Library::parse(&text) {
            Ok(library) => library,
            Err(Unreadable::Newer(version)) => {
                eprintln!(
                    "annota: {} is from a newer Annota (format {version}); quick styles are not saved",
                    path.display()
                );
                Library::read_only()
            }
            Err(Unreadable::Broken) => {
                let aside = path.with_extension("json.broken");
                eprintln!(
                    "annota: {} is not a quick style library; moved to {}",
                    path.display(),
                    aside.display()
                );
                match fs::rename(&path, &aside) {
                    Ok(()) => Library::default(),
                    Err(_) => Library::read_only(),
                }
            }
        }
    }

    pub fn save_library(&self, library: &Library) -> io::Result<()> {
        if library.read_only {
            return Ok(());
        }
        write_atomically(&self.library_file(), library.to_json().as_bytes())
    }

    /// Every theme: the built-in ones, the administrator's (by name), the user's (by name).
    pub fn load_themes(&self) -> Vec<Theme> {
        let mut themes = Theme::built_ins();
        let mut managed = read_dir_themes(&self.system_themes, |stem| {
            (format!("system/{stem}"), Origin::Managed)
        });
        managed.sort_by_key(|t| t.name.to_lowercase());
        themes.extend(managed);
        let mut mine = read_dir_themes(&self.user_themes(), |stem| {
            (format!("user/{stem}"), Origin::User(stem.to_string()))
        });
        mine.sort_by_key(|t| t.name.to_lowercase());
        themes.extend(mine);
        themes
    }

    /// A file name stem for a new theme called `name` that no user theme uses yet.
    fn free_stem(&self, name: &str) -> String {
        let base: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_lowercase().next().unwrap_or(c)
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let base = if base.is_empty() {
            "theme".to_string()
        } else {
            base
        };
        let dir = self.user_themes();
        let taken = |stem: &str| dir.join(format!("{stem}.{EXTENSION}")).exists();
        if !taken(&base) {
            return base;
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|s| !taken(s))
            .unwrap_or(base)
    }

    /// Writes `theme` to "My Themes". A theme that is not yet one of the user's (new,
    /// duplicated, imported) gets a file of its own, and its id and origin say so.
    pub fn save_theme(&self, theme: &mut Theme) -> io::Result<()> {
        let stem = match &theme.origin {
            Origin::User(stem) => stem.clone(),
            _ => {
                let stem = self.free_stem(&theme.name);
                theme.origin = Origin::User(stem.clone());
                theme.id = format!("user/{stem}");
                stem
            }
        };
        let path = self.user_themes().join(format!("{stem}.{EXTENSION}"));
        write_atomically(&path, theme.to_json().as_bytes())
    }

    /// Deletes one of the user's themes (built-in and managed themes cannot be deleted).
    pub fn delete_theme(&self, theme: &Theme) -> io::Result<()> {
        match &theme.origin {
            Origin::User(stem) => {
                fs::remove_file(self.user_themes().join(format!("{stem}.{EXTENSION}")))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "only the user's own themes can be deleted",
            )),
        }
    }

    /// Imports a `.annota-theme` file into "My Themes" (it is validated first; the file itself
    /// is not changed). The new theme is returned.
    pub fn import(&self, file: &Path) -> Result<Theme, ImportError> {
        let text = read_text(file).map_err(ImportError::Read)?;
        let mut theme =
            Theme::parse(&text, String::new(), Origin::BuiltIn).map_err(ImportError::Invalid)?;
        // Importing a theme of the same name again keeps both, told apart by name.
        let themes = self.load_themes();
        let taken = |name: &str| themes.iter().any(|t| t.name == name);
        if taken(&theme.name) {
            let base = theme.name.clone();
            if let Some(name) = (2..).map(|n| format!("{base} {n}")).find(|n| !taken(n)) {
                theme.name = name;
            }
        }
        // Not one of the user's yet: `save_theme` gives it a file of its own.
        self.save_theme(&mut theme).map_err(ImportError::Write)?;
        Ok(theme)
    }

    /// Writes `theme` to `file` (Export Theme…).
    pub fn export(theme: &Theme, file: &Path) -> io::Result<()> {
        write_atomically(file, theme.to_json().as_bytes())
    }
}

/// Reads a file as text; a file that is not UTF-8 reads as "not JSON" later.
fn read_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8(bytes).unwrap_or_default())
}

/// Reads why `file` is not a theme, if it is not one (the import error alert's reason).
pub fn check(file: &Path) -> Result<Theme, ImportError> {
    let text = read_text(file).map_err(ImportError::Read)?;
    Theme::parse(&text, String::new(), Origin::BuiltIn).map_err(ImportError::Invalid)
}

fn read_dir_themes(dir: &Path, id: impl Fn(&str) -> (String, Origin)) -> Vec<Theme> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut themes = Vec::new();
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION) && p.is_file())
        .collect();
    paths.sort();
    for path in paths {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let (id, origin) = id(stem);
        match read_text(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| Theme::parse(&text, id, origin).map_err(|e| e.to_string()))
        {
            Ok(theme) => themes.push(theme),
            Err(err) => eprintln!("annota: skipping theme {}: {err}", path.display()),
        }
    }
    themes
}

fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no parent directory"))?;
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::tool::{Tool, Toolbox};
    use crate::styles::theme::{ThemeError, DEFAULT_ID};

    struct Tmp(PathBuf);

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn tmp(name: &str) -> (Tmp, StylePaths) {
        let dir = std::env::temp_dir().join(format!(
            "annota-styles-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let paths = StylePaths {
            data: dir.join("data"),
            system_themes: dir.join("system"),
        };
        (Tmp(dir), paths)
    }

    const HARBOR: &str = r##"{"format": "1.0", "requires": "0.1", "name": "Harbor Corporate",
        "colours": ["#14324F", "#3FA7D6", "#F0C808", "#D1495B"],
        "fonts": {"heading": "DejaVu Serif", "body": "DejaVu Sans"}}"##;

    #[test]
    fn library_round_trips_through_its_file() {
        let (_t, paths) = tmp("library");
        assert_eq!(paths.load_library(), Library::default(), "none yet");
        let mut lib = Library::default();
        lib.add(
            Tool::Arrow,
            "Teal bold",
            Toolbox::default().properties(Tool::Arrow),
        );
        lib.theme = "user/x".into();
        paths.save_library(&lib).unwrap();
        let back = paths.load_library();
        assert_eq!(back.styles(Tool::Arrow).len(), 1);
        assert_eq!(back.theme, "user/x");
    }

    #[test]
    fn newer_and_broken_libraries_are_not_destroyed() {
        let (_t, paths) = tmp("newer");
        fs::create_dir_all(&paths.data).unwrap();
        let newer = r#"{"format": "2.0", "styles": "elsewhere"}"#;
        fs::write(paths.library_file(), newer).unwrap();
        let lib = paths.load_library();
        assert!(lib.read_only);
        paths.save_library(&lib).unwrap();
        assert_eq!(fs::read_to_string(paths.library_file()).unwrap(), newer);

        fs::write(paths.library_file(), "garbage").unwrap();
        let lib = paths.load_library();
        assert!(!lib.read_only);
        assert_eq!(
            fs::read_to_string(paths.data.join("styles.json.broken")).unwrap(),
            "garbage"
        );
    }

    #[test]
    fn managed_themes_appear_read_only() {
        // Scenario "Managed theme": an admin installs a theme into the system directory; it
        // appears for every user, read-only.
        let (_t, paths) = tmp("managed");
        fs::create_dir_all(&paths.system_themes).unwrap();
        fs::write(paths.system_themes.join("harbor.annota-theme"), HARBOR).unwrap();
        fs::write(paths.system_themes.join("broken.annota-theme"), "{").unwrap();
        fs::write(paths.system_themes.join("notes.txt"), HARBOR).unwrap();
        let themes = paths.load_themes();
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, [DEFAULT_ID, "monochrome", "system/harbor"]);
        let harbor = &themes[2];
        assert_eq!(harbor.origin, Origin::Managed);
        assert!(!harbor.editable());
        assert!(paths.delete_theme(harbor).is_err());
        assert!(fs::metadata(paths.system_themes.join("harbor.annota-theme")).is_ok());
    }

    #[test]
    fn import_export_duplicate_and_delete() {
        let (t, paths) = tmp("import");
        let file = t.0.join("brand-2024.annota-theme");
        fs::write(&file, HARBOR).unwrap();
        let theme = paths.import(&file).unwrap();
        assert_eq!(theme.id, "user/harbor-corporate");
        assert!(theme.editable());
        // Importing it again keeps both, told apart by name.
        let again = paths.import(&file).unwrap();
        assert_eq!(again.id, "user/harbor-corporate-2");
        assert_eq!(again.name, "Harbor Corporate 2");
        let ids: Vec<String> = paths.load_themes().into_iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 4);
        // Export writes the same theme.
        let out = t.0.join("out.annota-theme");
        StylePaths::export(&theme, &out).unwrap();
        let back = check(&out).unwrap();
        assert_eq!(
            (back.name, back.colours),
            (theme.name.clone(), theme.colours.clone())
        );
        // Duplicating a managed theme makes an editable copy.
        let mut copy = Theme {
            name: "Harbor Corporate Copy".into(),
            origin: Origin::Managed,
            ..theme.clone()
        };
        paths.save_theme(&mut copy).unwrap();
        assert_eq!(copy.id, "user/harbor-corporate-copy");
        paths.delete_theme(&again).unwrap();
        assert_eq!(paths.load_themes().len(), 4);
    }

    #[test]
    fn invalid_imports_say_why() {
        let (t, paths) = tmp("invalid");
        let file = t.0.join("x.annota-theme");
        fs::write(&file, [0xFF, 0xD8, 0xFF]).unwrap();
        assert!(matches!(
            paths.import(&file),
            Err(ImportError::Invalid(ThemeError::NotJson))
        ));
        assert!(matches!(
            paths.import(&t.0.join("missing.annota-theme")),
            Err(ImportError::Read(_))
        ));
        assert!(!paths.user_themes().exists(), "nothing imported");
    }
}
