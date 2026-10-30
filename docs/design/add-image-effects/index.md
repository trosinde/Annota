# add-image-effects — mockups

Window 1280 × 800, editor shell from `docs/design/kit/template.html`. Every screen has
`png/<name>-{light,dark}@{1,2}x.png`; sources in `src/*.html`.

The effects live in the right sidebar, reached through a **Tool | Effects** switcher at the top
of the properties pane (Ctrl+Shift+E). The canvas shows the rendered result on a checkerboard
(transparent margin created by shadows, edges and perspective) with the output size under it.

## Scenario → mockup

| Requirement · Scenario | Mockup | Notes |
|---|---|---|
| Non-destructive effects · stack | `01-effects-panel-border` | Stack of three effects (Border, Torn Edge, Shadow), drag handles, per-effect switch; Border selected and edited below. |
| Non-destructive effects · add effect | `02-add-effect-menu` | "+" opens a searchable menu grouped Frame / Colour / Overlay with the full catalogue. |
| Non-destructive effects · empty | `03-effects-empty` | Status page "No Effects" + Add Effect…, quick effects listed below. |
| Non-destructive effects · Edit later | `15-edit-later-reopened` | Reopened `harbor-backup.annota`: Border still editable (width spin focused, 6 → 12 px), Shadow toggled off and dimmed. |
| Effect catalogue · Torn bottom edge | `04-edges-torn-bottom` | Edges = Torn, Sides = Bottom only, size 18 px, roughness 55. Only the bottom edge is torn. |
| Effect catalogue · perspective, shadow, reflection | `05-perspective-shadow-reflection` | Tilt 6°, rotation −16°, presets Left / Flat / Right; reflection fades below. |
| Effect catalogue · shadow, page curl | `06-shadow-page-curl` | Shadow colour, X/Y offset, blur, opacity; page curl corner picker (four corners). |
| Effect catalogue · colour adjustment, filters | `09-colour-adjustment` | Brightness, contrast, hue, saturation, gamma + Reset; filter row Grayscale / Sepia / Invert / Sharpen / Soften. |
| Effect catalogue · effect on a selection, replace colour | `10-effect-on-selection` | Grayscale limited to a 660 × 130 selection (the two right-hand KPI cards); Replace Colour controls (from, to, eyedropper, tolerance). |
| Effect catalogue · watermark | `11-watermark` | Image + text, 3 × 3 position grid, scale, opacity, Overlay / Emboss / Tiled. |
| Capture info · Footer | `07-capture-info-footer` | Window capture with Date and Time + Application Name checked → dark footer "Captured 30 Oct 2026, 14:22 · App Harbor Backup". Field checkboxes for all six fields. |
| Capture info · missing metadata | `08-capture-info-screen` | Screen capture: Application Name and Window Title disabled ("Not available for screen captures"); OS, size and custom text in the footer. |
| Quick effects · save | `12-save-quick-effect` | Dialog: name, list of included effects, "Apply After Every Capture" switch. |
| Quick effects · apply / manage | `13-quick-effects` | Quick-effect cards with preview thumbnails; "Docs Frame" marked **Auto**; context menu Apply, Apply After Every Capture ✓, Rename…, Update From Current Stack, Delete. |
| Quick effects · Auto-apply | `14-auto-apply-after-capture` | New capture opens with the "Docs Frame" stack applied; banner "Quick effect "Docs Frame" was applied after capture" + Remove. Undo starts disabled. |

## Redlines

**Sidebar switcher.** Full-width `.toggle-group`: Tool | Effects. The Effects toggle carries a
neutral badge with the stack count. Ctrl+Shift+E opens Effects.

**Effect stack.** Group header "Effect Stack", description "<target> · applied top to bottom"
(target = "Whole Image" or "Selection W × H"), "+" flat image button on the right (pressed while
the menu is open). Boxed list rows 50 px: drag-handle icon (45 % opacity) · title + one-line
summary · switch. Selected row = `.row.selected`. Disabled effects: switch off, text at 55 %.
Footer bar: "Save as Quick Effect…".

**Add-effect menu.** Popover 240 px, search entry on top, sections Frame (Border, Edges, Shadow,
Perspective, Page Curl, Reflection), Colour (Colour Adjustment, Filter, Replace Colour),
Overlay (Watermark, Capture Info).

**Effect editors** (below the stack, 18 px gap):
- Border — 7 Annota swatches + custom (default Ink), Width 1–48 px (default 6), Radius 0–48 px (default 12).
- Edges — Torn | Wave | Zigzag | Fade; Sides Top | Right | Bottom | Left (multi-select); Size 4–64 px (18); Roughness 0–100 (55).
- Shadow — colour (Ink), Offset X/Y spin (8, 10), Blur 0–64 px (20), Opacity 0–100 % (40).
- Perspective — Tilt −30…30° (6), Rotation −45…45° (−16), presets Left / Flat / Right.
- Page Curl — four-corner picker (↖ ↗ ↙ ↘), default bottom right.
- Reflection — Size (30 %), Opacity (25 %).
- Colour Adjustment — Brightness, Contrast, Saturation −100…100, Hue −180…180°, Gamma 0.1–3.0, Reset.
- Filter — Grayscale | Sepia | Invert | Sharpen | Soften (+ strength slider when Sharpen/Soften).
- Replace Colour — from / to chips, eyedropper, Tolerance 0–100 (20).
- Watermark — Image (file button) and/or Text, 3 × 3 position grid, Scale (30 %), Opacity (55 %), Overlay | Emboss | Tiled.

**Capture info.** Boxed list of check rows (44 px): Date and Time, Application Name, Window
Title, Operating System, Image Size, Custom Text; the subtitle shows the value from capture
metadata or "Not available for screen captures" (row disabled). Style Dark | Light | Accent,
Position (Bottom/Top), Date Format. Footer at 100 %: 52 px high, 28 px side padding, Inter 20/600
(rendered at 10 px at 50 % zoom); key labels at 60 % opacity; small "annota" mark right when
the remaining space allows.

**Quick effects.** 2-column cards (radius 9, padding 8): 52 px thumbnail, bold name, "Auto"
accent badge on the one applied after capture. Selected card = 2 px accent ring. Right-click
or long-press opens the context menu. Only one quick effect can be "Apply After Every Capture"
at a time; choosing another moves the check.

**Auto-apply banner.** Standard `.banner` at the top of the canvas column, text "Quick effect
"<name>" was applied after capture", button Remove (removes the stack in one undoable step).

## Open questions

1. Selection-scoped effects (`10`) need a pixel selection from the Selection tool (Q) of
   add-advanced-annotation-tools — confirm that dependency.
2. Capture-info footer adds height (66 px at 1600 px width). Should it ever overlay inside the
   image instead? Mockup assumes "adds a band".
3. Page curl corner picker uses arrow glyphs in the toggle group; a dedicated corner icon
   would need adding to the kit (not done — kit is read-only for this task).
