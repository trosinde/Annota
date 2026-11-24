# Add recording extras — mockups (not yet redrawn)

> **Moved unchanged from the first add-screen-recording set (October 2026). They predate the
> core redesign (control window, no region frame, Export Recording window) and must be redrawn
> against the approved core mockups at this change's own design gate.**
>
> In particular they still show things the core design removed for platform reasons: a red
> dashed frame around the region while recording, a control pinned 18 px under the region and
> kept out of the video, an Annota timer pill in GNOME's top bar, and a video editor that opens
> right after stop. Do not implement from these PNGs. Core reference:
> `docs/design/add-screen-recording/index.md`.

Files: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png` (rendered October 2026; not
re-rendered since the move).

## Screens

| # | Screen | Shows | Future requirement it illustrates |
|---|---|---|---|
| 01 | [01-recording-setup-extras](png/01-recording-setup-extras-light@1x.png) | The old recording setup popover with the extra rows: Webcam (summary row "Circle · bottom right"), Highlight cursor, Animate clicks. | Webcam overlay; Cursor highlight; Click animation (setup entries) |
| 02 | [02-webcam-overlay-options](png/02-webcam-overlay-options-light@1x.png) | Webcam row expanded: shape circle / rounded square / square, 3×3 position grid, size slider, live preview inside the region. | Webcam overlay (shape, position, size) |
| 03 | [03-recording-control-effects](png/03-recording-control-effects-light@1x.png) | Old floating control with Draw, Swap and more; amber cursor highlight ring and Coral click ripples; webcam avatar in the region. | Cursor highlight; Click animation; control entries for Draw and Swap |
| 04 | [04-draw-while-recording](png/04-draw-while-recording-light@1x.png) | Draw toggled on: second OSD bar with arrow / rectangle / step, three colours, fade dropdown ("Fade after 3 s" / "Keep until cleared"), Clear; fading annotations. | Draw while recording |
| 05 | [05-swapped-full-webcam](png/05-swapped-full-webcam-light@1x.png) | Swap pressed: the region shows the full webcam, label "Webcam only", button "Show Screen". | Webcam overlay › Swap to full webcam |
| 06 | [06-video-editor-trim](png/06-video-editor-trim-light@1x.png) | Video editor: preview, timeline with two combined clips, Mic and System waveforms, trim handles, playhead; sidebar Trim times and Clips list. | Video editor: trim, combine clips |
| 07 | [07-video-editor-cut-range](png/07-video-editor-cut-range-light@1x.png) | Selected range "Cut 0:24.0 – 0:30.0", Cut Out Range, sidebar From/To; toast after Save Frame. | Video editor: cut a range, save a frame |
| 08 | [08-export-gif](png/08-export-gif-light@1x.png) | Old Export Video dialog in GIF mode: frame rate, width (height follows), Loop forever, size estimate. | GIF export (gifski) |

## Redraw checklist for this change's design gate

- The setup rows (01, 02) go into the core setup popover of add-screen-recording 01 below the
  existing rows, without breaking its 380 px width.
- Control additions (03, 04, 05) go into the core Recording control window (add-screen-recording
  05): an own toplevel placed by GNOME, not kept on top, recorded when inside the area.
- Cursor highlight, click ripples, webcam and drawings must be composited into the stream by the
  pipeline — nothing may rely on an Annota overlay window being on top during recording.
- The video editor (06, 07) and GIF (08) must start from the core Export Recording window
  (add-screen-recording 07), e.g. an "Edit…" entry and a GIF format, instead of replacing it.
- Shortcuts from `docs/design/decisions.md` D5 (video editor: Play `Space`, Split `S`, Cut range
  `Delete`, Save frame `F`) still apply.
