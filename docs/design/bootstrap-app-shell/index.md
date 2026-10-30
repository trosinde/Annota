# bootstrap-app-shell — mockups

Source HTML: `src/`, renders: `png/<name>-{light,dark}@{1,2}x.png`. Render with
`python3 tools/render-mockups.py docs/design/bootstrap-app-shell`.

| # | Screen | Light 1× |
|---|---|---|
| 01 | First-launch welcome (no document open) | [png](png/01-first-launch-welcome-light@1x.png) |
| 02 | Main menu open | [png](png/02-main-menu-light@1x.png) |
| 03 | Preferences — General (save folder, pattern, default export format = JPG) | [png](png/03-preferences-general-light@1x.png) |
| 04 | Preferences — default format dropdown open | [png](png/04-preferences-format-dropdown-light@1x.png) |
| 05 | Preferences — Capture (after-capture behaviour, capture defaults) | [png](png/05-preferences-capture-light@1x.png) |
| 06 | About dialog with app icon | [png](png/06-about-dialog-light@1x.png) |
| 07 | Keyboard shortcuts window | [png](png/07-keyboard-shortcuts-light@1x.png) |
| 08 | Editor after `annota --open shot.annota` | [png](png/08-editor-open-file-light@1x.png) |
| 09 | GNOME overview app grid with Annota | [png](png/09-gnome-app-grid-light@1x.png) |
| 10 | GNOME overview search "anno" | [png](png/10-gnome-search-light@1x.png) |
| 11 | Terminal: `--version`, invalid mode, invalid delay | [png](png/11-cli-errors-light@1x.png) |

Every screen also exists as `-dark@1x/2x`. Screens 09–11 show GNOME Shell / Terminal (not Annota UI); they stay dark in both renders on purpose.

## Scenario → mockup

| Capability | Scenario | Mockup | Notes |
|---|---|---|---|
| app-shell | Second launch forwards request | — | No UI of its own. The running instance starts the region capture overlay (designed in `add-screen-capture`). Verified by test. |
| app-shell | Invalid mode | 11 | Exit status 2, stderr lists `region, window, screen`. Delay outside 0–60 shown in the same mock. |
| app-shell | Open file | 08 | Title = file name, subtitle = folder · size. |
| app-shell | Setting persists | 03, 04 | Default format JPG chosen in 04, still shown after restart in 03. |
| app-shell | Open shortcuts | 07, 02 | `Ctrl+?` from any window; also from main menu (02). |
| app-shell | System switches to dark | all `-dark` renders | No in-app style switch; the Appearance row in 03 only states "System". |
| app-shell | No network at all | 01, 06 | No sign-in, account or connectivity UI anywhere. About states "Works entirely on this computer". |
| app-shell | No outgoing connections | — | No UI. Verified by test (traffic monitor). |
| debian-packaging | Offline build | — | No UI — verified by test (CI). |
| debian-packaging | App grid | 09, 10 | Icon `io.github.trosinde.Annota`, name "Annota", comment "Capture and annotate screenshots". |
| debian-packaging | Dependencies resolved | — | No UI — verified by test. |
| debian-packaging | Failing lint blocks | — | No UI — verified by test (CI). |

## Redlines

### Window and welcome (01)
- Window 1280×800 default, `AdwApplicationWindow` + `AdwToolbarView`. Header bar 47 px.
- Header bar with no document: start = New Capture (`capture-region` icon, tooltip "New Capture (Ctrl+N)"); title "Annota"; end = main menu (`open-menu`), close. No undo/redo/copy/Export while no document is open.
- Welcome = `AdwStatusPage`-style: app icon 128 px, 24 px gap, title "Capture Anything" (title-1, 32 px/800), description max 460 px, 30 px gap.
- Three mode cards 156×108 px, 12 px gap, radius 12, `.card.activatable`: icon 32 px, label bold, shortcut keycaps. Labels/shortcuts: Region `Print`, Window `Alt+Print`, Screen `Shift+Print`. First card has keyboard focus on launch (focus ring 2 px accent 50 %).
- Hint line (13 px, dim): folder icon, "or open an image or .annota file with Ctrl O — or drop it here". The whole window is a drop target.

