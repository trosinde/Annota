# Tasks

## 1. Design gate

- [x] 1.1 `designer` agent: mockups 01–05 of design.md's "Mockups" table in
      `docs/design/fix-portal-shortcut-bindings/` (HTML source + PNG, light + dark, 1× + 2×) and an
      `index.md` mapping every scenario of both spec deltas to a mockup or to "live check". Base:
      add-screen-capture 01/09/11, bootstrap-app-shell 01. Brief: the portal path must read as
      "GNOME owns these keys" at a glance, with no chevrons, no editable look and one clear way out
      ("Open GNOME Settings"). Done when the PNGs exist and render with
      `python3 tools/render-mockups.py docs/design/fix-portal-shortcut-bindings`.
- [x] 1.2 Link each mockup in design.md's "Mockups" table (file paths). Done when every scenario in
      `specs/` appears in it or in `index.md`.
- [x] 1.3 Maintainer approval recorded in design.md (`Design approved: <date>`). No code before.

- [x] 1.4 `designer`: mockup 06 for D7 (trigger taken by GNOME) on Preferences, launcher and
      welcome; link it in design.md; maintainer approval of 06 recorded. No D7 UI code before.

## 2. Portal measurements (before building on them, CLAUDE.md §5)

- [x] 2.1 Extend `tools/portal-probe.py` with a `--global-shortcuts` mode. It registers as
      `io.github.trosinde.Annota`, binds the three IDs, prints the response and `ListShortcuts`,
      and closes the session. Nothing is stored. Run it under `LANG=en_US.UTF-8` and
      `LANG=de_DE.UTF-8` and record the exact `trigger_description` strings in design.md D3.
- [x] 2.2 Find the GNOME Settings entry point that lists Annota's portal shortcuts
      (gnome-control-center 50.3: panel name / `launch-panel` argument, and where the list sits).
      Record it in design.md D4 and close its Open Question. Verify by launching it once.
- [x] 2.3 With the probe's session open, rebind one Annota shortcut in GNOME Settings. Record
      whether `ShortcutsChanged` arrives and whether `ListShortcuts` reports the new trigger
      (design.md D2).
- [x] 2.4 Record which of GNOME Shell (`screenshot`) and Annota (`screen`) reacts to `Shift+Print`
      when both bind it on GNOME 50. Record it in design.md. The spec wording stays
      "depends on the compositor".

## 3. Bound-trigger store (design.md D1–D3)

- [ ] 3.1 `run_portal_shortcuts` keeps `bind_shortcuts`' response in `BOUND` (id → trigger
      description) and notifies `connect_bound_changed` subscribers. It also listens to
      `receive_shortcuts_changed` and updates `BOUND`. Verify: `live_tests.rs` case where the fake
      portal answers the bind with a trigger different from the preferred one, and `BOUND` holds
      the portal's.
- [ ] 3.2 `refresh_bound()`: `list_shortcuts` on the live session, then replace and notify.
      No-op without a session. Verify: live test, fake portal changes its answer between bind and
      list, and the store follows.
- [ ] 3.3 `shown_trigger(mode) -> ShownTrigger { Pending, NotSet, Accel, Text }`: portal path
      from `BOUND`, fallback path from `shortcut-*`. Verify: unit tests for `Press <Shift>Print` →
      `Accel("<Shift>Print")`, empty → `NotSet`, unparsable text → `Text` verbatim, no answer yet →
      `Pending`, fallback → setting.
- [ ] 3.4 Regression test for the reported defect: settings `<Control>q`, fake portal binds
      `screen` to `<Shift>Print`, so `shown_trigger(Screen)` is `Accel("<Shift>Print")` and never
      `<Control>q`.

- [ ] 3.5 D7: `taken_by` lookup over GNOME's public keybinding schemas, shared with
      `find_conflict`. Verify: unit tests - `<Shift>Print` against a list holding "Take a
      screenshot" gives that name; a free trigger gives none; Annota's own fallback custom
      keybindings never count.

## 4. UI surfaces

- [ ] 4.1 Preferences › Capture (`src/preferences.rs`, `data/ui/preferences.blp`): on the portal
      path, rows read-only (no chevron, not activatable), group description and an "Open GNOME
      Settings" header button (D4). A row shows "Not set" or no trigger while `Pending`.
      `refresh_bound()` runs on `map`. Matches mockups 02/03.
- [ ] 4.2 "Open GNOME Settings" opens the entry point found in 2.2. It is hidden when
      `org.gnome.Settings.desktop` is not installed. Verify by a manual check row.
- [ ] 4.3 Launcher popover (`application.rs` `setup_launcher`): per-mode keycaps and the Capture
      button's accelerator from `shown_trigger`. `refresh_bound()` runs on popover `show`.
      Matches mockup 01.
- [ ] 4.4 Welcome cards (`application.rs` `setup_welcome`): keycaps from `shown_trigger`. They
      update on `connect_bound_changed` and on `shortcut-*` changes. `NotSet` and `Pending` show
      no keycaps. Matches mockup 05.
- [ ] 4.6 D7 surfaces per mockup 06: warning subtitle on the Preferences row, warning badge +
      tooltip + accessible description on launcher and welcome keycaps.
- [ ] 4.5 All new strings through gettext. `po/de.po` updated in the same commit ("GNOME manages
      these shortcuts. Change them in GNOME Settings.", "Open GNOME Settings", "Not set",
      "Used by Annota: {action}", "Used by GNOME: {action}").

## 5. App-accelerator conflicts (design.md D5)

- [ ] 5.1 `find_conflict` checks the running application's registered accelerators first and
      names them "Annota: <label>". The label table is in one place. Verify: unit test with a
      fake accelerator list, where `<Control>q` gives "Annota: Quit" and an unregistered combo
      gives no conflict.
- [ ] 5.2 Set Shortcut dialog shows "Used by Annota: Quit" for `Ctrl+Q` and keeps "Set"
      insensitive (mockup 04). Verify: manual check row, plus the 5.1 test covering the decision.

## 6. Verify

- [ ] 6.1 `manual-checks.md` with live rows, **all run before archive**: the maintainer's
      current state (Preferences, launcher and welcome show the triggers GNOME reports - since 2026-11-14 evening `Ctrl+Print` / `Ctrl+Alt+Print` /
      `Shift+Ctrl+Print` - not `Ctrl+P` / `Ctrl+W` / `Ctrl+Q`); a trigger taken by GNOME shows "Used by GNOME"; rebind in GNOME Settings and see it
      after reopening Preferences; press the rebound key, which starts that capture; "Open GNOME
      Settings"; fallback-path dialog conflict on `Ctrl+Q`; dark style.
- [ ] 6.2 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `meson test -C _build`
      green.
- [ ] 6.3 `openspec validate --all --strict` passes.
- [ ] 6.4 `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean.
- [ ] 6.5 `reviewer` agent (fresh context): specs vs. tests, mockups vs. screenshots, no reads of
      `org.gnome.settings-daemon.global-shortcuts`. No blocking findings.

## Workflow follow-up

- `/opsx:archive fix-portal-shortcut-bindings` once 6.x is done.
- Follow-up change for `ConfigureShortcuts` (portal v2, `ashpd` 0.12) when a v2 GNOME backend is
  available to measure.
