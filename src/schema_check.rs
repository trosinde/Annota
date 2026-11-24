//! Startup check of the GSettings schema (fix-stale-settings-schema).
//!
//! `gio::Settings::new` and `Settings::get` abort the whole process with a `GLib-GIO-ERROR` when
//! the schema is not installed or lacks a key. A stale `meson install` into `/usr/local` shadows
//! the packaged schema in `/usr/share`, because `XDG_DATA_DIRS` lists `/usr/local/share/` first.
//! `main` therefore checks the schema GLib resolves before any window opens, and on a mismatch
//! prints which keys are missing, which schema files lie on the search path (in lookup order) and
//! the remedy that fits what it found.
//!
//! The expected keys come from the schema file this build ships (`include_str!`), so the list can
//! never drift from the build.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

/// The schema source file name, as installed into `<data dir>/glib-2.0/schemas/`.
pub const SCHEMA_FILE: &str = "io.github.trosinde.Annota.gschema.xml";

/// The compiled schema cache GLib actually reads, one per schema directory.
pub const COMPILED_FILE: &str = "gschemas.compiled";

/// The schema this build ships and every other module reads through `gio::Settings`.
const SHIPPED_SCHEMA: &str = include_str!("../data/io.github.trosinde.Annota.gschema.xml");

/// What GLib resolves for the schema id, compared with the shipped schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaStatus {
    /// The resolved schema has every key of the shipped schema.
    Ok,
    /// No schema with this id is installed on the search path.
    NotInstalled,
    /// The resolved schema lacks these keys (in the shipped schema's order).
    MissingKeys(Vec<String>),
}

/// Options of `annota` that take a value in the next argument (`src/application.rs`
/// `add_options`). GOption takes that argument as the value even if it looks like an option, so
/// `annota --open --version` opens a file named `--version`.
const VALUE_OPTIONS: &[&str] = &[
    "--capture",
    "-c",
    "--record",
    "-r",
    "--delay",
    "-d",
    "--open",
    "-o",
];

/// `--version` (`-v`) and the GApplication help options are answered without any settings, so a
/// broken installation can still report its version and usage. Options after `--` are positional;
/// the value of a value-taking option is never read as an option.
pub fn skips_check<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args.into_iter().skip(1);
    while let Some(arg) = args.next() {
        let Some(arg) = arg.as_ref().to_str() else {
            continue;
        };
        if arg == "--" {
            return false;
        }
        if VALUE_OPTIONS.contains(&arg) {
            args.next();
            continue;
        }
        if matches!(arg, "--version" | "-v" | "-h" | "-?") || arg.starts_with("--help") {
            return true;
        }
    }
    false
}

/// The value of attribute `name` in the attribute text of one XML start tag.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let preceded_by_space = rest[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
        let after = rest[at + name.len()..].trim_start();
        if preceded_by_space {
            if let Some(value) = after.strip_prefix('=') {
                let value = value.trim_start();
                if let Some(quote) = value.chars().next().filter(|c| *c == '"' || *c == '\'') {
                    let value = &value[1..];
                    return value.find(quote).map(|end| &value[..end]);
                }
            }
        }
        rest = &rest[at + name.len()..];
    }
    None
}

/// Every start tag `<element …>` in `xml`, as the text between the element name and `>`.
fn start_tags<'a>(xml: &'a str, element: &'a str) -> impl Iterator<Item = (usize, &'a str)> + 'a {
    let open = format!("<{element}");
    xml.match_indices(open.clone().as_str())
        .map(|(at, _)| at)
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(move |at| {
            let after = &xml[at + open.len()..];
            // `<keys…` or `<schemalist` are other elements.
            if !after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
                return None;
            }
            let end = after.find('>')?;
            Some((at, &after[..end]))
        })
}

