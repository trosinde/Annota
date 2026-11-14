# Proposal

**Roadmap phase:** MVP (milestone 0.1). This is a defect fix in `add-screen-capture`'s archived
hotkey work.
**Benchmark feature (docs/feature-catalog.md):** "Global hotkeys"

## Why

On the GlobalShortcuts portal path, Annota shows the shortcuts it *wants* and not the ones the
desktop *has*. It also lets the user "change" them, but the change never reaches the desktop. The
maintainer hit this live on 2026-11-14 (GNOME Shell 50.1, gnome-control-center 50.3,
xdg-desktop-portal 1.21.1, `org.freedesktop.portal.GlobalShortcuts` `version` = 1). The report
was "capture screen does not work, it always runs a region capture":

- Annota's settings held `shortcut-region='<Control>p'`, `shortcut-window='<Control>w'` and
  `shortcut-screen='<Control>q'`. Preferences, the launcher and the welcome cards all showed these.
- GNOME had bound `region` to `<Shift><Control>p`, `window` to `<Alt>Print` and `screen` to
  `<Shift>Print`.
- Pressing `Ctrl+Q` produced no `Activated` signal (D-Bus monitor). Inside an Annota window it ran
  `app.quit`, the app's own `Ctrl+Q`.
- `<Shift>Print` is also GNOME Shell's own `screenshot` key. In the same way, `Print` (GNOME's
  region-style screenshot UI) is GNOME's key and not Annota's. That is most likely the "region"
  the user saw.
- Measured with a probe that registered as `io.github.trosinde.Annota` and called `BindShortcuts`
  with the same IDs and `CTRL+p`/`CTRL+w`/`CTRL+q` as preferred triggers: GNOME answered within
  0.20 s with no dialog. It returned the *stored* triggers (`trigger_description` =
  `Press <Shift><Control>p`, `Press <Alt>Print`, `Press <Shift>Print`) and left its store
  unchanged. On a version-1 portal, `preferred_trigger` counts only the first time an ID is bound.

Launcher, CLI and the D-Bus `capture('screen')` action were also tested live. They deliver a
correct screen capture ("Screen captured · 3840 × 2160"). The capture modes are fine. What is
broken is how the hotkey is shown and edited.

## What Changes

- On the portal path, the shortcuts Annota *shows* (Preferences › Capture, launcher popover,
  welcome cards) SHALL be the triggers the portal reports for Annota's three shortcut IDs. They
  SHALL never be Annota's own `shortcut-*` settings. A shortcut the portal has not bound shows as
  "Not set".
- On the portal path, Preferences SHALL NOT offer the Set Shortcut dialog. Editing it there cannot
  have an effect on a version-1 portal. Preferences SHALL say instead that GNOME owns these
  shortcuts, and offer a button that opens GNOME Settings at its keyboard shortcuts.
- The shown triggers SHALL refresh when Annota binds, and when Preferences or the launcher opens.
  A user who rebinds in GNOME Settings then sees the new keys without restarting Annota, as long
  as the portal reports them.
- The Set Shortcut dialog (GNOME-custom-keybinding fallback, GNOME 46) SHALL also reject Annota's
  own application accelerators (`Ctrl+Q`, `Ctrl+O`, `Ctrl+L`, `Ctrl+,`, `Shift+Ctrl+V` and the
  editor's). A global binding on one of them takes that key from every application, Annota
  included.
- The spec's scenario "No takeover on the GlobalShortcuts portal path" is corrected. Its current
  wording assumes Annota's preferred triggers are what gets bound, and that is false after the
  first bind.

**Out of scope**
- `ConfigureShortcuts` (portal version 2) and the `ashpd` upgrade it would need (0.11 → 0.12). No
  version-2 GNOME backend exists on any target platform to measure against. A follow-up change
  takes it on when one does.
- Migrating or resetting values that already sit in `shortcut-*`. They stay as they are and keep
  driving the fallback path.
- Which of GNOME Shell or Annota reacts when both bind the same key (e.g. `Shift+Print`). The spec
  keeps saying the compositor decides. One probe task records what GNOME 50 does.
- Annota never reads or writes `org.gnome.settings-daemon.global-shortcuts` (GNOME-private
  storage, CLAUDE.md §2). Everything shown comes from the portal.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `capture`: "Global capture hotkey" (what is shown and editable on the portal path, and the
  refresh) and "Reversible takeover of GNOME's screenshot keys" (the corrected portal-path
  scenario, and the extended conflict rule in the Set Shortcut dialog).
- `app-shell`: "Welcome page" (keycaps show the triggers that are actually bound, not the
  settings).

## Impact

- Code: `src/capture/shortcuts.rs` (keep the `BindShortcuts` response, expose the bound triggers,
  watch for changes, extend `find_conflict` with app accelerators), `src/preferences.rs` +
  `data/ui/preferences.blp` (portal-path rows read-only, "Open GNOME Settings"),
  `src/application.rs` (launcher and welcome keycaps from the bound triggers), `po/de.po`.
- Mockups: the add-screen-capture screens 01, 09, 10 and 11 gain portal-path variants, and the
  welcome card keycaps (bootstrap-app-shell 01) gain a "Not set" state.
- Dependencies: none new. `ashpd` stays at 0.11.1.
- Platform: GNOME ≥ 48 (portal path) changes. GNOME 46 (fallback path) changes only in the
  conflict rule. X11 sessions on GNOME ≥ 48 use the same portal and behave the same.
