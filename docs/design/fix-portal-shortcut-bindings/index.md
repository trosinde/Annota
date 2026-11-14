# fix-portal-shortcut-bindings — mockups

Change: `openspec/changes/fix-portal-shortcut-bindings/`. Source HTML in `src/`, renders in `png/`
(`-light@1x`, `-dark@1x`, `@2x`). All screens 1280 × 800. Built on add-screen-capture 01, 09, 10 and 11
and on bootstrap-app-shell 01. Only the parts named in the redlines change. Everything else is as
drawn in those base mockups.

The example triggers are the ones measured on the maintainer's machine on 2026-11-14:
`region` = `<Shift><Control>p`, `window` = `<Alt>Print`, `screen` = `<Shift>Print`.

| # | Screen | File (light@1x) |
|---|---|---|
| 01 | Capture launcher, portal path, Screen selected | [png/01-launcher-portal-triggers-light@1x.png](png/01-launcher-portal-triggers-light@1x.png) |
| 02 | Preferences › Capture, portal path, read-only rows, "Open GNOME Settings" (keyboard focus) | [png/02-prefs-portal-read-only-light@1x.png](png/02-prefs-portal-read-only-light@1x.png) |
| 03a | Preferences › Capture, portal path, Window "Not set" | [png/03a-prefs-portal-not-set-light@1x.png](png/03a-prefs-portal-not-set-light@1x.png) |
| 03b | Preferences › Capture, portal path, portal has not answered yet (Pending) | [png/03b-prefs-portal-pending-light@1x.png](png/03b-prefs-portal-pending-light@1x.png) |
| 04 | Set Shortcut dialog, fallback path (GNOME 46), `Ctrl+Q` refused: "Used by Annota: Quit" | [png/04-set-shortcut-app-conflict-light@1x.png](png/04-set-shortcut-app-conflict-light@1x.png) |
| 05 | Welcome page, portal triggers, Screen has no trigger | [png/05-welcome-portal-triggers-light@1x.png](png/05-welcome-portal-triggers-light@1x.png) |

## Scenario → mockup

| Spec / Requirement / Scenario | Mockups | Notes |
|---|---|---|
| capture / Global capture hotkey / Hotkey on GNOME 46 | existing: add-screen-capture 09 → 10, 11 | Unchanged by this change. Covered by the archived change's manual check. |
| capture / Global capture hotkey / Portal keeps an earlier trigger | 01, 02, 05 | Screen shows `Shift+Print` in all three surfaces, `Ctrl+Q` nowhere. 05 shows the Region card variant of the same rule. |
| capture / Global capture hotkey / Shortcut the portal has not bound | 03a (Preferences), 05 (welcome card without keycaps) | Launcher: the toggle loses its caption line, see redlines. The spec example is `window`; 05 shows the rule on `screen`, as briefed. Same rendering for every mode. |
| capture / Global capture hotkey / Shortcuts are managed by GNOME | 02, 03a, 03b | Rows have no chevron and do not react to activation. Description and button are in the group header. |
| capture / Global capture hotkey / Open GNOME Settings | 02 (button, focus ring) | What opens is GNOME Settings itself. No Annota UI. Live check (task 4.2, 6.1). |
| capture / Global capture hotkey / Trigger changed in GNOME Settings | live check | Same screen as 02 with `Super+Shift+S` in the Screen row. Nothing new to draw. |
| capture / Reversible takeover / Takeover is reversible | existing: add-screen-capture 11 | Unchanged. |
| capture / Reversible takeover / No takeover on the GlobalShortcuts portal path | live check (+ 01/02 for "the triggers Annota shows") | GNOME's own bindings are not visible in Annota. |
| capture / Reversible takeover / Shortcut conflict | existing: add-screen-capture 11 + 04 for the error styling | 04 is the error form of the status line for every conflict source. |
| capture / Reversible takeover / Conflict with an Annota application shortcut | 04 | |
| app-shell / Welcome page / Mode card starts a capture | existing: bootstrap-app-shell 01, live check | Unchanged. |
| app-shell / Welcome page / Shortcut keycaps follow the settings | existing: bootstrap-app-shell 01, live check | Fallback path, unchanged. |
| app-shell / Welcome page / Keycaps show the portal's triggers | 05 | Region card: `Shift` `Ctrl` `P`. |

## Redlines

### One rule for all surfaces

Every surface renders `shortcuts::shown_trigger(mode)`:

| `ShownTrigger` | Preferences row suffix | Launcher toggle caption | Launcher Capture button | Welcome card |
|---|---|---|---|---|
| `Accel(a)` | `GtkShortcutLabel` keycaps (`Shift` `Ctrl` `P`) | `gtk::accelerator_get_label(a)`, 11 px dim (`Shift+Ctrl+P`) | dim suffix label, same text | `GtkShortcutLabel` keycaps |
| `Text(t)` | `t` verbatim, dim label, body size, ellipsize end, max 24 chars | `t` verbatim, ellipsize end | `t` verbatim, ellipsize end | `t` verbatim, dim caption 11 px, ellipsize end |
| `NotSet` | "Not set", dim label, body size (`.dim-label`) | no caption line | no suffix | no keycaps |
| `Pending` | empty suffix | no caption line | no suffix | no keycaps |

- Keycap order and labels are GTK's own (`GtkShortcutLabel`): `Shift` `Ctrl` `P`, `Alt` `Print`,
  `Shift` `Print`. Do not reorder them to match the portal string.
