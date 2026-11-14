# Design

## Context

See proposal.md, "Why", for the defect and the measurements behind it.

The current code, as read on 2026-11-14:

- `shortcuts::run_portal_shortcuts` (`src/capture/shortcuts.rs`) builds one `NewShortcut` per
  mode from the `shortcut-*` settings and calls `bind_shortcuts`. It then **discards the
  response** (`.response()?` is checked for errors only) and loops over `receive_activated`.
- `change_shortcut` on the portal path writes the setting and calls `restart_portal_shortcuts`.
  That closes the session and binds again. On a version-1 GNOME portal the bind returns the
  stored triggers, so the edit has no effect at the desktop.
- Every place that shows a shortcut reads the setting directly:
  `preferences.rs` (`ShortcutsView`, rows with `gtk::ShortcutLabel`), `application.rs`
  (`setup_launcher` `refresh`, `setup_welcome` `fill_keycaps`) and the Set Shortcut dialog's
  conflict check (`find_conflict`).
- `ashpd` 0.11.1 (the locked version) already offers `list_shortcuts` and
  `receive_shortcuts_changed`. It does not offer `configure_shortcuts`, which arrived in 0.12
  together with portal version 2.

What the live portal does, measured 2026-11-14 with a probe registered as
`io.github.trosinde.Annota` (GNOME Shell 50.1, xdg-desktop-portal 1.21.1,
`GlobalShortcuts.version` = 1, `LANG=en_US.UTF-8`):

| Call | Result |
|---|---|
| `CreateSession`, then `ListShortcuts` before any bind | `0`, `shortcuts: []` |
| `BindShortcuts` with `region`/`window`/`screen` and `CTRL+p`/`CTRL+w`/`CTRL+q` preferred | `0` after 0.20 s, no dialog. Returned triggers: `Press <Shift><Control>p`, `Press <Alt>Print`, `Press <Shift>Print` |
| `ListShortcuts` after the bind, same session | the same three triggers |
| GNOME's store after the bind | unchanged: `<Shift><Control>p`, `<Alt>Print`, `<Shift>Print` |

gnome-control-center 50.3 contains a global-shortcuts rebind client
(`cc_global_shortcuts_rebind_*`, `cc_keyboard_item_load_from_global_shortcuts`). GNOME Settings
can therefore change Annota's triggers in its keyboard panel. The exact page and path are not
measured yet (task 2.2).

**Wayland vs X11.** The GlobalShortcuts portal is the same D-Bus interface in both sessions.
Annota chooses the path by whether the portal answers, never by display server. The only
behaviour difference is the one that already exists: GNOME < 48 has no portal and uses the custom
keybinding fallback, under both Wayland and X11.

## Goals / Non-Goals

**Goals:**
- One source of truth per path for "which key starts this capture": the portal's answer on the
  portal path, the `shortcut-*` settings on the fallback path. Every UI surface reads the shown
  triggers through that one source.
- No UI that pretends to edit something it cannot change.
- A global binding can no longer take one of Annota's own app shortcuts.

**Non-Goals:**
- Changing how the fallback path writes GNOME keybindings, or the D9 takeover logic.
- Parsing or normalising GNOME's trigger strings beyond what display needs. No logic decides
  anything from a parsed portal trigger.
- A "reset to defaults" for portal triggers. GNOME Settings owns that.

## Decisions

### D1: Keep the bind response as the bound-trigger store

`run_portal_shortcuts` keeps `response.shortcuts()` in a thread-local `BOUND` map (id → trigger
description, missing = not bound). It replaces the map on every bind, on `ShortcutsChanged`, and
on an explicit `refresh_bound()` that calls `list_shortcuts` on the live session. A plain
callback list (`connect_bound_changed`) notifies the UI, in the same thread-local style as
`PORTAL_ACTIVE`/`PORTAL_APP`. All three UI surfaces subscribe to it, plus `notify` from the
`shortcut-*` settings on the fallback path. They then call one function,
`shortcuts::shown_trigger(mode) -> ShownTrigger { Pending, NotSet, Accel(String), Text(String) }`.

*Alternatives.* (a) Read GNOME's dconf store `org.gnome.settings-daemon.global-shortcuts`:
rejected, because it is compositor-private (CLAUDE.md §2) and differs per desktop. (b) Write the
reported trigger back into `shortcut-*`: rejected, because it would erase the user's preference
for a later first bind (e.g. after GNOME forgets the app) and blur the fallback path's meaning of
those keys.

### D2: Refresh on bind, on `ShortcutsChanged`, and when a surface opens

`ShortcutsChanged` is the clean signal, but nobody has measured whether GNOME 50 emits it when
the user rebinds in GNOME Settings (task 2.3). `ListShortcuts` on the live session is measured
and cheap (no dialog, < 0.2 s). Preferences › Capture (`map`) and the launcher popover (`show`)
therefore call `refresh_bound()` as well. The welcome page has no open event of its own and
follows the callback. While the refresh is in flight, a surface keeps its last known triggers. It
shows `Pending` (no keycaps, row subtitle empty) only before the first answer of the process.

