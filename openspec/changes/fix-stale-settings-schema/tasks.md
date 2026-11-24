# Tasks

## 1. Design gate

- [x] 1.1 Not applicable: no UI change. The fix only prints to stderr, like the existing
      gresource error (CLAUDE.md §5 bugfix path; design.md "Design gate"). No mockups, no
      approval needed.

## 2. Schema check

- [x] 2.1 Add the requirement "Settings schema check at startup" to `specs/app-shell/spec.md`
      before the code.
- [x] 2.2 `src/schema_check.rs`: expected keys from the shipped schema (D1, attribute-order-proof
      parser), `check` against a given `SettingsSchemaSource` (D2), `search_dirs` in GLib's order
      (D4), `find_schemas` (compiled cache and source file per directory), `report` with honest
      file labels (D2) and one remedy per situation (D5). Unit tests (temp dirs compiled with
      `glib-compile-schemas`, removed at test end):
      Stale schema shadows the installed one (`stale_schema_shadowing_the_installed_one_is_reported`),
      Compiled schema does not match its file (`outdated_compiled_cache_next_to_a_complete_file_is_recompiled`),
      Installed schema older than Annota (`stale_schema_without_a_complete_one_asks_for_a_reinstall`),
      Schema file not compiled (`uncompiled_complete_schema_file_is_compiled_not_removed`),
      Schema not installed (`schema_not_installed_is_reported`),
      Matching schema starts normally (`shipped_schema_passes`, `every_key_of_the_shipped_schema_is_expected`),
      Search path order (`search_path_follows_glib_order`),
      Version and help without a schema (`version_and_help_skip_the_check`).
- [x] 2.3 `src/main.rs`: `setlocale`, then the check before resource load and
      `application::run()` unless `skips_check` (D6); on a mismatch print the report and return
      `glib::ExitCode::FAILURE` (D3).
- [x] 2.4 Process tests `tests/schema_check.rs` (the real binary, clean environment, temp XDG
      data dirs): Stale schema shadows the installed one and Schema not installed end with status
      1 and no `GLib-GIO-ERROR`; Matching schema starts normally gets past the check; Version and
      help without a schema.
- [x] 2.5 Translate the report (maintainer decision 2026-11-24, D3): gettext for every line,
      German in `po/de.po`, `src/schema_check.rs` in `po/POTFILES.in`. Proves: Report in the
      user's language (`report_is_translated_under_language_de`, live run, own process).

- [x] 2.6 Reviewer pass 2: process test `report_is_german_under_language_de` (real binary,
      `bwrap` overlay of the fresh `de/annota.mo`, negative control without `setlocale` fails);
      remedy for a stale schema shadowing an uncompiled complete file (Stale schema shadows an
      uncompiled one, `stale_schema_shadowing_an_uncompiled_complete_file_names_both_directories`);
      `skips_check` skips the values of `-c/-r/-d/-o` (Version and help without a schema,
      `version_and_help_skip_the_check`); D3 locale side effect measured.

## 3. Verify

- [x] 3.1 Live check on the maintainer's machine with the real stale `/usr/local` schema
      (`manual-checks.md`): message + exit 1; with `XDG_DATA_DIRS=/usr/share` it starts;
      `--version`/`--help` work with the stale schema.
- [x] 3.2 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` green.
- [x] 3.3 `openspec validate --all --strict` passes.
- [x] 3.4 `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean.
      2026-11-24, by the reviewer (pass 2): make-deb.sh exit 0, `annota_0.2.0_amd64.deb`, lintian
      clean, appstream and desktop file valid.
- [x] 3.5 `reviewer` agent (fresh context): spec vs. tests, no blocking findings (2026-11-24).
      Pass 1 (2026-11-24): NOT READY - findings 1, 2, 4, 6, 7, 8, 9 addressed in this tree.
      Pass 2 (2026-11-24): NOT READY - blocker 1 (German proof) and findings 2, 3, 4, 5 addressed
      in task 2.6.
- [x] 3.6 `python3 tools/scenario_gate.py verify fix-stale-settings-schema` exits 0 on the final
      HEAD and `scenario-report.md` is committed.