/// The key names of schema `schema_id` in a gschema XML document, in file order. Attribute order
/// and quoting do not matter.
fn keys_in(xml: &str, schema_id: &str) -> Vec<String> {
    let Some((start, _)) =
        start_tags(xml, "schema").find(|(_, tag)| attribute(tag, "id") == Some(schema_id))
    else {
        return Vec::new();
    };
    let body = &xml[start..];
    let body = match body.find("</schema>") {
        Some(end) => &body[..end],
        None => body,
    };
    start_tags(body, "key")
        .filter_map(|(_, tag)| attribute(tag, "name"))
        .map(str::to_owned)
        .collect()
}

/// Every key of `schema_id` in the schema this build ships.
pub fn expected_keys(schema_id: &str) -> Vec<String> {
    keys_in(SHIPPED_SCHEMA, schema_id)
}

/// Checks the schema `source` resolves for `schema_id` against `expected`.
pub fn check(
    source: Option<&gio::SettingsSchemaSource>,
    schema_id: &str,
    expected: &[String],
) -> SchemaStatus {
    let Some(schema) = source.and_then(|source| source.lookup(schema_id, true)) else {
        return SchemaStatus::NotInstalled;
    };
    let missing: Vec<String> = expected
        .iter()
        .filter(|key| !schema.has_key(key))
        .cloned()
        .collect();
    if missing.is_empty() {
        SchemaStatus::Ok
    } else {
        SchemaStatus::MissingKeys(missing)
    }
}

/// GLib's schema directories in lookup order: every `GSETTINGS_SCHEMA_DIR` entry, then
/// `<user data dir>/glib-2.0/schemas`, then `<system data dir>/glib-2.0/schemas` for each
/// `XDG_DATA_DIRS` entry. The first directory whose compiled cache holds the schema wins.
pub fn search_dirs(
    gsettings_schema_dir: Option<&str>,
    user_data_dir: &Path,
    system_data_dirs: &[PathBuf],
) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = gsettings_schema_dir
        .map(|value| {
            value
                .split(':')
                .filter(|entry| !entry.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    dirs.push(user_data_dir.join("glib-2.0").join("schemas"));
    dirs.extend(
        system_data_dirs
            .iter()
            .map(|dir| dir.join("glib-2.0").join("schemas")),
    );
    dirs
}

/// The schema directories of this process, in GLib's lookup order.
pub fn process_search_dirs() -> Vec<PathBuf> {
    let env = std::env::var("GSETTINGS_SCHEMA_DIR").ok();
    search_dirs(
        env.as_deref(),
        &glib::user_data_dir(),
        &glib::system_data_dirs(),
    )
}

/// The schema source file (`io.github.trosinde.Annota.gschema.xml`) in one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// No such file.
    Absent,
    /// The file exists but cannot be read.
    Unreadable,
    /// The file defines the schema and lacks these expected keys (empty = complete).
    Missing(Vec<String>),
}

impl Source {
    fn is_complete(&self) -> bool {
        matches!(self, Source::Missing(keys) if keys.is_empty())
    }
}

/// What one schema directory holds for the schema id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub dir: PathBuf,
    /// `None` if the directory's `gschemas.compiled` does not hold the schema (or is missing);
    /// otherwise the expected keys the compiled schema lacks (empty = complete). Only this file
    /// counts for GLib.
    pub compiled: Option<Vec<String>>,
    /// The source file next to it, which GLib ignores at runtime.
    pub source: Source,
}

/// Every directory of `dirs` with a compiled schema or a source file for `schema_id`.
pub fn find_schemas(dirs: &[PathBuf], schema_id: &str, expected: &[String]) -> Vec<Found> {
    let mut found = Vec::new();
    for dir in dirs {
        let compiled = dir
            .to_str()
            .and_then(|path| gio::SettingsSchemaSource::from_directory(path, None, false).ok())
            .and_then(|source| match check(Some(&source), schema_id, expected) {
                SchemaStatus::Ok => Some(Vec::new()),
                SchemaStatus::MissingKeys(keys) => Some(keys),
                SchemaStatus::NotInstalled => None,
            });
        let xml = dir.join(SCHEMA_FILE);
        let source = if !xml.is_file() {
            Source::Absent
        } else {
            match std::fs::read_to_string(&xml) {
                Ok(text) => {
                    let keys = keys_in(&text, schema_id);
                    Source::Missing(
                        expected
                            .iter()
                            .filter(|key| !keys.contains(key))
                            .cloned()
                            .collect(),
                    )
                }
                Err(_) => Source::Unreadable,
            }
        };
        if compiled.is_none() && source == Source::Absent {
            continue;
        }
        found.push(Found {
            dir: dir.clone(),
            compiled,
            source,
        });
    }
    found
}

