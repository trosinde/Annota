## ADDED Requirements

### Requirement: Settings schema check at startup
Before any window and before handling command-line options other than `--version` and `--help`, Annota SHALL check that the GSettings schema GLib resolves for `io.github.trosinde.Annota` has every key of the schema this build ships. If not, Annota SHALL NOT abort with a GLib error. It SHALL print to stderr, in the user's language, the missing keys, the schema files on the search path in GLib's lookup order and a remedy that fits what it found, then exit with status 1. No dialog.

#### Scenario: Stale schema shadows the installed one
- **WHEN** `/usr/local/share/glib-2.0/schemas/` holds a compiled schema `io.github.trosinde.Annota` without the key `shortcut-record`, `/usr/share/glib-2.0/schemas/` holds this build's schema compiled, and `XDG_DATA_DIRS` lists `/usr/local/share/` before `/usr/share/`
- **THEN** Annota prints the missing keys, `shortcut-record` among them, to stderr, every line starting with `annota: `
- **AND** it lists `/usr/local/share/glib-2.0/schemas/gschemas.compiled` as in use before `/usr/share/glib-2.0/schemas/gschemas.compiled` as shadowed and complete, each followed by the `io.github.trosinde.Annota.gschema.xml` next to it
- **AND** it tells the user to remove the stale `io.github.trosinde.Annota.gschema.xml` in `/usr/local/share/glib-2.0/schemas/` and run `glib-compile-schemas` on that directory
- **AND** it exits with status 1 without a `GLib-GIO-ERROR`

#### Scenario: Stale schema shadows an uncompiled one
- **WHEN** the schema in use lacks keys, no complete compiled schema exists on the search path, and a directory further down holds this build's `io.github.trosinde.Annota.gschema.xml`
- **THEN** Annota tells the user to remove the stale schema file and run `glib-compile-schemas` on both directories, naming each
- **AND** it does not tell them to remove the complete file or to reinstall Annota

#### Scenario: Compiled schema does not match its file
- **WHEN** the `gschemas.compiled` in use lacks keys, and the `io.github.trosinde.Annota.gschema.xml` in the same directory has every key
- **THEN** Annota tells the user to run `glib-compile-schemas` on that directory, and does not tell them to remove any file

#### Scenario: Installed schema older than Annota
- **WHEN** the schema in use lacks keys and no complete schema exists anywhere on the search path
- **THEN** Annota tells the user to reinstall Annota, and does not tell them to remove any file

#### Scenario: Schema file not compiled
- **WHEN** a directory on the search path holds this build's `io.github.trosinde.Annota.gschema.xml`, but no `gschemas.compiled` on the search path holds the schema
- **THEN** Annota prints that the settings schema is not installed, lists that file as complete and not compiled
- **AND** it tells the user to run `glib-compile-schemas` on that directory, and does not tell them to remove any file

#### Scenario: Schema not installed
- **WHEN** no directory on the schema search path holds a compiled schema `io.github.trosinde.Annota` or an `io.github.trosinde.Annota.gschema.xml`
- **THEN** Annota prints to stderr that the settings schema `io.github.trosinde.Annota` is not installed and tells the user to reinstall Annota
- **AND** it exits with status 1 without a `GLib-GIO-ERROR`

#### Scenario: Matching schema starts normally
- **WHEN** the schema GLib resolves for `io.github.trosinde.Annota` has every key of this build's schema
- **THEN** Annota prints nothing about the schema and starts as before

#### Scenario: Version and help without a schema
- **WHEN** no schema `io.github.trosinde.Annota` is installed and the user runs `annota --version` or `annota --help`
- **THEN** Annota prints its version or its usage and exits with status 0, without a schema message
- **AND** `annota --open --version` is still checked, because `--version` is the value of `--open` there

#### Scenario: Report in the user's language
- **WHEN** the schema is not installed and Annota runs with `LANGUAGE=de`
- **THEN** the report is in German ("Das Einstellungsschema io.github.trosinde.Annota ist nicht installiert.")
- **AND** the `annota: ` prefix, file paths, key names and commands stay untranslated

#### Scenario: Search path order
- **WHEN** `GSETTINGS_SCHEMA_DIR` is `/a/schemas:/b/schemas`, the user data directory is `~/.local/share` and `XDG_DATA_DIRS` is `/usr/local/share/:/usr/share/`
- **THEN** Annota lists schema files found in `/a/schemas`, `/b/schemas`, `~/.local/share/glib-2.0/schemas`, `/usr/local/share/glib-2.0/schemas` and `/usr/share/glib-2.0/schemas`, in that order
