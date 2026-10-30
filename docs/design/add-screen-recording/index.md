# Add screen recording — mockups

Recording UI floats over the GNOME desktop (1280×800 shown; same layout on 1920×1080). Captured content: `d-tidewater-analytics`. Webcam always shows a geometric avatar. The video editor uses the editor header-bar pattern with a timeline at the bottom.

All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with `python3 tools/render-mockups.py docs/design/add-screen-recording`.

## Scenario → mockup

| Scenario / state | Mockup | Notes |
|---|---|---|
| Start and stop (setup) | [01-recording-setup](png/01-recording-setup-light@1x.png) | Region overlay + recording setup popover: Region/Window/Screen, Microphone and System audio with level meters, Webcam (summary row), Highlight cursor, Animate clicks, Countdown spin (0–10 s, default 3). Bottom pill switches Screenshot/Video. |
| Webcam overlay (shape, position, size) | [02-webcam-overlay-options](png/02-webcam-overlay-options-light@1x.png) | Webcam row expanded: shape picker circle/rounded square/square, 3×3 position grid, size slider. Live preview inside the region. |
| Start and stop (countdown) | [03-countdown](png/03-countdown-light@1x.png) | 3-2-1 countdown disc centred in the region, pill “Recording starts in 3 s · Cancel”. Region frame turns red dashed. |
| Start and stop › Stop by hotkey | [04-recording-control](png/04-recording-control-light@1x.png) | Floating control under the region: red dot, elapsed time, Pause, Stop (tooltip “Stop (Shift+Ctrl+Print)”), Draw, Mic (mute + meter), Swap, more. Shell top bar shows the red recording indicator with time. After stop the video editor (08) opens. |
| Cursor effects › Click ripple | [04-recording-control](png/04-recording-control-light@1x.png) | Amber highlight ring around the cursor; Coral double ripple at the click. |
| Audio › Mute mic | [05-paused-mic-muted](png/05-paused-mic-muted-light@1x.png) | Mic button pressed with slash, meter grey, tooltip “Microphone muted · M”. Also shows Paused state (pause glyph in amber, Play button, “Paused” OSD). |
| Draw while recording › Arrow overlay | [06-draw-while-recording](png/06-draw-while-recording-light@1x.png) | Draw toggled on → secondary OSD bar: arrow/rectangle/step, 3 colours (Coral default), fade dropdown (“Fade after 3 s” / “Keep until cleared”), Clear. A fading rectangle + step 1 shown at 50 % opacity. |
| Webcam overlay › Swap | [07-swapped-full-webcam](png/07-swapped-full-webcam-light@1x.png) | Swap pressed: the region shows the full webcam, label “Webcam only”, button reads “Show Screen”. |
| Video editing (trim, combine) | [08-video-editor-trim](png/08-video-editor-trim-light@1x.png) | Preview + timeline: video track with thumbnails for two combined clips, Mic and System waveform tracks, yellow trim handles with hatched trimmed parts, playhead. Sidebar: Trim times, Clips list (drag to reorder, Add Clip…). |
| Video editing (cut range, frame grab) | [09-video-editor-cut-range](png/09-video-editor-cut-range-light@1x.png) | Selected range in red “Cut 0:24.0 – 0:30.0” with the muted mic gap visible; *Cut Out Range* active; sidebar From/To + *Cut Out 6.0 s*. Toast after *Save Frame*. |
| Video editing and export › GIF | [10-export-gif](png/10-export-gif-light@1x.png), [11-export-progress](png/11-export-progress-light@1x.png) | Export Video dialog: MP4/WebM/GIF; GIF group with frame rate (15 fps), width (800 px, height follows), Loop forever; size estimate. 11 = progress state. |
| Export MP4/WebM | [12-export-mp4](png/12-export-mp4-light@1x.png) | Quality dropdown (H.264 / VP9 for WebM) and Include audio. |

## Screens