/// `template` with every `{name}` replaced. Paths, key names and commands are filled in after
/// translation, so they never reach a translator.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("{{{name}}}"), value)
        })
}

fn completeness(missing: &[String]) -> String {
    if missing.is_empty() {
        // Translators: state of a GSettings schema file in Annota's startup schema report.
        gettext("complete")
    } else {
        let n = u32::try_from(missing.len()).unwrap_or(u32::MAX);
        fill(
            // Translators: state of a GSettings schema file; {n} is a number of settings keys.
            &ngettext("lacks {n} key", "lacks {n} keys", n),
            &[("n", &missing.len().to_string())],
        )
    }
}

fn compile_command(dir: &Path) -> String {
    format!("glib-compile-schemas {}", dir.display())
}

/// The remedy line(s) for what `found` holds. Each one is true for every situation that reaches
/// it (CLAUDE.md §5): it never tells the user to delete a complete schema.
fn remedy(found: &[Found]) -> Vec<String> {
    let in_use = found.iter().position(|entry| entry.compiled.is_some());
    let Some(index) = in_use else {
        // Nothing compiled holds the schema.
        return match found.iter().find(|entry| entry.source.is_complete()) {
            Some(entry) => vec![fill(
                // Translators: {command} is a shell command, keep the backticks around it.
                &gettext("compile the complete schema file: run `{command}` (as root for a system directory)."),
                &[("command", &compile_command(&entry.dir))],
            )],
            None => vec![gettext(
                "no complete schema file of this version was found: reinstall Annota.",
            )],
        };
    };
    let entry = &found[index];
    let dir = entry.dir.display().to_string();
    let command = compile_command(&entry.dir);
    let compiled = entry.dir.join(COMPILED_FILE).display().to_string();
    if entry.source.is_complete() {
        return vec![fill(
            // Translators: {compiled} is a file path, {file} a file name, {command} a shell
            // command; keep the backticks.
            &gettext("{compiled} does not match the complete {file} next to it: run `{command}` (as root for a system directory)."),
            &[
                ("compiled", &compiled),
                ("file", SCHEMA_FILE),
                ("command", &command),
            ],
        )];
    }
    let complete_later = found[index + 1..]
        .iter()
        .any(|later| later.compiled.as_ref().is_some_and(|keys| keys.is_empty()));
    if !complete_later {
        // A complete source file further down that GLib does not use yet: compile it, after the
        // stale schema in front of it is gone.
        let Some(later) = found[index + 1..]
            .iter()
            .find(|later| later.source.is_complete())
        else {
            return vec![gettext(
                "no complete schema of this version is installed: reinstall Annota.",
            )];
        };
        let later_file = later.dir.join(SCHEMA_FILE).display().to_string();
        let later_command = compile_command(&later.dir);
        return match entry.source {
            Source::Absent => vec![fill(
                // Translators: {compiled} and {later_file} are file paths, {command} and
                // {later_command} shell commands; keep the backticks.
                &gettext("the stale schema in {compiled} has no source file left and shadows the complete schema file {later_file}, which GLib does not use yet: run `{command}`, then `{later_command}` (as root for a system directory)."),
                &[
                    ("compiled", &compiled),
                    ("later_file", &later_file),
                    ("command", &command),
                    ("later_command", &later_command),
                ],
            )],
            _ => vec![fill(
                // Translators: {dir} is a directory, {file} and {later_file} file paths,
                // {command} and {later_command} shell commands; keep the backticks.
                &gettext("the stale schema in {dir} shadows the complete schema file {later_file}, which GLib does not use yet: remove {file}, then run `{command}` and `{later_command}` (as root for a system directory)."),
                &[
                    ("dir", &dir),
                    ("file", &entry.dir.join(SCHEMA_FILE).display().to_string()),
                    ("later_file", &later_file),
                    ("command", &command),
                    ("later_command", &later_command),
                ],
            )],
        };
    }
    match entry.source {
        Source::Absent => vec![fill(
            // Translators: {compiled} is a file path, {command} a shell command; keep the
            // backticks.
            &gettext("the stale schema in {compiled} has no source file left: run `{command}` (as root for a system directory) to drop it."),
            &[("compiled", &compiled), ("command", &command)],
        )],
        _ => vec![fill(
            // Translators: {dir} is a directory, {file} a file path, {command} a shell command;
            // keep the backticks and `meson install`.
            &gettext("the stale schema in {dir} shadows the complete one further down (often left by an old `meson install` into /usr/local): remove {file}, then run `{command}` (as root for a system directory)."),
            &[
                ("dir", &dir),
                ("file", &entry.dir.join(SCHEMA_FILE).display().to_string()),
                ("command", &command),
            ],
        )],
    }
}