### Main menu (02)
Popover 290 px, items and accels exactly: New Capture `Ctrl+N` · Open… `Ctrl+O` · Capture Library `Ctrl+L` · separator · Preferences `Ctrl+,` · Keyboard Shortcuts `Ctrl+?` · About Annota.

### Preferences (03–05)
- `AdwPreferencesDialog` 640×680, two pages in a view switcher in the header: General (`settings` icon), Capture (`camera` icon); search button at start.
- General → group "Saving" (description "Where Ctrl+S puts new captures"): row "Save folder" subtitle path + folder button (opens portal file chooser); entry row "File name pattern" monospace, default `Annota %Y-%m-%d %H-%M-%S`, info button opens a pattern-help popover. Under the group, caption "Example: …" updated live.
- Group "Export": "Default format" combo row PNG/JPG/WebP/PDF (popover in 04, selected item with check); "Export folder" row + folder button.
- Group "Appearance": "Style" — read-only value "System", subtitle "Follows the system light or dark style and accent colour". No style override in 0.1.
- Capture → group "After Capture": three radio rows *Open in editor* (default) / *Copy to clipboard* (subtitle "No window opens") / *Open in editor and copy to clipboard*. Group "Capture Defaults": Default mode combo (Region/Window/Screen), Delay spin 0–60 s, Include pointer switch. Group "Shortcut": Capture region shows the current binding as keycap.
- GSettings keys: `save-folder`, `filename-pattern`, `default-export-format` (enum png/jpg/webp/pdf), `export-folder`, `after-capture` (enum editor/clipboard/both), `capture-mode`, `capture-delay`, `include-pointer`.

### About (06)
`AdwAboutDialog`: icon 128 px, name "Annota", developer "The Annota Contributors", version pill "0.1.0", rows Website, Report an Issue, Credits, Legal. Comment/footer: "Works entirely on this computer. No account, no tracking." · "© 2026 The Annota Contributors · GPL-3.0-or-later".

### Shortcuts window (07)
`GtkShortcutsWindow`, 4 columns, section titles in accent colour. Groups and keys exactly as drawn:
General (New capture Ctrl+N, Open file Ctrl+O, Capture library Ctrl+L, Preferences Ctrl+comma, Keyboard shortcuts Ctrl+?, Close window Ctrl+W, Quit Ctrl+Q), Capture (Print, Alt+Print, Shift+Print), Document (Save Ctrl+S, Save as Ctrl+Shift+S, Export Ctrl+E, Copy image Ctrl+C, Undo Ctrl+Z, Redo Ctrl+Shift+Z, Zoom Ctrl+plus/minus, Best fit Ctrl+0), Tools (Select V, Arrow A, Line L, Pen P, Shape U, Text T, Callout K, Step S, Highlighter H, Redact B), More Tools (Crop C, Magnify M, Spotlight G, Stamp I, Cut-out X, Fill F, Eraser E, Magic wand W, Selection Q).

### Launcher (09, 10)
- Desktop file: `Name=Annota`, `Comment=Capture and annotate screenshots`, `Icon=io.github.trosinde.Annota`, `Keywords=screenshot;capture;annotate;markup;Bildschirmfoto;`, `Categories=Graphics;Utility;`.
- Scalable icon `docs/design/kit/app-icon/annota.svg`, symbolic `annota-symbolic.svg`.

### CLI (11)
- `annota --version` → `annota 0.1.0`, exit 0.
- Invalid mode: `error: invalid value 'banana' for '--capture <MODE>'` + `[possible values: region, window, screen]` + usage + `--help` hint, exit 2.
- Delay out of range: `… 90 is not in 0..=60`, exit 2.
- `--open` with valid file: no output, editor opens (08).
