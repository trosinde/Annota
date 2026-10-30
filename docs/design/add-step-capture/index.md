# Add step capture — mockups

Step capture records clicks in another app (`a-harbor-backup`) and turns them into a guide edited in a step list window that follows the editor header-bar pattern. Export goes to the Step Guide template document, PDF or Markdown.

All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with `python3 tools/render-mockups.py docs/design/add-step-capture`.

## Scenario → mockup

| Scenario / state | Mockup | Notes |
|---|---|---|
| Record steps (start) | [01-step-capture-started](png/01-step-capture-started-light@1x.png) | Floating control: Coral counter “0 Steps”, Pause, Stop, Undo last step. Shell indicator. Hint pill until the first click. |
| Record steps › Three clicks | [02-step-capture-three-clicks](png/02-step-capture-three-clicks-light@1x.png) | Each click flashes a numbered Coral marker at the click position (earlier ones fade); OSD label names the new step. Counter shows 3. |
| Record steps (Pause) | [03-step-capture-paused](png/03-step-capture-paused-light@1x.png) | Counter label “Paused”, Play button pressed, pill “Paused · clicks are not recorded”. |
| Step descriptions › Button name | [04-step-list-editor](png/04-step-list-editor-light@1x.png) | Auto text from accessibility: `Click "Start backup"`, `Turn on "Back up on battery"`; magic-wand caption names role + container. |
| Step descriptions (editable) | [05-edit-step-description](png/05-edit-step-description-light@1x.png) | Inline entry for title + optional note; caption “Edited · was … · Restore”. |
| Guide output › edit list (reorder) | [06-reorder-steps](png/06-reorder-steps-light@1x.png) | Dragged card lifts (shadow, slight tilt), 4 px accent drop line marks the target. |
| Guide output › edit list (delete) | [07-step-deleted-undo](png/07-step-deleted-undo-light@1x.png) | Step removed, numbers renumber, toast with Undo. |
| Empty guide | [08-guide-empty](png/08-guide-empty-light@1x.png) | Status page, Export disabled. |
| Guide output › Markdown | [09-export-markdown](png/09-export-markdown-light@1x.png), [11-export-done](png/11-export-done-light@1x.png) | Export Guide dialog with three choices; Markdown preview of the folder tree (.md + images folder). 11: toast with Open Folder. |
| Guide output › PDF | [10-export-pdf-progress](png/10-export-pdf-progress-light@1x.png) | Progress state while writing. |
| Guide output › template document | [12-template-document](png/12-template-document-light@1x.png) | Step Guide template opened in the editor shell: title, meta, numbered steps with cropped images and markers. |

## Screens

- [01-step-capture-started](png/01-step-capture-started-light@1x.png) · [dark](png/01-step-capture-started-dark@1x.png)
- [02-step-capture-three-clicks](png/02-step-capture-three-clicks-light@1x.png) · [dark](png/02-step-capture-three-clicks-dark@1x.png)
- [03-step-capture-paused](png/03-step-capture-paused-light@1x.png) · [dark](png/03-step-capture-paused-dark@1x.png)
- [04-step-list-editor](png/04-step-list-editor-light@1x.png) · [dark](png/04-step-list-editor-dark@1x.png)
- [05-edit-step-description](png/05-edit-step-description-light@1x.png) · [dark](png/05-edit-step-description-dark@1x.png)
- [06-reorder-steps](png/06-reorder-steps-light@1x.png) · [dark](png/06-reorder-steps-dark@1x.png)
- [07-step-deleted-undo](png/07-step-deleted-undo-light@1x.png) · [dark](png/07-step-deleted-undo-dark@1x.png)
- [08-guide-empty](png/08-guide-empty-light@1x.png) · [dark](png/08-guide-empty-dark@1x.png)
- [09-export-markdown](png/09-export-markdown-light@1x.png) · [dark](png/09-export-markdown-dark@1x.png)
- [10-export-pdf-progress](png/10-export-pdf-progress-light@1x.png) · [dark](png/10-export-pdf-progress-dark@1x.png)
- [11-export-done](png/11-export-done-light@1x.png) · [dark](png/11-export-done-dark@1x.png)
- [12-template-document](png/12-template-document-light@1x.png) · [dark](png/12-template-document-dark@1x.png)

## Redlines
- **Start:** main menu / capture popover item “Step Capture”; `annota --capture steps`. Stop with the control or `Shift+Ctrl+Print` (same global stop as recording).
- **Floating control:** `.osd-toolbar.pill`, bottom centre 24 px above the screen edge, never captured. Counter: 26 px Coral circle with white number + label “Step/Steps/Paused”. Buttons: Pause/Resume, Stop (destructive pill with label), Undo Last Step (`undo`).
- **Click marker:** Coral circle r = 22 image px, 4 px white ring, outer halo `--annota-coral-a20` (r + 14), white bold number. Shown on screen for 600 ms, always baked into the step image (toggle *Click markers* in the guide sidebar).
- **Step image:** active window, cropped “Around click” (default 1200 × 670 around the click; options Whole window, Around click).
- **Step card:** card, 12 px padding, grid: drag handle 24 · number 36 (32 px Coral disc) · thumbnail 240×134 (radius 6, 1 px border) · text · actions (Edit image in editor `pen`, Delete `trash`). Text: caption with `magic-wand` “From accessibility · <role> · <container>” or `text` “Edited · was … · Restore”; title `--fs-title-4` bold; note `--fs-small` dim (“Add a note…” placeholder). Selected card: 2 px accent ring.
- **Description rules:** button/menu item → `Click "<name>"`; switch → `Turn on/off "<name>"`; check → `Check "<name>"`; entry → `Click in "<name>"`; unknown → `Click here`.
- **Guide sidebar (300 px):** Title entry row, Template dropdown, Image crop dropdown, Click markers switch; Marker colour swatches (Coral default); *Add Steps…*; local note.
- **Export Guide dialog:** 580 px; radio rows *Step Guide document* (`template`), *PDF* (`image`), *Markdown* (`text`); Folder (`~/Documents/Guides`), Image width dropdown (800/1200/1600 px). Markdown writes `<slug>.md` + `<slug>-images/step-NN.png`.

Shared: editor shell = kit `template.html` (header 47 px, tool strip 52 px, properties 300 px), 6 px grid, Annota Coral `#ff5a4e` for annotation ink, Adwaita blue for UI accent. No accounts, cloud or network anywhere.