/// The message printed to stderr for a status other than `Ok`; `None` for `Ok`. Every line
/// starts with `annota: ` (untranslated, like the other startup errors).
pub fn report(status: &SchemaStatus, schema_id: &str, found: &[Found]) -> Option<String> {
    let mut lines = Vec::new();
    match status {
        SchemaStatus::Ok => return None,
        SchemaStatus::NotInstalled => {
            lines.push(fill(
                // Translators: {schema} is a GSettings schema id.
                &gettext("the settings schema {schema} is not installed."),
                &[("schema", schema_id)],
            ));
        }
        SchemaStatus::MissingKeys(keys) => {
            lines.push(fill(
                // Translators: {schema} is a GSettings schema id.
                &gettext(
                    "the settings schema {schema} in use does not match this version of Annota.",
                ),
                &[("schema", schema_id)],
            ));
            lines.push(fill(
                // Translators: {keys} is a comma-separated list of settings key names.
                &gettext("missing keys: {keys}"),
                &[("keys", &keys.join(", "))],
            ));
        }
    }
    if found.is_empty() {
        lines.push(fill(
            // Translators: {file} is a file name, {schema} a GSettings schema id.
            &gettext("no {file} and no compiled schema {schema} on the schema search path."),
            &[("file", SCHEMA_FILE), ("schema", schema_id)],
        ));
    } else {
        lines.push(fill(
            // Translators: {compiled} is the file name gschemas.compiled.
            &gettext("schema files on the search path, in lookup order (GLib uses the first {compiled} that holds the schema):"),
            &[("compiled", COMPILED_FILE)],
        ));
        let mut in_use_seen = false;
        for entry in found {
            let dir = &entry.dir;
            if let Some(missing) = &entry.compiled {
                let role = if in_use_seen {
                    // Translators: a compiled schema that GLib does not use, because an earlier
                    // one on the search path wins.
                    gettext("shadowed")
                } else {
                    // Translators: the compiled schema GLib uses.
                    gettext("in use")
                };
                in_use_seen = true;
                lines.push(format!(
                    "  {} ({role}, {})",
                    dir.join(COMPILED_FILE).display(),
                    completeness(missing)
                ));
            }
            let mut notes = vec![
                // Translators: the schema source file (.gschema.xml), as opposed to the compiled one.
                gettext("source"),
            ];
            match &entry.source {
                Source::Absent => continue,
                // Translators: a schema file that cannot be read.
                Source::Unreadable => notes.push(gettext("unreadable")),
                Source::Missing(missing) => notes.push(completeness(missing)),
            }
            if entry.compiled.is_none() {
                // Translators: a schema source file that no gschemas.compiled holds.
                notes.push(gettext("not compiled"));
            }
            lines.push(format!(
                "  {} ({})",
                dir.join(SCHEMA_FILE).display(),
                notes.join(", ")
            ));
        }
    }
    lines.extend(remedy(found));
    let mut out = String::new();
    for line in lines {
        let _ = writeln!(out, "annota: {line}");
    }
    Some(out)
}

