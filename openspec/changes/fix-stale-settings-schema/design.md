# Design

## Context

See proposal.md, "Why". `gio::Settings::new` aborts on an unknown schema id and `Settings::get`
aborts on an unknown key (`g_error`, not a recoverable `GError`). GLib resolves a schema id in its
default `SettingsSchemaSource`, built from these directories, first match wins:
each `GSETTINGS_SCHEMA_DIR` entry, `$XDG_DATA_HOME/glib-2.0/schemas`, then
`<dir>/glib-2.0/schemas` for each `XDG_DATA_DIRS` entry. Only `gschemas.compiled` counts; the
`.gschema.xml` next to it is the source and is ignored at runtime.

On the maintainer's machine (2026-11-24) `XDG_DATA_DIRS` is
`/usr/share/ubuntu:/usr/share/gnome:/usr/local/share/:/usr/share/:/var/lib/snapd/desktop`.
`/usr/local/share/glib-2.0/schemas/` holds the schema XML from 2026-10-30 and a
`gschemas.compiled` from 2026-11-14 that lacks 10 keys.

**Wayland vs X11.** No difference: the check runs before GTK connects to a display.

## Design gate

Not applicable. The fix adds no UI: its only output is text on stderr, like the existing
"cannot load resources.gresource" error in `src/main.rs`. This is the CLAUDE.md §5 bugfix path
(no visible UI change, scenario added to the spec first, tagged tests). No mockups.

## Decisions

### D1: Expected keys come from the shipped schema

`include_str!("../data/io.github.trosinde.Annota.gschema.xml")`; the key names are the
`<key name="…">` entries inside `<schema id="io.github.trosinde.Annota">`. A key added to the
schema is checked from the same build on; no hand-kept list can drift. Only key presence is
checked, not types or defaults (out of scope).

*Alternative.* Catch the abort: impossible, `g_error` is fatal.

### D2: Check the default source, then explain from the directories

`check(source, id, expected)` takes a `SettingsSchemaSource` so tests pass one built with
`SettingsSchemaSource::from_directory` (chained with a parent to model shadowing). Production
passes `SettingsSchemaSource::default()` (`None` = nothing installed). Only on a mismatch does
`find_schemas` walk the search path (D4). For each directory it opens `gschemas.compiled` on its
own and parses the `io.github.trosinde.Annota.gschema.xml` next to it (same parser as D1).

The report labels files honestly (reviewer finding 8): GLib reads only `gschemas.compiled`, so
the compiled file is the one marked "in use" or "shadowed", with "complete" or "lacks N keys".
The source file is listed beneath it as "source", with its own completeness, and "not compiled"
when no compiled cache in its directory holds the schema. The two can differ, and the remedy
depends on which differs.

### D3: stderr, translated, exit status 1

`eprint!` with the untranslated `annota: ` prefix, as the gresource error, then
`glib::ExitCode::FAILURE`. No dialog (no working schema, no UI).