- [01-recording-setup](png/01-recording-setup-light@1x.png) · [dark](png/01-recording-setup-dark@1x.png)
- [02-webcam-overlay-options](png/02-webcam-overlay-options-light@1x.png) · [dark](png/02-webcam-overlay-options-dark@1x.png)
- [03-countdown](png/03-countdown-light@1x.png) · [dark](png/03-countdown-dark@1x.png)
- [04-recording-control](png/04-recording-control-light@1x.png) · [dark](png/04-recording-control-dark@1x.png)
- [05-paused-mic-muted](png/05-paused-mic-muted-light@1x.png) · [dark](png/05-paused-mic-muted-dark@1x.png)
- [06-draw-while-recording](png/06-draw-while-recording-light@1x.png) · [dark](png/06-draw-while-recording-dark@1x.png)
- [07-swapped-full-webcam](png/07-swapped-full-webcam-light@1x.png) · [dark](png/07-swapped-full-webcam-dark@1x.png)
- [08-video-editor-trim](png/08-video-editor-trim-light@1x.png) · [dark](png/08-video-editor-trim-dark@1x.png)
- [09-video-editor-cut-range](png/09-video-editor-cut-range-light@1x.png) · [dark](png/09-video-editor-cut-range-dark@1x.png)
- [10-export-gif](png/10-export-gif-light@1x.png) · [dark](png/10-export-gif-dark@1x.png)
- [11-export-progress](png/11-export-progress-light@1x.png) · [dark](png/11-export-progress-dark@1x.png)
- [12-export-mp4](png/12-export-mp4-light@1x.png) · [dark](png/12-export-mp4-dark@1x.png)

## Redlines
- **Shortcuts:** start recording = `Enter` in the setup; stop = `Shift+Ctrl+Print` (global); pause has no global shortcut in this change; mute mic = `M` while the control has focus; Export = `Ctrl+E`; Copy frame = `Ctrl+C`; Split = `S`; Cut Out Range = `Delete`; Save Frame = `F`; Play/Pause in editor = `Space`.
- **Setup popover:** 380 px wide, 12 px padding, gap 12. Rows 44 px with `--fs-small` titles. Level meter 8 bars 4×10 px, green (`--success-bg`), last two amber. Start button: destructive pill, full width, 40 px, `record` icon + `Enter` key hint.
- **Countdown disc:** 180 px, `--osd-bg`, numeral 96 px heavy, tabular.
- **Recording frame:** 2 px dashed `--destructive-bg`, outset 2 px. Not recorded.
- **Floating control:** `.osd-toolbar.pill`, placed 18 px under the region (or inside the bottom edge for full screen), never recorded. Order: rec dot · time (15 px bold, tabular, min 62 px) | Pause · Stop (circular destructive) | Draw · Mic (+5-bar meter) · Swap (camera icon + label) · more (`view-more`: countdown, cursor effects, hide control).
- **Cursor effects:** highlight ring r = 26 px, `--annota-amber` 2.5 px, fill amber 28 %; click ripple two Coral rings (r 30 / 40), 400 ms.
- **Webcam overlay:** default circle, bottom right, 160 px, 3 px white border + shadow; 18 px from region edges.
- **Draw bar:** second OSD row 12 px below the control: tools arrow, rectangle, step; swatches Coral/Amber/Azure (small); dropdown “Fade after 3 s” (1/3/5/10 s, Keep until cleared); Clear (`edit-clear`).
- **Video editor:** header like the editor (record-new / undo / redo | title “Recording HH:MM”, subtitle “0:52 · 2 clips · W × H · Edited” | Copy frame, Export, menu). Preview area `--canvas-bg`. Timeline 252 px: transport bar (prev, Play circular suggested, next, time “00:12.4 / 00:52.0”, Split, Cut Out Range, Save Frame, Add Clip, zoom); track headers 120 px (Video, Mic, System with speaker toggle); ruler every 5 s; video track 56 px, audio tracks 40 px. Trim handles 12 px `--annota-amber`, trimmed part hatched; cut range `--destructive-bg` 22 % fill with 2 px edges and label; playhead 2 px `--accent-bg`.
- **Export Video dialog:** 560 px. File name, Folder (`~/Videos/Annota`), Format toggle MP4/WebM/GIF. GIF: Frame rate spin (5–30, default 15), Width spin (default 800 px, height label follows), Loop forever (on). MP4/WebM: Quality dropdown, Include audio. Footer: trimmed range + size estimate; progress row with spinner, frame counter and remaining time.

Shared: editor shell = kit `template.html` (header 47 px, tool strip 52 px, properties 300 px), 6 px grid, Annota Coral `#ff5a4e` for annotation ink, Adwaita blue for UI accent. No accounts, cloud or network anywhere.