/// Checks this process's schema; `Err(message)` if Annota must not start.
pub fn check_process(schema_id: &str) -> Result<(), String> {
    let expected = expected_keys(schema_id);
    let source = gio::SettingsSchemaSource::default();
    let status = check(source.as_ref(), schema_id, &expected);
    if status == SchemaStatus::Ok {
        return Ok(());
    }
    let found = find_schemas(&process_search_dirs(), schema_id, &expected);
    Err(report(&status, schema_id, &found).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "io.github.trosinde.Annota";

    /// A schema as an `meson install` of 2026-10-30 left it: no recording keys.
    const STALE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<schemalist gettext-domain="annota">
  <schema id="io.github.trosinde.Annota" path="/io/github/trosinde/Annota/">
    <key name="save-folder" type="s"><default>''</default></key>
    <key name="shortcut-region" type="s"><default>'Print'</default></key>
  </schema>
</schemalist>
"#;

    const OTHER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<schemalist>
  <schema id="org.example.Other" path="/org/example/Other/">
    <key name="flag" type="b"><default>false</default></key>
  </schema>
</schemalist>
"#;

    /// Temp directories removed when the test ends (also on failure).
    struct TempDirs(Vec<PathBuf>);

    impl TempDirs {
        fn new() -> Self {
            Self(Vec::new())
        }

        fn dir(&mut self, name: &str) -> PathBuf {
            let dir = std::env::temp_dir()
                .join(format!("annota-schema-check-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            self.0.push(dir.clone());
            dir
        }

        /// A schema directory holding `files`, compiled with `glib-compile-schemas`.
        fn compiled(&mut self, name: &str, files: &[(&str, &str)]) -> PathBuf {
            let dir = self.dir(name);
            for (file, xml) in files {
                std::fs::write(dir.join(file), xml).unwrap();
            }
            compile(&dir);
            dir
        }
    }

    impl Drop for TempDirs {
        fn drop(&mut self) {
            for dir in &self.0 {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    fn compile(dir: &Path) {
        let status = std::process::Command::new("glib-compile-schemas")
            .arg(dir)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn shipped() -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join(SCHEMA_FILE);
        std::fs::read_to_string(path).unwrap()
    }

    fn source(dir: &Path, parent: Option<&gio::SettingsSchemaSource>) -> gio::SettingsSchemaSource {
        gio::SettingsSchemaSource::from_directory(dir.to_str().unwrap(), parent, false).unwrap()
    }

    fn assert_annota_lines(text: &str) {
        assert!(text.lines().all(|line| line.starts_with("annota: ")));
    }

    // spec: app-shell / Settings schema check at startup / Matching schema starts normally
    #[test]
    fn shipped_schema_passes() {
        let mut tmp = TempDirs::new();
        let xml = shipped();
        let dir = tmp.compiled("shipped", &[(SCHEMA_FILE, &xml)]);
        let expected = expected_keys(ID);
        assert!(expected.iter().any(|key| key == "shortcut-record"));
        assert!(expected.iter().any(|key| key == "window-maximized"));
        let status = check(Some(&source(&dir, None)), ID, &expected);
        assert_eq!(status, SchemaStatus::Ok);
        assert_eq!(report(&status, ID, &[]), None);
    }

    // spec: app-shell / Settings schema check at startup / Matching schema starts normally
    #[test]
    fn every_key_of_the_shipped_schema_is_expected() {
        let xml = shipped();
        let key_elements = xml.matches("<key ").count();
        let expected = expected_keys(ID);
        assert_eq!(expected.len(), key_elements);
        // Attribute order and quoting do not matter; other ids and elements are not keys.
        let shuffled = r#"<schemalist><schema path="/x/" id='io.github.trosinde.Annota'>
            <key type="s" name='b-key'/><keys/><key
              name="c-key" type="b"></key></schema>
            <schema id="io.github.trosinde.Annota.Other"><key name="no"/></schema></schemalist>"#;
        assert_eq!(keys_in(shuffled, ID), vec!["b-key", "c-key"]);
    }

    // spec: app-shell / Settings schema check at startup / Stale schema shadows the installed one
    #[test]
    fn stale_schema_shadowing_the_installed_one_is_reported() {
        let mut tmp = TempDirs::new();
        let xml = shipped();
        // Lookup order as on the maintainer's machine: /usr/local/share before /usr/share.
        let stale = tmp.compiled("stale", &[(SCHEMA_FILE, STALE)]);
        let installed = tmp.compiled("installed", &[(SCHEMA_FILE, &xml)]);
        let chain = source(&stale, Some(&source(&installed, None)));
        let expected = expected_keys(ID);

        let status = check(Some(&chain), ID, &expected);
        let SchemaStatus::MissingKeys(missing) = &status else {
            panic!("expected missing keys, got {status:?}");
        };
        assert!(missing.iter().any(|key| key == "shortcut-record"));
        assert!(!missing.iter().any(|key| key == "shortcut-region"));

        let found = find_schemas(&[stale.clone(), installed.clone()], ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains("missing keys: "));
        assert!(text.contains("shortcut-record"));
        let n = expected.len() - 2;
        let stale_at = text
            .find(&format!(
                "{} (in use, lacks {n} keys)",
                stale.join(COMPILED_FILE).display()
            ))
            .unwrap();
        let installed_at = text
            .find(&format!(
                "{} (shadowed, complete)",
                installed.join(COMPILED_FILE).display()
            ))
            .unwrap();
        assert!(stale_at < installed_at);
        assert!(text.contains(&format!(
            "remove {}, then run `glib-compile-schemas {}`",
            stale.join(SCHEMA_FILE).display(),
            stale.display()
        )));
        assert!(!text.contains(&format!("remove {}", installed.display())));
        assert_annota_lines(&text);

        // Only the compiled cache is left of the stale schema: nothing to remove, recompile.
        std::fs::remove_file(stale.join(SCHEMA_FILE)).unwrap();
        let found = find_schemas(&[stale.clone(), installed], ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(!text.contains("remove "));
        assert!(text.contains(&format!("run `glib-compile-schemas {}`", stale.display())));
    }

    // spec: app-shell / Settings schema check at startup / Compiled schema does not match its file
    #[test]
    fn outdated_compiled_cache_next_to_a_complete_file_is_recompiled() {
        let mut tmp = TempDirs::new();
        let dir = tmp.compiled("outdated", &[(SCHEMA_FILE, STALE)]);
        std::fs::write(dir.join(SCHEMA_FILE), shipped()).unwrap();
        let expected = expected_keys(ID);
        let status = check(Some(&source(&dir, None)), ID, &expected);
        assert!(matches!(status, SchemaStatus::MissingKeys(_)));
        let found = find_schemas(std::slice::from_ref(&dir), ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains(&format!(
            "{} (source, complete)",
            dir.join(SCHEMA_FILE).display()
        )));
        assert!(text.contains(&format!("run `glib-compile-schemas {}`", dir.display())));
        assert!(!text.contains("remove "));
        assert!(!text.contains("reinstall"));
        assert_annota_lines(&text);
    }

    // spec: app-shell / Settings schema check at startup / Installed schema older than Annota
    #[test]
    fn stale_schema_without_a_complete_one_asks_for_a_reinstall() {
        let mut tmp = TempDirs::new();
        let dir = tmp.compiled("only-stale", &[(SCHEMA_FILE, STALE)]);
        let expected = expected_keys(ID);
        let status = check(Some(&source(&dir, None)), ID, &expected);
        let found = find_schemas(&[dir], ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains("missing keys: "));
        assert!(text.contains("reinstall Annota"));
        assert!(!text.contains("remove "));
        assert!(!text.contains("glib-compile-schemas"));
        assert_annota_lines(&text);
    }

    // spec: app-shell / Settings schema check at startup / Stale schema shadows an uncompiled one
    #[test]
    fn stale_schema_shadowing_an_uncompiled_complete_file_names_both_directories() {
        let mut tmp = TempDirs::new();
        let stale = tmp.compiled("shadows-uncompiled", &[(SCHEMA_FILE, STALE)]);
        let later = tmp.dir("uncompiled-later");
        std::fs::write(later.join(SCHEMA_FILE), shipped()).unwrap();
        let expected = expected_keys(ID);
        let status = check(Some(&source(&stale, None)), ID, &expected);
        let found = find_schemas(&[stale.clone(), later.clone()], ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains(&format!(
            "remove {}, then run `glib-compile-schemas {}` and `glib-compile-schemas {}`",
            stale.join(SCHEMA_FILE).display(),
            stale.display(),
            later.display()
        )));
        assert!(!text.contains(&format!("remove {}", later.display())));
        assert!(!text.contains("reinstall"));
        assert_annota_lines(&text);

        // Without a source file left, the stale directory is only recompiled.
        std::fs::remove_file(stale.join(SCHEMA_FILE)).unwrap();
        let found = find_schemas(&[stale.clone(), later.clone()], ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(!text.contains("remove "));
        assert!(text.contains(&format!(
            "run `glib-compile-schemas {}`, then `glib-compile-schemas {}`",
            stale.display(),
            later.display()
        )));
    }

    // spec: app-shell / Settings schema check at startup / Schema file not compiled
    #[test]
    fn uncompiled_complete_schema_file_is_compiled_not_removed() {
        let mut tmp = TempDirs::new();
        let dir = tmp.dir("uncompiled");
        std::fs::write(dir.join(SCHEMA_FILE), shipped()).unwrap();
        let expected = expected_keys(ID);
        let status = check(None, ID, &expected);
        assert_eq!(status, SchemaStatus::NotInstalled);
        let found = find_schemas(std::slice::from_ref(&dir), ID, &expected);
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains("is not installed"));
        assert!(text.contains(&format!(
            "{} (source, complete, not compiled)",
            dir.join(SCHEMA_FILE).display()
        )));
        assert!(text.contains(&format!("run `glib-compile-schemas {}`", dir.display())));
        assert!(!text.contains("remove "));
        assert!(!text.contains("stale"));
        assert_annota_lines(&text);
    }

    // spec: app-shell / Settings schema check at startup / Schema not installed
    #[test]
    fn schema_not_installed_is_reported() {
        let mut tmp = TempDirs::new();
        let dir = tmp.compiled("other", &[("org.example.Other.gschema.xml", OTHER)]);
        let expected = expected_keys(ID);
        let status = check(Some(&source(&dir, None)), ID, &expected);
        assert_eq!(status, SchemaStatus::NotInstalled);
        assert_eq!(check(None, ID, &expected), SchemaStatus::NotInstalled);

        let found = find_schemas(&[dir], ID, &expected);
        assert!(found.is_empty());
        let text = report(&status, ID, &found).unwrap();
        assert!(text.contains("is not installed"));
        assert!(text.contains(&format!("no {SCHEMA_FILE} and no compiled schema")));
        assert!(text.contains("reinstall Annota"));
        assert!(!text.contains("remove "));
        assert_annota_lines(&text);
    }

    // spec: app-shell / Settings schema check at startup / Search path order
    #[test]
    fn search_path_follows_glib_order() {
        let dirs = search_dirs(
            Some("/a/schemas:/b/schemas"),
            Path::new("/home/u/.local/share"),
            &[
                PathBuf::from("/usr/local/share/"),
                PathBuf::from("/usr/share/"),
            ],
        );
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/a/schemas"),
                PathBuf::from("/b/schemas"),
                PathBuf::from("/home/u/.local/share/glib-2.0/schemas"),
                PathBuf::from("/usr/local/share/glib-2.0/schemas"),
                PathBuf::from("/usr/share/glib-2.0/schemas"),
            ]
        );
        let without_env = search_dirs(None, Path::new("/h"), &[]);
        assert_eq!(without_env, vec![PathBuf::from("/h/glib-2.0/schemas")]);
    }

    // spec: app-shell / Settings schema check at startup / Report in the user's language
    #[test]
    #[ignore = "sets the process locale and text domain; runs in its own process (live run)"]
    fn report_is_translated_under_language_de() {
        use gettextrs::LocaleCategory;
        use gettextrs::{bind_textdomain_codeset, bindtextdomain, setlocale, textdomain};

        let mut tmp = TempDirs::new();
        let locale_dir = tmp.dir("locale");
        let messages = locale_dir.join("de").join("LC_MESSAGES");
        std::fs::create_dir_all(&messages).unwrap();
        let po = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("po")
            .join("de.po");
        let status = std::process::Command::new("msgfmt")
            .arg("-o")
            .arg(messages.join("annota.mo"))
            .arg(po)
            .status()
            .unwrap();
        assert!(status.success());

        // Edition 2021: setting the environment is safe here, the test runs alone in its process.
        std::env::set_var("LANGUAGE", "de");
        // LANGUAGE is ignored under C and C.UTF-8 (measured); any other installed locale will do.
        let locale = ["de_DE.UTF-8", "en_US.UTF-8"]
            .into_iter()
            .find(|name| setlocale(LocaleCategory::LcAll, *name).is_some());
        assert!(locale.is_some(), "no usable locale installed");
        bindtextdomain("annota", locale_dir.to_str().unwrap()).unwrap();
        bind_textdomain_codeset("annota", "UTF-8").unwrap();
        textdomain("annota").unwrap();

        let dir = tmp.dir("de-uncompiled");
        std::fs::write(dir.join(SCHEMA_FILE), shipped()).unwrap();
        let expected = expected_keys(ID);
        let found = find_schemas(std::slice::from_ref(&dir), ID, &expected);
        let text = report(&SchemaStatus::NotInstalled, ID, &found).unwrap();
        assert!(
            text.contains(
                "annota: Das Einstellungsschema io.github.trosinde.Annota ist nicht installiert."
            ),
            "{text}"
        );
        assert!(
            text.contains("(Quelle, vollständig, nicht kompiliert)"),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "Führen Sie `glib-compile-schemas {}` aus",
                dir.display()
            )),
            "{text}"
        );
        assert_annota_lines(&text);

        let missing = SchemaStatus::MissingKeys(vec!["shortcut-record".to_owned()]);
        let text = report(&missing, ID, &[]).unwrap();
        assert!(
            text.contains("annota: Fehlende Schlüssel: shortcut-record"),
            "{text}"
        );
    }

    // spec: app-shell / Settings schema check at startup / Version and help without a schema
    #[test]
    fn version_and_help_skip_the_check() {
        for args in [
            &["annota", "--version"][..],
            &["annota", "-v"],
            &["annota", "--help"],
            &["annota", "-h"],
            &["annota", "--help-all"],
            &["annota", "--capture", "region", "--version"],
            &["annota", "--open=x.png", "--version"],
            &["annota", "--open", "x.png", "-v"],
            // GOption takes `--` as the value of `--open` (measured 2026-11-24).
            &["annota", "--open", "--", "--version"],
        ] {
            assert!(skips_check(args), "{args:?}");
        }
        for args in [
            &["annota"][..],
            &["annota", "--capture", "region"],
            &["annota", "--", "--version"],
            &["annota", "--versions"],
            &["annota", "--open", "--version"],
            &["annota", "-o", "-h"],
            &["annota", "--capture", "--help"],
            &["annota", "-c", "-v"],
            &["annota", "--record", "--version"],
            &["annota", "-r", "--help-all"],
            &["annota", "--delay", "-v"],
            &["annota", "-d", "-h"],
        ] {
            assert!(!skips_check(args), "{args:?}");
        }
    }
}
