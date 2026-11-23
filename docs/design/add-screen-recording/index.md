# Add screen recording (core) — mockups

Core recording only: record a region, a window or a screen through the ScreenCast portal +
PipeWire, countdown, a small Recording control window (pause/resume, stop, microphone and
system-audio mute), the global Record shortcut, and the Export Recording window (MP4 or WebM).
Webcam, cursor effects, drawing while recording, the video editor and GIF are in
`add-recording-extras` (`docs/design/add-recording-extras/`).

Redrawn on 2026-11-22 after the maintainer split the feature. The first set (October 2026)
had a red recording frame, a control pinned under the region, an Annota timer in GNOME's top bar
and a video editor after stop; none of that survives here (see "Platform notes").

Scene: GNOME desktop 1280×800, the invented `d-tidewater-analytics` dashboard maximised
behind. All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`.
Render with `python3 tools/render-mockups.py docs/design/add-screen-recording`.
Violet boxes and labels in the scenes are designer notes, not UI.

## Scenario → mockup

Titles are *Requirement › Scenario* exactly as in the spec files of this change.

### screen-recording

| Requirement › Scenario | Mockup | Notes |
|---|---|---|
| Recording setup › Switch the overlay to video | [01](png/01-recording-setup-light@1x.png) | Mode pill at the bottom set to Video; the setup popover sits beside the selection. |
| Recording setup › Choices are remembered | [01](png/01-recording-setup-light@1x.png) | No own screen: the popover opens with the saved values (source, both audio switches, pointer, countdown). |
| Recording setup › Cancel the setup | [01](png/01-recording-setup-light@1x.png) | Hint `Esc` Cancel in the mode pill. Overlay closes, no portal request. |
| Recording source › Record a region | [01](png/01-recording-setup-light@1x.png), [03](png/03-waiting-for-screen-light@1x.png) | Region asks for a monitor source → "Waiting for a screen…". Crop to the selection: no UI (pipeline test). |
| Recording source › Region not on the chosen screen | [13](png/13-region-not-on-screen-light@1x.png) | Toast in the main window. |
| Recording source › Record a window | — (reuses fix-window-capture-screencast 02) | "Waiting for a window…" OSD and GNOME's window chooser unchanged; the stream is kept instead of one frame. |
| Recording source › Record the screen | [03](png/03-waiting-for-screen-light@1x.png) | "Waiting for a screen…". Frame size: no UI (pipeline test). |
| Recording source › Source request cancelled | [03](png/03-waiting-for-screen-light@1x.png) | Cancel / `Esc` / chooser closed → nothing shown afterwards. |
| Recording source › Pointer in the recording | [01](png/01-recording-setup-light@1x.png) | "Include pointer" row (the capture setting). Cursor mode: no UI (portal test). |
| Recording source › Source not offered | [02](png/02-setup-unavailable-light@1x.png) | Window segment insensitive, tooltip "Screen sharing for this source is not available on this desktop". |
| Recording countdown › Countdown is not recorded | [04](png/04-countdown-light@1x.png) | OSD + dashed outline only during the countdown; first frame after both are gone (pipeline test). |
| Recording countdown › Cancel the countdown | [04](png/04-countdown-light@1x.png) | Hint `Esc` Cancel. No message afterwards. |
| Recording countdown › No countdown | [01](png/01-recording-setup-light@1x.png) | Countdown spin at `0 s` → no OSD, the control (05) appears directly. |
| Recording control › Elapsed time | [05](png/05-recording-control-light@1x.png) | `0:12`, `M:SS`, tabular figures. |
| Recording control › Minimize the control | [05](png/05-recording-control-light@1x.png) | State card "Hover · Minimize": the window is minimised, recording continues. |
| Recording control › Second recording request | [05](png/05-recording-control-light@1x.png) | State card "Microphone only · second request": the control is presented (raised), nothing new starts. |
| Pause and resume › Paused time is not recorded | — (no UI; pipeline test) | |
| Pause and resume › Paused state | [06](png/06-paused-mic-muted-light@1x.png) | "Paused", Resume (play), time frozen at `0:27`. |
| Audio › Mute the microphone | [06](png/06-paused-mic-muted-light@1x.png) | Slashed microphone, grey meter, tooltip "Microphone muted" `M`. Silence: pipeline test. |
| Audio › Mute system audio | [05](png/05-recording-control-light@1x.png) | State card "Hover · System audio" (`S`); muted look as the microphone in 06. |
| Audio › No microphone | [02](png/02-setup-unavailable-light@1x.png) | Microphone row insensitive, "No microphone found". Note shows the "No audio output found" variant. |
| Audio › No audio selected | [05](png/05-recording-control-light@1x.png) | No audio buttons in the control (state card "Microphone only · second request" explains). Track layout: pipeline test. |
| Stop recording › Stop by button | [05](png/05-recording-control-light@1x.png) → [07](png/07-export-recording-light@1x.png) | Control closes, Export Recording opens. |
| Stop recording › Stop by hotkey | [05](png/05-recording-control-light@1x.png) → [07](png/07-export-recording-light@1x.png) | Stop tooltip shows the Record trigger. Also works while paused (06). |
| Stop recording › Session ended by the desktop | [07](png/07-export-recording-light@1x.png) | Same window, no banner (a normal end). |
| Stop recording › Recorded window closed | [07](png/07-export-recording-light@1x.png) | Same window, no banner. |
| Record shortcut › Shortcut opens the setup | [01](png/01-recording-setup-light@1x.png) | |
| Record shortcut › Shortcut cancels the countdown | [04](png/04-countdown-light@1x.png) | Note "Not recorded". |
| Record shortcut › Record shortcut on GNOME 46 | — (reuses add-screen-capture 09/10) | GNOME-keybinding path of Preferences › Capture gains a "Record video" row with the same layout as the Region/Window/Screen rows there. |
| Record shortcut › Record row shows the portal's trigger | [14](png/14-preferences-record-shortcut-light@1x.png), [12](png/12-welcome-record-card-light@1x.png) | Drawn with the preferred `Shift+Ctrl+Print`; any reported trigger (e.g. `Super+Alt+R`) replaces it on both. |
| Record shortcut › Record trigger taken by GNOME | [14](png/14-preferences-record-shortcut-light@1x.png), [12](png/12-welcome-record-card-light@1x.png) | Not drawn separately: same "Used by GNOME: <action>" treatment as the capture rows (fix-portal-shortcut-bindings 06a–06c). |
| Recording on the command line › Record a screen from the command line | [03](png/03-waiting-for-screen-light@1x.png) → [04](png/04-countdown-light@1x.png) | |
| Recording on the command line › Invalid record mode | — (no UI; CLI test) | |
| Working file › Recording survives a crash | — (no UI; pipeline test) | |
| Working file › Name already taken | — (no UI; file-system test) | The Export window's default file name then carries the ` (2)` suffix. |
| Working file › Recording error | [11](png/11-recording-error-light@1x.png) | Error banner, duration `0:10`. |
| Export after recording › Export as MP4 | [07](png/07-export-recording-light@1x.png) → [09](png/09-export-saved-light@1x.png) | |
| Export after recording › Export as WebM | [08](png/08-export-progress-light@1x.png) | Progress bar + Cancel, WebM subtitle. |
| Export after recording › Export without audio | [07](png/07-export-recording-light@1x.png) | Include audio switch; insensitive when the recording has no audio. |
| Export after recording › Cancel the export | [08](png/08-export-progress-light@1x.png) → [07](png/07-export-recording-light@1x.png) | Cancel returns to the settings, unchanged. |
| Export after recording › Target name taken | — (no UI; file-system test) | "Saved" (09) shows the name actually written, ` (2)` included. |
| Export after recording › Show in Files | [09](png/09-export-saved-light@1x.png) | |
| Discard a recording › Discard | [10](png/10-discard-confirm-light@1x.png) | |
| Discard a recording › Close asks first | [10](png/10-discard-confirm-light@1x.png) | Same alert from the window's close button. |

### app-shell

| Requirement › Scenario | Mockup | Notes |
|---|---|---|
| Welcome page › Mode card starts a capture | [12](png/12-welcome-record-card-light@1x.png) | Region card unchanged, focused first. |
| Welcome page › Shortcut keycaps follow the settings | [12](png/12-welcome-record-card-light@1x.png) | Keycaps of every card follow the shortcut source. |
| Welcome page › Keycaps show the portal's triggers | [12](png/12-welcome-record-card-light@1x.png) | Same as fix-portal-shortcut-bindings 05. |
| Welcome page › Record card starts the recording setup | [12](png/12-welcome-record-card-light@1x.png) → [01](png/01-recording-setup-light@1x.png) | |

### debian-packaging

| Requirement › Scenario | Mockup | Notes |
|---|---|---|
| Recording runtime dependencies › Recording elements installed | — (no UI; package test) | |

## Screens

| # | Screen | Light | Dark |
|---|---|---|---|
| 01 | Region overlay in Video mode + recording setup (default, Start Recording focused) | [light](png/01-recording-setup-light@1x.png) | [dark](png/01-recording-setup-dark@1x.png) |
| 02 | Setup with no microphone and the Window source not offered (tooltip) | [light](png/02-setup-unavailable-light@1x.png) | [dark](png/02-setup-unavailable-dark@1x.png) |
| 03 | "Waiting for a screen…" OSD + GNOME's screen chooser (schematic) | [light](png/03-waiting-for-screen-light@1x.png) | [dark](png/03-waiting-for-screen-dark@1x.png) |
| 04 | Countdown OSD "Recording starts in 3 s" + dashed region outline | [light](png/04-countdown-light@1x.png) | [dark](png/04-countdown-dark@1x.png) |
| 05 | Recording control window + states band (hover, focus, tooltips, Minimize, mic only + second request) | [light](png/05-recording-control-light@1x.png) | [dark](png/05-recording-control-dark@1x.png) |
| 06 | Paused, microphone muted | [light](png/06-paused-mic-muted-light@1x.png) | [dark](png/06-paused-mic-muted-dark@1x.png) |
| 07 | Export Recording window (MP4, File name focused) | [light](png/07-export-recording-light@1x.png) | [dark](png/07-export-recording-dark@1x.png) |
| 08 | Exporting WebM, progress + Cancel | [light](png/08-export-progress-light@1x.png) | [dark](png/08-export-progress-dark@1x.png) |
| 09 | Saved, Show in Files / Close | [light](png/09-export-saved-light@1x.png) | [dark](png/09-export-saved-dark@1x.png) |
| 10 | "Discard Recording?" alert | [light](png/10-discard-confirm-light@1x.png) | [dark](png/10-discard-confirm-dark@1x.png) |
| 11 | Export Recording after a recording error (banner) | [light](png/11-recording-error-light@1x.png) | [dark](png/11-recording-error-dark@1x.png) |
| 12 | Welcome page with the Record card | [light](png/12-welcome-record-card-light@1x.png) | [dark](png/12-welcome-record-card-dark@1x.png) |
| 13 | Welcome page, toast "The selected area is not on the screen you chose. Nothing was recorded." | [light](png/13-region-not-on-screen-light@1x.png) | [dark](png/13-region-not-on-screen-dark@1x.png) |
| 14 | Preferences › Capture (portal path) with the "Record video" row | [light](png/14-preferences-record-shortcut-light@1x.png) | [dark](png/14-preferences-record-shortcut-dark@1x.png) |

## Redlines

### Shortcuts

| Where | Key | Action |
|---|---|---|
| Recording setup | `Enter` | Start Recording (default widget, keycap shown in the button) |
| Recording setup, waiting OSD, countdown | `Esc` | Cancel |
| Countdown | `Enter` | Start now |
| Recording control (window focused) | `Space` | Pause / Resume |
| Recording control | `M` | Mute / unmute microphone |
| Recording control | `S` | Mute / unmute system audio |
| Recording control | — (button only; GNOME's own `Super+H` also works) | Minimize |
| Global, ID `record` | `Shift+Ctrl+Print` (preferred; show the trigger the portal reports, none if unbound) | Open the setup · cancel a countdown · stop a running or paused recording |
| Export Recording | `Enter` | Export (default widget, also from the File name entry) |
| Export Recording | `Esc` / close button | Same as closing: asks "Discard Recording?" before an export finished |

Tooltips use the kit pattern label + `.shortcut` chip: "Pause" `Space`, "Resume" `Space`,
"Stop" `<Record trigger>`, "Mute microphone" `M` / "Microphone muted" `M`,
"Mute system audio" `S` / "System audio muted" `S`, "Minimize" (no shortcut chip).

### 01–02 Recording setup (region overlay, Video mode)

- Overlay as add-screen-capture: dim `rgb(0 0 6 / 48%)`, selection 1 px white border, 8 round
  handles 12 px (white, 2 px `--accent-bg` ring), size chip `--accent-bg` above the top-left
  corner ("700 × 470 at 130, 92").
- Setup popover: kit `.popover`, arrow left, 18 px right of the selection (flips to the left or
  below when there is no room), 380 px wide, padding 12, gap 12.
  - Source: `.toggle-group` full width, three segments with `capture-region` / `capture-window` /
    `capture-screen` icons, labels Region, Window, Screen.
  - Boxed list 1: Microphone (`microphone`, subtitle = default input device name, e.g.
    "Built-in Audio"), System audio (`speaker`, subtitle = default output name, e.g. "Built-in
    Speakers"). Suffix: level meter + switch. Rows 50 px, title `--fs-small`, subtitle `--fs-caption`.
  - Level meter: 8 bars 4×10 px, gap 2, `--success-bg`, last bar `--warning-bg` when hot, unlit
    `--fill-2`; meter dimmed (60 %) while the switch is off.
  - Boxed list 2: Include pointer (`cursor`, subtitle "Same setting as for captures", switch —
    bound to the capture setting), Countdown (`timer`, `.spin` 108 px, 0–10, unit " s", default 3).
  - Start Recording: `.btn.pill.destructive-action`, full width, 40 px, `record` icon, label,
    `kbd` "Enter" on `rgb(255 255 255 / 22%)`. Initial keyboard focus (ring 2 px `--accent-a50`, offset 2).
- Mode pill: `--osd-bg`, pill radius, 24 px above the bottom edge, centred. Toggle Screenshot
  (`camera`) | Video (`record`), active segment `--osd-fill-active`. Hints: "Drag to change the
  area" · `Enter` Start · `Esc` Cancel.
- Insensitive (02): row `.disabled`, prefix and text at `--dim-opacity`, meter hidden, switch off
  and insensitive; subtitle "No microphone found" / "No audio output found". An unavailable
  source segment is insensitive with the tooltip "Screen sharing for this source is not available
  on this desktop" (tooltips show on insensitive widgets). If the saved source is unavailable,
  Region is selected.
- No webcam, cursor-highlight, click-animation or quality options in this change.

### 03 Waiting for a screen

- Same component as "Waiting for a window…" (fix-window-capture-screencast 02, design.md D8):
  300 px wide, `--osd-bg` + blur 16, radius 30, padding 30/36/24, gap 18; activity ring 72 px with
  a `--annota-coral` arc; icon `capture-screen` 32 px; label "Waiting for a screen…" 15 px bold;
  Cancel pill 34 × ≥120 (initial focus); hint `Esc` Cancel.
- Shown for Region and Screen (both request a monitor source) and `--record screen`; Window
  shows "Waiting for a window…". Centred on the monitor under the pointer; non-modal; the OSD is
  the chooser's parent. The region overlay closes on Start Recording and keeps the selection.

### 04 Countdown

- Capture countdown OSD of add-screen-capture 07 except label and arc colour: ring 148 px,
  stroke 8, **`--destructive-bg` (red) arc** — the capture countdown keeps `--annota-coral`, so the
  colour tells a recording countdown from a capture countdown; numeral 72 px heavy tabular; label
  "Recording starts in N s"; hints `Enter` Start now · `Esc` Cancel.
- Region only: 2 px dashed `--destructive-bg` outline, offset 2 px outside the selection, drawn
  by the same transparent maximised countdown window. Screen and Window: no outline.
- Both vanish before the first frame is taken. Countdown 0 → no OSD.

### 05–06 Recording control

- Own small toplevel window (not a popover, not an overlay), undecorated, title "Recording",
  `--osd-bg-solid` background, radius 26 (pill), height 52, padding 0 8 0 18, gap 6, shadow as an
  OSD plus a soft 30 px drop shadow. Draggable by its whole body (window drag on any non-button area).
- Order: rec dot · time | Pause · Stop | Microphone · System audio | Minimize. Width ≈ 403 px with
  both audio sources (≈ 346 px before Minimize was added), height 52.
  - Rec dot 10 px `--destructive-bg` with a 3 px 35 % halo. Time 15 px bold, tabular, min 40 px, `M:SS`
    (`H:MM:SS` from one hour), without paused time.
  - Separators 1 × 24 px `rgb(255 255 255 / 20%)`, margin 0 4.
  - Pause / Resume: circular 36 px, `--osd-fill`; icons `pause` / `play`.
  - Stop: circular 36 px `--destructive-bg`, icon `stop`.
  - Microphone / System audio: 36 px high pill buttons, padding 0 10, icon + 5-bar meter (3×12 px,
    gap 2, `--success-bg`, last bar `--warning-bg` when hot, unlit `rgb(255 255 255 / 25%)`).
    A source not recorded has no button; with no audio, that group and its separator go.
  - Minimize: flat circular 36 px button (transparent, hover `--osd-fill-hover`), glyph
    `window-minimize` (page-local, see "Kit gaps"), tooltip "Minimize". Minimises the window
    (`gtk_window_minimize`); the recording continues. Always last, after its own separator.
  - Hover `--osd-fill-hover`; Stop hover a lighter red. Keyboard focus 2 px `rgb(120 174 237 / 85%)`
    ring, offset 1. Tab order Pause, Stop, Microphone, System audio, Minimize. Initial focus: Pause.
- Paused (06): rec dot → `pause` icon in `--annota-amber`, then the frozen time, then "Paused"
  (13 px bold, `--annota-amber`). Pause → Resume (`play`). Keyboard focus stays on that button.
- Muted: slashed icon (2 px line at −45°, see "Kit gaps"), meter unlit, tooltip "Microphone muted"
  `M` / "System audio muted" `S`. No colour change of the button: the slash carries the meaning.
- A second recording request calls `present()` on this window; GNOME decides whether it is raised
  or shows "Recording is ready".

### 07–11 Export Recording window

- Own Annota window (`AdwWindow`), 560 px wide, height by content, not resizable; header bar
  with the title "Export Recording" and the close button only. Opens right after stop; GNOME places it.
- Body padding 18/24/24, gap 18.
  - Poster row: poster frame 192 × 120, radius 9, 1 px `--borders`; duration badge bottom-right
    (`--osd-bg-solid`, 12 px bold tabular). Beside it: name 15 px bold; caption
    "1280 × 800 · 0:52 · Microphone + system audio" (`--dim-label`; audio part: "Microphone",
    "System audio", "Microphone + system audio" or "No audio").
  - Boxed list 1: File name `AdwEntryRow`, default = the working file's name without `.mkv`, the
    extension shown dimmed after the text and not editable; Folder row, subtitle path, flat
    `folder` button (opens a folder chooser); default the last used folder, else `~/Videos/Annota`.
  - Boxed list 2: Format row with toggle MP4 | WebM and subtitle "H.264 + AAC · plays almost
    everywhere" / "VP9 + Opus · open format for the web"; Include audio switch, subtitle "Mixes
    microphone and system audio into one track" (insensitive and off without audio).
  - Footer padding 0 24 24: Discard (flat, `--destructive` text) left; Export (`suggested-action`,
    default widget) right. Initial focus: File name.
- Exporting (08): all rows insensitive; footer becomes "Exporting…" + percent (tabular,
  `--dim-label`) above a 6 px progress bar, Cancel button (focused) on the right.
- Saved (09): poster row stays; below it a 64 px `--success-bg` circle with `check` 32 px,
  "Saved" (`--fs-title-2` heavy), the written file name in bold, "14.2 MB · ~/Videos/Annota"
  caption; buttons Show in Files and Close (`suggested-action`, default, focused), 140 px min,
  centred. The working file is gone by then.
- Discard (10): `AdwAlertDialog` over the window: heading "Discard Recording?", body
  "“<name>” (<duration>) will be deleted. This cannot be undone.", responses Cancel (default,
  focused) and Discard (destructive). Same alert from the close button or `Esc` before an export
  has finished.
- Error (11): `AdwBanner` (error style) below the header bar: "The recording stopped because of an
  error. What was recorded until then is kept." No button. Rest as 07.

### 12–14 Welcome page, toast, Preferences

- Welcome (based on fix-portal-shortcut-bindings 05): fourth card "Record" (`record` icon 32 px)
  after Region, Window, Screen; same 156 × 108 cards, gap 12; keycaps = the trigger of `record`
  (none if unbound). Description: "Pick what to capture or record. A capture opens here, ready to
  mark up. Every annotation stays editable until you export." Region stays the first focused card.
- Toast (13): kit `.toast`, no action button, dismiss `close`; text "The selected area is not on
  the screen you chose. Nothing was recorded." Shown in the main window, which is presented if
  it was hidden. Keyboard focus returns to the Record card.
- Preferences › Capture, portal path (based on fix-portal-shortcut-bindings 02): "Record video"
  read-only row after "Screen capture", same keycap suffix. On the GNOME-keybinding path the row
  is editable like the other three (add-screen-capture 09/10).

## Platform notes (Wayland)

- Annota cannot position its windows or keep them on top. The Recording control is an ordinary
  toplevel: GNOME places it, the user drags it, and minimises it with its Minimize button or via GNOME. The positions drawn in 05/06 are
  examples only.
- The control is not excluded from the recording: if it sits inside the recorded area it is in
  the video. Users move it outside or record a window.
- No frame or overlay around the region while recording — Annota cannot keep one on top, and it
  would be recorded. The dashed outline exists only during the countdown, inside the countdown's
  own transparent maximised window, and nothing before the countdown's end is recorded.
- GNOME shows its own screen-sharing indicator in GNOME's panel / quick settings (wherever the
  system status area is) while a ScreenCast session is open: a screen-cast icon with a small ■
  stop button (on the maintainer's desktop in the bottom panel), and it is in the video of a
  screen recording (probe P12). Whether ■ ends Annota's session is unmeasured (P5). It is drawn generically in 05/06 and
  labelled "GNOME's indicator, not drawn by Annota". Annota draws nothing in GNOME's panel.
- GNOME's window and screen choosers are GNOME UI; they appear only as labelled schematic
  placeholders (03, fix-window-capture-screencast 02). Their position and look are GNOME's.

## Kit gaps

- No `microphone-disabled` / `speaker-muted` symbolic icons: muted is a page-local CSS slash over
  `microphone` / `speaker` (06, 05 notes). Add `microphone-muted` and `speaker-muted` via
  `tools/build-icons.py` before implementation (the engineer uses Adwaita's
  `microphone-disabled-symbolic` / `audio-volume-muted-symbolic` in code).
- No success/check-circle icon: 09 uses `check` on a `--success-bg` circle.
- No `window-minimize` icon: 05/06 use a page-local glyph (10 × 2 px bar, radius 1, at y 10 of the
  16 px grid). Add `window-minimize` to the kit; in code use Adwaita's `window-minimize-symbolic`.

## Decisions (2026-11-22)

Maintainer approval of this set, with two changes now drawn:

- The Recording control gets a Minimize button at the end (05, 06), in the Tab order after System audio.
- The recording countdown ring is red (`--destructive-bg`), unlike the Coral capture countdown (04).

Designer assumptions accepted as drawn:

- Pause has the initial keyboard focus in the Recording control.
- Cancel is the default response of "Discard Recording?".
- If the saved source is not offered, the setup falls back to Region.
- The Export Recording window keeps its footer buttons (Discard left, Export right) instead of the
  header-bar actions of the add-export-share dialog.