Every line of the report goes through gettext (`gettext`/`ngettext` with `{name}` placeholders,
filled after translation), so file paths, key names, schema ids and commands are never
translated. Maintainer decided 2026-11-24: translated. German is in `po/de.po`,
`src/schema_check.rs` is in `po/POTFILES.in`. `main` now calls `setlocale(LC_ALL, "")` before the
check: until GTK starts, the process runs in the C locale and gettext would ignore the user's
language. GTK calls the same `setlocale` itself later. Side effect: GOption parsing and
`handle_local_options` now also run in the user's locale, so GOption's own help texts ("Usage:",
"Help Options:") are localized where a `glib20.mo` for the language is installed. Measured
2026-11-24: `annota --help` under `LANG=C` and `LANG=en_US.UTF-8` is byte-identical, under
`LANGUAGE=de LANG=en_US.UTF-8` still English (no German `glib20.mo` on this machine), and the
"invalid capture mode" message is unchanged (Annota's own option texts are not translated).

A real-binary German run needs the new `annota.mo` in the binary's `LOCALEDIR`. The process test
`report_is_german_under_language_de` overlays it with `bwrap` in a private mount namespace (no
root) and skips where `bwrap` or a non-C locale is unavailable: under `C`/`C.UTF-8` gettext ignores
`LANGUAGE` (measured). It fails when `setlocale` is removed from `main` (negative control run
2026-11-24).

### D4: Search path in GLib's order

`search_dirs(GSETTINGS_SCHEMA_DIR, user_data_dir, system_data_dirs)` is pure and unit-tested;
`process_search_dirs()` feeds it `glib::user_data_dir()` and `glib::system_data_dirs()`, the same
values GLib itself uses. `GSETTINGS_SCHEMA_DIR` is split on `:`.

### D5: One remedy per situation, each true for every cause that reaches it

Reviewer finding 1: the first version always said "remove the stale schema file", which told a
user with only an uncompiled but correct file to delete it. `remedy(found)` now decides from what
it found (CLAUDE.md §5: a message must be true for every cause that reaches it):

| Found | Remedy |
|---|---|
| No compiled schema; a complete source file in some directory | run `glib-compile-schemas <that dir>` |
| No compiled schema; no complete source file | reinstall Annota |
| Compiled schema in use lacks keys; its own source file is complete | run `glib-compile-schemas <dir>` (cache out of date) |
| Compiled schema in use lacks keys; a complete compiled schema is shadowed further down; source file present | remove `<dir>/io.github.trosinde.Annota.gschema.xml`, then `glib-compile-schemas <dir>` |
| Same, but the stale schema has no source file left | `glib-compile-schemas <dir>` (drops it) |
| Compiled schema in use lacks keys; no complete compiled schema, but a complete source file further down | remove the stale `<dir>/io.github.trosinde.Annota.gschema.xml` (if present), then `glib-compile-schemas <dir>` and `glib-compile-schemas <later dir>`, both named |
| Compiled schema in use lacks keys; no complete compiled schema or source file anywhere | reinstall Annota |

"Remove" is only said when a complete schema - compiled, or a complete source file the remedy
also tells the user to compile - exists further down the path. The complete file is never the
one named for removal, so following the advice can never leave the user without a schema. Each row has a unit test. Commands carry
"(as root for a system directory)".

### D6: `--version` and `--help` skip the check

Reviewer finding 6: `--version` is answered in `handle_local_options` (`src/application.rs`)
before any `Settings` use, and GApplication answers `--help`/`-h`/`-?`/`--help-*` itself, so
neither needs the schema. GOption takes the next argument as the value of `-c/--capture`,
`-r/--record`, `-d/--delay` and `-o/--open` even when it looks like an option, `--` included
(measured 2026-11-24: `annota --open -- --version` prints the version). `skips_check` skips that
value the same way (reviewer pass 2, finding 3), so `annota --open --version` is checked. `skips_check(args)` exempts them (and `-v`), stopping at `--`, so a
broken installation still reports its version and usage. The gresource load that follows still
runs for them, as before.

Every other option goes through the check first. Under a stale schema `annota --capture banana`
therefore exits 1 with the schema report, not 2 with the valid modes: the "Invalid mode" scenario
(`app-shell` / "Command-line options") holds only with a working schema. Accepted: the schema
problem is the one the user must fix first, and checking it before option validation keeps one
rule for every option that may reach `Settings`.

## Risks

- A schema with all keys but changed types still aborts later. Accepted (out of scope); key
  presence covers the measured defect class, a stale install from an older version.
- Cost at startup: one schema lookup and one `has_key` call per key (30); the directory walk runs only on a
  mismatch.
- A development build whose schema gained a key that is not yet installed now refuses to start at
  all; before, it aborted only when that key was read. Developers run with the build's schema
  installed or `GSETTINGS_SCHEMA_DIR` pointing at a compiled copy, as the live tests do.