*Alternative.* Rebind (`restart_portal_shortcuts`) on every open: rejected. It closes and
recreates the session and drops `Activated` events in between, for no gain over `ListShortcuts`.

### D3: Showing `trigger_description`

The portal defines `trigger_description` as free text for display. Measured GNOME 50 form:
`Press <accelerator>`. Annota shows keycaps (`gtk::ShortcutLabel`) only when the text ends in a
token that `gtk::accelerator_parse` accepts: the last whitespace-separated token, here
`<Shift><Control>p`. Otherwise it shows the text verbatim as a label, which covers other
desktops, other locales and a future format. An empty description means `NotSet`. Task 2.1
measures the format under `LANG=de_DE.UTF-8`. If GNOME translates "Press", only the verbatim
fallback is affected, not correctness.

### D4: Portal-path Preferences rows are read-only, plus "Open GNOME Settings"

On the portal path the three rows lose their chevron and `activatable`. The group description
reads "GNOME manages these shortcuts. Change them in GNOME Settings." and the group header gets a
button "Open GNOME Settings" (mockups 02 and 03). The button calls
`gio::AppInfo` for `org.gnome.Settings.desktop` with the `keyboard` panel argument, or the
`org.gnome.Settings` `launch-panel` action over D-Bus, whichever task 2.2 shows lands on the page
that lists Annota's shortcuts. If GNOME Settings is not installed, the button is hidden.

*Alternative.* Keep the Set Shortcut dialog and send its value as `preferred_trigger` again:
rejected. That is exactly today's no-op.

### D5: App-accelerator conflicts come from the running application

`find_conflict` gets a further source before the GNOME schemas: every accelerator the running
`gtk::Application` has registered (`list_action_descriptions` + `accels_for_action`), named
"Annota: <action label>". The labels come from one small table keyed by action name (Quit, Open
Image, Open Library, Preferences, Paste Image, and the editor's window actions). A missing label
falls back to "Annota". Reading the live registrations means the list cannot drift from the code.
The check applies on the fallback path, the only path that still has the dialog.

### D6: Settings stay put

`shortcut-*` are left as they are on both paths. On the portal path they keep one job: the
preferred triggers of a *first* bind. The Preferences footnote on the portal path does not show
them. Nothing migrates, so rollback means reverting the code alone.

## Mockups

Produced by the `designer` agent in `docs/design/fix-portal-shortcut-bindings/` (design gate,
task group 1). Planned screens and the scenarios they cover:

| # | Screen | Scenarios |
|---|---|---|
| 01 | Capture launcher, portal path, triggers from the portal (Screen `Shift+Print`) | capture: Portal keeps an earlier trigger |
| 02 | Preferences › Capture, portal path: read-only rows, description, "Open GNOME Settings" | capture: Shortcuts are managed by GNOME; Open GNOME Settings; Portal keeps an earlier trigger |
| 03 | Preferences › Capture, portal path: one row "Not set", and the pending state | capture: Shortcut the portal has not bound |
| 04 | Set Shortcut dialog (fallback): "Used by Annota: Quit" | capture: Conflict with an Annota application shortcut |
| 05 | Welcome page, portal triggers, one card without keycaps | app-shell: Keycaps show the portal's triggers; capture: Shortcut the portal has not bound |

Each in light and dark, 1× and 2×, built on add-screen-capture 01/09/11 and bootstrap-app-shell
01. Scenarios with no screen ("Trigger changed in GNOME Settings", "No takeover…", "Hotkey on
GNOME 46") are covered by a live check (`manual-checks.md`) or by existing tests.

Design approved: _pending_

## Risks / Trade-offs

- [GNOME never emits `ShortcutsChanged` for a rebind in Settings] → D2's refresh on open still
  catches it. The welcome page may then lag until the launcher or Preferences opens. This is
  documented in `manual-checks.md` if measured so.
- [Another desktop returns triggers in a format D3 cannot parse] → verbatim text is shown. It is
  still the truth, only less pretty.
- [The portal answers the bind with an error] → `BOUND` stays empty and the surfaces show
  `Pending`, not the settings. This is honest (no binding is known), but on that error the user
  sees no keys at all. The error is already logged
  (`annota: global shortcuts portal not used: …`).
- [Users relied on editing in Preferences on GNOME ≥ 48] → it never worked after the first bind,
  so no working behaviour is lost.

## Migration Plan

None. No data or settings change. Rollback is a plain revert.

## Open Questions

- Exact GNOME Settings entry point (panel name or launch-panel argument) that shows the
  global-shortcuts list. Task 2.2 answers it before D4 is built. It changes only the argument,
  not the design.
