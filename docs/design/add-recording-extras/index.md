# Add recording extras — mockups

Design approved: _pending_

Extras on top of the core recording (`add-screen-recording`, `docs/design/add-screen-recording/`):
webcam overlay with layout and Swap, pointer highlight, drawing while recording, the video editor
v1 (trim, cut out a range, undo/redo, save a frame) entered with **Edit…** from the Export
Recording window, and GIF as a third export format.

Redrawn on 2026-11-22 in the visual language of the approved core set, after the maintainer
narrowed the scope (design.md Q1–Q3). The first set (October 2026) showed click ripples, combined
clips with Add Clip and Split, a red recording frame, a control pinned under the region and a timer
in GNOME's top bar; none of that survives here.

Scene: GNOME desktop 1280×800, the invented `d-tidewater-analytics` dashboard behind. All screens:
`src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with
`python3 tools/render-mockups.py docs/design/add-recording-extras`.
Violet boxes and labels are designer notes, not UI. **Violet "In the recording" cards show what the
video frame contains** (composited by the pipeline) — they are not on the desktop.

## Scenario → mockup

Titles are *Requirement › Scenario* exactly as in
`openspec/changes/add-recording-extras/specs/screen-recording/spec.md`.

| Requirement › Scenario | Mockup | Notes |
|---|---|---|
| Webcam overlay › Webcam in the recording | [04](png/04-recording-control-extras-light@1x.png) | Screen recording: camera circle only inside the "In the recording" frame; desktop shows no camera window. |
| Webcam overlay › No camera | [02](png/02-setup-extras-unavailable-light@1x.png) | Webcam row insensitive, "No camera found", no expander arrow, no preview. |
| Webcam overlay › Camera cannot be opened | [04](png/04-recording-control-extras-light@1x.png) | State card "Webcam unavailable": status label replaces Swap; recording continues. |
| Webcam layout › Webcam placement | [01](png/01-setup-extras-light@1x.png), [04](png/04-recording-control-extras-light@1x.png) | Default Circle · bottom right · 20 %, 24 px margin. Pixel sizes: pipeline test. |
| Webcam layout › Square in the top left | [03](png/03-webcam-options-light@1x.png) | Same controls (Shape Square, top-left cell, Size 40 %); result size: pipeline test. |
| Webcam layout › Layout preview | [03](png/03-webcam-options-light@1x.png) | Top right cell just picked; preview moved to the top right of the selection. |
| Swap to webcam › Swap | [05](png/05-swapped-webcam-light@1x.png) | Swap pressed, tooltip "Show Screen" `W`; frame shows only the camera, letterboxed. Frame timing: pipeline test. |
| Swap to webcam › No swap without webcam | [04](png/04-recording-control-extras-light@1x.png) | State card "Webcam off · no Swap". |
| Pointer highlight › Highlight ring | [04](png/04-recording-control-extras-light@1x.png) | Amber ring around the pointer inside the frame card only. Position: pipeline test. |
| Pointer highlight › Pointer drawn from metadata | [01](png/01-setup-extras-light@1x.png) | Include pointer + Highlight pointer both on. Cursor mode / single pointer image: portal + pipeline test. |
| Pointer highlight › Highlight without pointer image | [01](png/01-setup-extras-light@1x.png) | Same rows with Include pointer off; ring only: pipeline test. |
| Pointer highlight › Metadata cursor not offered | [02](png/02-setup-extras-unavailable-light@1x.png) | Row insensitive, "Not available on this desktop". |
| Draw while recording › Arrow in the recording | [06](png/06-draw-while-recording-light@1x.png) | Arrow tool, Coral. Positions in the frame: pipeline test. |
| Draw while recording › Input returns after drawing | [06](png/06-draw-while-recording-light@1x.png) → [04](png/04-recording-control-extras-light@1x.png) | `Esc`, `D`, Done or the pressed Draw button: surface closes, control returns to its narrow form. |
| Draw while recording › Draw in a window recording | [04](png/04-recording-control-extras-light@1x.png) | State card "Window recording · Draw insensitive" with the tooltip text. |
| Draw while recording › Strokes outside the region | — (no UI; pipeline test) | The surface covers the whole monitor; the region crop drops strokes outside the selection. See open question 2. |
| Drawing tools and fade › Drawing fades | [06](png/06-draw-while-recording-light@1x.png) | Fade menu card; the rectangle drawn at 0:14 is shown fading. Timing: pipeline test. |
| Drawing tools and fade › Keep until cleared | [06](png/06-draw-while-recording-light@1x.png) | "Keep until cleared" item; Clear `C` (hover card). |
| Drawing tools and fade › Step numbers count up | [06](png/06-draw-while-recording-light@1x.png) | Steps 1 and 2 on screen; Step tool card. |
| Edit a recording › Open the editor | [07](png/07-export-recording-edit-light@1x.png) → [09](png/09-video-editor-trim-light@1x.png) | Edit… (`Ctrl+E`). Opens at 0:00 with everything kept (09 shows a later, trimmed state). |
| Edit a recording › Done returns to export | [08](png/08-export-recording-edited-light@1x.png) | "0:13" + tag "Edited", poster badge 0:13. |
| Edit a recording › Working file unchanged | — (no UI; file test) | 08 note: Discard still deletes the untouched working file. |
| Edit a recording › Cancel asks first | [12](png/12-discard-edits-light@1x.png) | "Discard Edits?", Cancel default. |
| Trim and cut › Trim | [09](png/09-video-editor-trim-light@1x.png) | Handles at 0:02 / 0:15, Result 0:13.0. Export length: pipeline test. |
| Trim and cut › Cut out a range | [10](png/10-video-editor-cut-range-light@1x.png) → [11](png/11-frame-saved-toast-light@1x.png) | Selection 0:05–0:08, Cut Out Range `Delete`; 11 shows the cut band and Result 0:17.0. |
| Trim and cut › Undo a cut | [11](png/11-frame-saved-toast-light@1x.png) | Undo tooltip "Undo Cut" `Ctrl+Z`; after undo the timeline looks like 10 without the selection. |
| Trim and cut › Keep at least half a second | [09](png/09-video-editor-trim-light@1x.png) | Note "Minimum 0.5 s". |
| Save a frame › Save frame | [11](png/11-frame-saved-toast-light@1x.png) | Save Frame / `F` at 0:04, toast "Frame saved". PNG size: file test. |
| Save a frame › Open a saved frame | [11](png/11-frame-saved-toast-light@1x.png) | Toast action "Open in Annota". |
| GIF export › GIF | [13](png/13-export-gif-light@1x.png) | 1600 × 1000 recording, 15 fps, 800 px, "Height follows: 500 px", Loop forever on. |
| GIF export › GIF plays once | [13](png/13-export-gif-light@1x.png) | Loop forever switch off → no loop extension (file test). |
| GIF export › GIF of an edited recording | [08](png/08-export-recording-edited-light@1x.png) + [13](png/13-export-gif-light@1x.png) | Same window, GIF chosen after editing; frame count: file test. |
| GIF export › No sound in a GIF | [13](png/13-export-gif-light@1x.png) | Include audio insensitive, off, "GIFs have no sound". |

## Screens

| # | Screen | Light | Dark |
|---|---|---|---|
| 01 | Setup with Webcam on (collapsed) + Highlight pointer, placement preview in the selection | [light](png/01-setup-extras-light@1x.png) | [dark](png/01-setup-extras-dark@1x.png) |
| 02 | Setup: no camera, metadata cursor not offered | [light](png/02-setup-extras-unavailable-light@1x.png) | [dark](png/02-setup-extras-unavailable-dark@1x.png) |
| 03 | Webcam expanded: Shape, Position (top right picked, focused), Size; preview moved | [light](png/03-webcam-options-light@1x.png) | [dark](png/03-webcam-options-dark@1x.png) |
| 04 | Recording control with Swap + Draw, "In the recording" frame (camera + ring) + states band (Swap hover, Draw focus, window recording, webcam off, webcam unavailable) | [light](png/04-recording-control-extras-light@1x.png) | [dark](png/04-recording-control-extras-dark@1x.png) |
| 05 | Swapped to the webcam ("Show Screen"), frame = camera only, letterboxed | [light](png/05-swapped-webcam-light@1x.png) | [dark](png/05-swapped-webcam-dark@1x.png) |
| 06 | Draw while recording: strokes on the surface, widened control + states band (Fade menu, Step tool / Clear hover) | [light](png/06-draw-while-recording-light@1x.png) | [dark](png/06-draw-while-recording-dark@1x.png) |
| 07 | Export Recording with Edit… (hover tooltip) and MP4 \| WebM \| GIF | [light](png/07-export-recording-edit-light@1x.png) | [dark](png/07-export-recording-edit-dark@1x.png) |
| 08 | Export Recording after Done: 0:13 · Edited, focus on Edit… | [light](png/08-export-recording-edited-light@1x.png) | [dark](png/08-export-recording-edited-dark@1x.png) |
| 09 | Edit Recording: trimmed 0:02–0:15, end handle hover, Play tooltip | [light](png/09-video-editor-trim-light@1x.png) | [dark](png/09-video-editor-trim-dark@1x.png) |
| 10 | Edit Recording: range 0:05–0:08 selected, selection bar, Cut Out Range tooltip | [light](png/10-video-editor-cut-range-light@1x.png) | [dark](png/10-video-editor-cut-range-dark@1x.png) |
| 11 | Edit Recording: range cut, toast "Frame saved" · Open in Annota, Undo tooltip | [light](png/11-frame-saved-toast-light@1x.png) | [dark](png/11-frame-saved-toast-dark@1x.png) |
| 12 | "Discard Edits?" alert over the editor | [light](png/12-discard-edits-light@1x.png) | [dark](png/12-discard-edits-dark@1x.png) |
| 13 | Export Recording with GIF selected | [light](png/13-export-gif-light@1x.png) | [dark](png/13-export-gif-dark@1x.png) |

## Redlines

Everything not listed here is exactly as in the core redlines
(`docs/design/add-screen-recording/index.md`).

### Shortcuts

| Where | Key | Action |
|---|---|---|
| Recording control (window focused) | `W` | Swap: Show webcam only ↔ Show Screen (only with Webcam on) |
| Recording control | `D` | Draw on / off (region and screen recordings) |
| Recording control or drawing surface, Draw on | `A` · `R` · `N` | Arrow (default) · Rectangle · Step number |
| Recording control or drawing surface, Draw on | `C` | Clear drawings |
| Drawing surface or control, Draw on | `Esc` or `D` | Draw off (Done) |
| Export Recording | `Ctrl+E` | Edit… |
| Edit Recording | `Space` | Play / Pause |
| Edit Recording | `Delete` | Cut Out Range (with a selection) |
| Edit Recording | `F` | Save Frame |
| Edit Recording | `Ctrl+Z` / `Shift+Ctrl+Z` | Undo / Redo |
| Edit Recording | `Esc` | Clears a range selection first; otherwise Cancel |

D5 of `decisions.md` also lists Split `S` for the video editor: **not in this change** (no Split
button, `S` unbound in the editor). The shortcuts window lists only the keys above.

Tooltips (label + `.shortcut` chip): "Show webcam only" `W` / "Show Screen" `W`, "Draw" `D`,
"Drawing is available for region and screen recordings" (insensitive Draw, no chip),
"Arrow" `A`, "Rectangle" `R`, "Step number" `N`, "Coral" / "Azure" / "Amber", "Fade after 3 s"
(the dropdown), "Clear drawings" `C`, "Stop drawing" `Esc` (Done), "Edit Recording" `Ctrl+E`,
"Play" / "Pause" `Space`, "Undo <action>" `Ctrl+Z` (e.g. "Undo Cut", "Undo Trim"), "Redo <action>"
`Shift+Ctrl+Z`, "Save Frame" `F`, "Cut Out Range" `Delete`.

### 01–03 Recording setup additions

- Popover unchanged in width (380 px), padding 12, gap 12. A **third boxed list** after
  Include pointer / Countdown, before Start Recording:
  - **Webcam** — `AdwExpanderRow` with enable switch: prefix `camera-video` glyph (page-local,
    see Kit gaps), title "Webcam", subtitle = summary "Circle · bottom right · 20 %" (shape ·
    position · size, lower-case position names: top left, top centre, top right, middle left,
    centre, middle right, bottom left, bottom centre, bottom right) or "Off". Suffix switch +
    expander arrow (`pan-down` / `pan-up`, 24 px). Default off.
  - **Highlight pointer** — prefix `pointer-highlight` glyph (page-local), subtitle "Ring around the
    pointer in the video", switch, default off.
  - Rows 50 px, title `--fs-small`, subtitle `--fs-caption` (as the core rows).
- Expanded Webcam (03), content rows padded 8/12/10/12, separated by 1 px `--separator`:
  - **Shape**: caption "Shape" (13 px), then a full-width `.toggle-group` with Circle · Rounded
    square · Square, each with a 12 px outline glyph of the shape (2 px stroke) and the label.
  - **Position**: title "Position", subtitle = current cell ("Top right"); suffix a 3 × 3 grid of
    toggle cells **32 × 24 px**, gap 2, in a `--fill-1` tray (radius 9, padding 3). Cell glyph: 6 px
    dot `--fill-3`; picked cell `--accent-bg` with an 8 px white dot. Keyboard: arrow keys move
    focus between cells, `Space`/`Enter` pick; one Tab stop for the grid. Each cell's accessible
    label is its position name. (The 24 px height is below the 32 × 32 target rule; acceptable
    for a mouse-optional grid with full keyboard support — or make cells 32 × 32, which adds 24 px
    to the popover.)
  - **Size**: label "Size", `GtkScale` 10–40, step 1, marks none, value label "20 %" (tabular).
  - Total popover height expanded ≈ 670 px; the popover then sits 8 px below the top bar
    (top 40) and keeps its arrow on the selection. On monitors shorter than ~760 px it flips below
    or GTK shrinks it; the content must scroll then (`GtkScrolledWindow`, propagate-natural-height).
- **Placement preview** (region overlay only, 01/03): outline in the chosen shape, 2 px dashed
  white, `rgb(0 0 6 / 35%)` fill, 1 px dark outer line; centred `camera-video` glyph; label chip
  "Webcam" (12 px bold, `rgb(0 0 6 / 70%)`, radius 6) 28 px outside the preview, towards the
  selection's centre. Size = Size % × the selection's shorter side; margin 24 px. Updates live
  while dragging the selection or the slider. Hidden when Webcam is off or no camera exists.
  Screen and Window sources have no selection, so no preview (the summary subtitle suffices).
- Unavailable (02): row `.disabled` look as the core ("No microphone found"): prefix, text and
  arrow at `--dim-opacity`, switch off and insensitive; Webcam shows no expander arrow.

### 04–06 Recording control additions

- Same window, pill, heights, separators and colours as core 05. Order:
  rec dot · time | Pause · Stop | Microphone · System audio | **Swap · Draw** | Minimize.
  Width with both audio sources and Swap + Draw ≈ 496 px. Tab order: Pause, Stop, Microphone,
  System audio, Swap, Draw, Minimize.
- **Swap**: flat circular 36 px, `swap` glyph (page-local). Only when Webcam was on. Swapped:
  pressed look `--osd-fill-active` (hover `rgb(255 255 255 / 34%)`), tooltip / accessible name
  "Show Screen".
- **Draw**: flat circular 36 px toggle, kit `pen`. Window recording: insensitive (contents at 40 %),
  tooltip "Drawing is available for region and screen recordings". With Webcam off the group is
  Draw only (no empty slot).
- **Webcam unavailable**: the Swap slot shows a status label instead: 36 px high pill, padding
  0 12 0 10, `rgb(245 165 36 / 14%)` fill, kit `warning` icon + "Webcam unavailable" (13 px bold,
  `--annota-amber`). Tooltip "The camera could not be opened. Recording continues without it."
  Not focusable; announced once via its accessible label when the control appears. Stays until stop.
- **Draw on** (06): Draw pressed (`--osd-fill-active`); right after it a tool group (height 44,
  radius 22, `rgb(255 255 255 / 6%)`, padding 0 4, gap 6):
  - Tools Arrow (kit `arrow`) · Rectangle (page-local outline rect, see Kit gaps — not the
    generic `shape` icon) · Step number (kit `step`): 36 px circular, one active.
  - Separator (24 px, margin 0 2), three swatches 22 px: Coral `--annota-coral` (default), Azure
    `--annota-azure`, Amber `--annota-amber`; selected = 2 px ring in the pill colour + 2 px white.
  - Separator, **Fade** dropdown: 36 px pill `--osd-fill`, `timer` icon, the selected item text,
    `pan-down`; menu items "Fade after 1 s", "Fade after 3 s" (default), "Fade after 5 s",
    "Fade after 10 s", "Keep until cleared", check on the current one.
  - **Clear**: circular 36 px, kit `edit-clear`.
  - **Done**: 36 px pill, `--accent-bg`, label "Done", padding 0 14; turns Draw off.
  - Width with both audio sources, no Swap ≈ 960 px; with Swap ≈ 1000 px.
- Strokes (drawn by the surface and composited by the pipeline with the same renderer and
  styles as the editor's tools): Arrow 6 px line, filled head; Rectangle 4 px stroke, radius 6; Step
  number 34 px disc, white 17 px heavy numeral, 2 px white ring; all with the annotation tools'
  white halo/shadow. Fade: full opacity for the fade time, then 0.5 s linear fade-out.
- Drawing surface: undecorated, transparent, fullscreen on the recorded monitor, input region =
  whole monitor, cursor `crosshair`. No border, no hints, no toolbar.

### 07–08, 13 Export Recording additions

- **Edit…** (`Ctrl+E`): normal `.btn` text button (34 px, `--fill-1`) in the poster row's meta
  column, 8 px below the caption, left-aligned. Placement rationale: Edit… changes *what* is
  exported (the recording in the poster), not *how* it is written; the footer keeps the core's
  Discard (left) / Export (right, default) pair; putting a third action into the footer would
  sit next to Discard and invite mis-clicks. Tab order: Edit…, File name, Folder, Format, …,
  Discard, Export; initial focus stays on File name (07); after Done focus is on Edit… (08).
- **Edited** (08): caption line 1 "1280 × 800 · 0:13" followed by the tag "Edited" (20 px pill,
  `--accent-a12` background, `--accent` 12 px bold text); the audio part moves to a second
  caption line. Poster badge = edited duration; poster = first kept frame. No tag when nothing
  was edited (an editor closed by Done without edits leaves the window unchanged).
- **Format** toggle MP4 | WebM | GIF. Subtitles: "H.264 + AAC · plays almost everywhere" /
  "VP9 + Opus · open format for the web" / "Animated image · no sound, larger files". The file
  name's dimmed extension follows (`.gif`).
- **GIF rows** (13), inserted after Format in the same boxed list: Frame rate (subtitle
  "5–30 fps", `.spin` 128 px, "15 fps"); Width (subtitle "Height follows: 500 px", live, rounded to
  even; `.spin` 140 px, "800 px", 160 … recording width, step 10); Loop forever (subtitle "Plays
  again from the start", switch on). Include audio: `.disabled`, off, "GIFs have no sound";
  restores its previous value when MP4/WebM is chosen again. Window grows by 3 rows (≈ 174 px);
  GNOME keeps it on screen.

### 09–12 Edit Recording window

- `AdwWindow`, 1100 × 730 (default; resizable, min 900 × 640), modal and transient for Export
  Recording. Header bar: Cancel (text button, start), title "Edit Recording", subtitle = recording
  name, Done (`suggested-action`, end). No close button (Cancel and `Esc` take its role).
- **Preview** (flex, `--canvas-bg`): the frame at the playhead, aspect-fitted, max 544 × 340 at the
  default size, shadow as the canvas image. Toasts (`AdwToastOverlay`) live over this area only.
- **Transport bar** 54 px, `--window-bg`, 1 px separators top and bottom, padding 0 18:
  start Undo · Redo (flat image buttons, insensitive when unavailable); centre Play/Pause
  (circular 40 px, `play`/`pause`) + time "0:04.2 / 0:20.0" (15 px bold tabular; total part dimmed;
  position and total are **recording time**, `M:SS.d`); end "Save Frame" (`.btn`, `image` icon).
- **Timeline** 236 px, `--view-bg`, 50 px per second at the default width (scales to fit the
  whole recording; no zoom in v1):
  - Gutter 48 px with `image` (video) and `speaker` (audio) glyphs, dimmed.
  - Ruler 22 px, labels every 2 s (11 px tabular, `--dim-label`), ticks every second.
  - Thumbnail strip 64 px (radius 6, 1 px `--borders`), one thumbnail per second.
  - Audio lane 40 px, `--fill-1`, one **mixed** waveform (microphone + system audio), bars 2 px,
    gap 1, `--accent-bg` 75 %. No lane without audio.
  - Trim: kept range framed by 3 px `--accent-bg` lines top and bottom; handles 14 × 118 px
    `--accent-bg`, outer corners radius 6, two 2 × 20 px white grips; hover `--accent-bg-hover`
    + 3 px `--accent-a25` halo, tooltip "Start 0:02.0" / "End 0:15.0". Trimmed parts dimmed
    `rgb(0 0 6 / 58%)`. Handles stop 0.5 s apart (25 px here).
  - Playhead: 2 px `--view-fg` line over all lanes, 14 × 12 px pentagon knob on the ruler.
    Click the ruler/strip to seek.
  - Range selection (10): drag across the strip; `--accent-a25` fill, 2 px `--accent-bg` border,
    radius 4, 8 × 28 px edge grips. Playhead jumps to its start.
  - Cut range (11): stays on the timeline as a hatched band (−45° stripes, `rgb(0 0 6 / 62%)` /
    `40 %`), 2 px `--destructive-bg` edges, centred chip "Cut · 3.0 s" (`--osd-bg-solid`). Playback
    skips it.
  - Bottom bar 68 px (1 px separator), 13 px: default "Keep 0:02.0 – 0:15.0 · [1 cut ·] Result
    0:13.0" + dimmed hint "Drag the blue edges to trim · drag across the strip to select a range";
    with a selection: "Selection", From / To `.spin` 118 px (0.1 s steps), length "3.0 s",
    "Clear Selection" (flat), "Cut Out Range" (`.btn`).
- **Toast** (11): kit `.toast`, "Frame saved", action button "Open in Annota", dismiss. File
  `<recording name> – frame M-SS.png` (en dash), e.g. "Recording 2026-11-22 10-42-07 – frame
  0-04.png", into the recordings folder, " (2)" suffix if taken.
- **Discard Edits?** (12): `AdwAlertDialog`, body "The changes made in this editing session will be
  lost.", responses Cancel (default, focused) and Discard (destructive). Only when the session has
  changes; otherwise Cancel closes at once.

## Platform notes (Wayland)

- **Composited, not windows.** Annota cannot keep a window on top and has no global input on
  GNOME Wayland. The webcam picture, the pointer ring and the swap are produced by the recording
  pipeline (`compositor`); the desktop shows none of them. The mockups show them only in violet
  "In the recording" frame cards.
- **Pointer ring** uses the ScreenCast metadata cursor mode; with it, the stream carries no
  embedded pointer, so Annota draws the pointer image itself when Include pointer is on.
- **Drawing surface takes all input.** While Draw is on, apps under the surface get no clicks; the
  user cannot interact with the recorded app until Draw is off. Hence Done, `Esc` and `D`.
- **Its own pixels are in a screen recording.** The surface sits on the recorded monitor, so the
  ScreenCast stream captures whatever it renders. It therefore renders nothing but strokes (no
  chrome). See open question 1 on strokes being both rendered and composited.
- **The control may be hidden by the surface** — GNOME decides stacking of two Annota windows,
  one fullscreen. Probe E3; every drawing action also has a key, so drawing works with the control
  hidden.
- The Recording control (also widened while drawing) is in a screen recording when it sits on the
  recorded monitor, exactly as in the core.

## Open questions

1. **Strokes twice in the video?** The surface must show the strokes as feedback, and its pixels
   are captured by the ScreenCast stream (screen and region recordings alike, region being a crop
   of the monitor stream). If the pipeline composites the same strokes on top, both copies coincide
   only if renderer, position and fade timing match exactly (stream latency makes fades differ).
   Options: (a) the surface renders the strokes and the pipeline adds nothing (contradicts "composited
   by the pipeline"), (b) both render, pipeline copy identical and drawn on top, (c) the surface is
   excluded from the stream (not possible on GNOME without compositor-private API). Needs a design.md
   decision / probe.
2. While drawing in a **region** recording the user cannot see where the region is (no frame, no
   chrome). An outline drawn by the surface 2 px *outside* the selection would not be in the
   cropped video (like the countdown outline). Not drawn; proposal for the maintainer.
3. "Recordings folder" for saved frames: the folder of the working files or the export folder
   (`~/Videos/Annota`)? The spec says "recordings folder"; drawn as the latter.
4. Editor keyboard navigation beyond the spec (←/→ frame step, Home/End, `[`/`]` for trim at the
   playhead) is not specified; not drawn.
5. Position grid cells 32 × 24 vs. the 32 × 32 target rule (see redlines 01–03).

## Kit gaps

Page-local glyphs in these mockups; add to `kit/icons/` via `tools/build-icons.py` (code uses
the Adwaita names in brackets where one exists):

- `camera-video` — Webcam row, placement preview (`camera-video-symbolic`).
- `camera-swap` — Swap button (`camera-switch-symbolic` or an own glyph).
- `pointer-highlight` — Highlight pointer row (ring + pointer; own glyph).
- `draw-rectangle` — Rectangle drawing tool (outline rectangle; the kit `shape` icon is the
  generic shape tool).
- `window-minimize` — still missing from the core set (`window-minimize-symbolic`).
- Expander arrows use page-local `pan-down` / `pan-up` (kit has `pan-down` only).
- The webcam picture is an invented illustration drawn inline (no sample-content asset); a
  `sample-content/webcam-person.svg` would make it reusable.