- **No spinner for Pending.** The measured answer arrives in about 0.2 s. A spinner would only flash.
  The kit has a spinner, but `AdwSpinner` needs libadwaita 1.6 and Ubuntu 24.04 has 1.5. Pending
  shows only before the first answer of the process (design.md D2). Later refreshes keep the last
  known triggers, with no visible change while they run.
- **Rows keep their height in every state.** Preferences rows are 50 px with no subtitle. "Not set"
  and an empty suffix do not change the row height.
- **Welcome cards keep their layout.** A card without keycaps keeps the keycap line's 22 px height
  (an empty box with `min-height: 22px`, hidden from accessibility). Icon and title then line up across
  all three cards (05).
- **Launcher toggles keep their height.** The toggle stays 76 px when the caption line is missing.
  Icon and label stay centred.

### Capture launcher (01)

Same as add-screen-capture 01 (popover 356 px, padding 12, toggles 76 px), with these changes:
- Toggle captions come from `shown_trigger`: Region `Shift+Ctrl+P`, Window `Alt+Print`, Screen
  `Shift+Print`.
- In the drawn state Screen is active, and the suggested button reads "Capture Screen" with the dim
  suffix `Shift+Print`.
- `refresh_bound()` runs on the popover's `show`. The captions update in place when the answer differs.
- Header-bar capture button tooltip: "New Capture (Ctrl+N)", as built. It does not show a global trigger.

### Preferences › Capture, portal path (02, 03a, 03b)

`AdwPreferencesDialog` 660 × 720, as add-screen-capture 09. The view switcher shows the pages that
exist in the build (General · Capture). The Library page is drawn in add-capture-library.
- **No banner** on the portal path.
- **Group "Global Shortcuts"** (`AdwPreferencesGroup`):
  - description: "GNOME manages these shortcuts. Change them in GNOME Settings."
  - `header-suffix`: `GtkButton` "Open GNOME Settings", default style (not flat, not suggested),
    `valign: center`. The description wraps beside it, as drawn. Hidden when
    `org.gnome.Settings.desktop` is not installed (design.md D4). The description then stays
    unchanged.
  - This replaces the old portal description "Work in every app while Annota is running".
- **Rows** "Region capture", "Window capture", "Screen capture": `AdwActionRow`,
  `activatable: false`, **no chevron**, **no subtitle**. "System shortcut" is dropped, because the
  description already says who owns the keys. The suffix follows the table above. The rows stay
  focusable, so a screen reader reads them as "Region capture, Shift+Ctrl+P".
- **Keyboard:** focus order Close · view switcher · Open GNOME Settings · the three rows · the Capture
  group. 02 shows the focus ring on the button.
- `refresh_bound()` runs on the page's `map`.
- Group "Capture" unchanged.

### Set Shortcut dialog, conflict with Annota (04)

As add-screen-capture 11 (440 px, Cancel / Set Shortcut / Set). Background: Preferences on the
fallback path after "Create GNOME Shortcuts" (add-screen-capture 10 without the toast), triggers
`Print`, `Alt+Print`, `Shift+Print` (D9).
- Prompt: "Press the new shortcut for **Screen capture**".
- Live keycaps `Ctrl` `Q`, 34 px, each with a 1 px `--destructive` inset ring (as add-capture-presets 04).
- Status line, `.error` style, 13 px (`--fs-small`), `warning` icon 16 px + "Used by Annota: Quit". The icon and
  the text carry the meaning, so it does not rely on colour alone.
- "Set" insensitive. Cancel and Esc still close the dialog. Backspace still disables the shortcut.
- The text is the existing string "Used by {name}" with `{name}` = "Annota: {action}" (design.md D5).
  The action labels are the menu and shortcuts-window labels: Quit, Open Image, Open Library,
  Preferences, Paste Image, and the editor's window actions.

### Welcome page (05)

As bootstrap-app-shell 01. Cards 156 × 108, keycaps `kbd` 22 px.
- Region: `Shift` `Ctrl` `P`. Window: `Alt` `Print`. Screen: no keycaps (line kept empty, see above).
- The Region card keeps the initial focus.
- The keycaps follow `connect_bound_changed` and, on the fallback path, `notify::shortcut-*`.

### Strings

`po/de.po` addresses the user as "Sie", and the existing string "Used by {name}" is "Belegt durch {name}".
The German below follows both.

| English | German |
|---|---|
| GNOME manages these shortcuts. Change them in GNOME Settings. | GNOME verwaltet diese Tastenkürzel. Ändern Sie sie in den GNOME-Einstellungen. |
| Open GNOME Settings | GNOME-Einstellungen öffnen |
| Not set | Nicht festgelegt |
| Used by {name} (existing) → "Used by Annota: Quit" | Belegt durch {name} (existing) → "Belegt durch Annota: Beenden" |
| Annota: {action} | Annota: {action} |

The brief proposed "Belegt von Annota: Beenden". The existing translation uses "Belegt durch", so
this change uses "Belegt durch" too.

## Open questions

- **Bind error leaves Pending forever.** design.md "Risks" accepts this: on a bind error, `BOUND`
  stays empty and the rows stay empty. The user then sees three empty rows that look unfinished. A
  `Pending` that does not end could show "Not available" ("Nicht verfügbar") after the bind fails. That
  needs a fifth `ShownTrigger` state or an error flag. Not drawn, decision needed.
- **Base mockups are stale.** add-screen-capture 01, 09, 10 and 11 still show `Shift+Print` /
  `Super+Shift+W` / `Super+Shift+S`. Their own index and decision D9 say `Print` / `Alt+Print` /
  `Shift+Print`. These mockups do not repeat the stale keys. The base files were not edited.
- Exact GNOME Settings target (task 2.2) does not change any screen.
