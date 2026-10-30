import pathlib, re

ROOT = pathlib.Path("/home/claude/annota")

DOCS = {
"add-text-recognition": dict(
title="Add text recognition — mockups",
intro="Grab text (whole image or selection), edit text in place, `text` capture mode, OCR preferences and the missing-Tesseract state. Sample content: `f-field-notes` (article), `a-harbor-backup` (typo fix), `e-relay-chat` (text capture), `d-tidewater-analytics` (no text found).",
rows=[
("Local recognition › Offline", "02, 04, 10", "Every OCR surface carries the line *“Recognised on this computer with Tesseract. Works offline.”* (info icon, dim). No network state exists."),
("Grab text › Copy selection", "01, 02, 03", "01: Recognise Text tool (`O`) shows dashed text areas; the selected paragraph has handles and a sidebar preview. `Ctrl+Shift+C` opens 02 (review dialog, line breaks kept). Copy closes it and shows the toast 03."),
("Grab text (whole image)", "04", "No selection + `Ctrl+Shift+C` = whole image. Loading state: spinner + determinate progress, Copy disabled."),
("Grab text (nothing found)", "05", "Empty state inside the dialog; Copy disabled; sidebar says *No text areas found*."),
("Edit text › Fix typo", "06, 07", "Double-click a recognised line → inline field over the line (06). Sidebar shows the recognised text, matched font, estimated size, sampled text and background colour. Enter applies (07): original covered with sampled background, new text object selected, toast with Undo."),
("Text capture mode › Direct", "08, 09", "`annota --capture text`: region overlay with hint pill (08). After Enter: GNOME notification *Text copied* with the first line as preview and actions Show Text / Open in Editor (09). No editor window opens."),
("Language configuration (requirement text)", "10", "Preferences › Text Recognition: follow system language, per-language checks, not-installed languages show the `apt` command."),
("Tesseract missing (package Recommends only)", "11", "Warning banner below the header bar with the exact command and Copy Command; sidebar status page with Try Again."),
("Narrow (≤ 700 px)", "12", "Grab-text dialog becomes a bottom sheet."),
],
redlines="""
- **Tool:** *Recognise Text*, icon `ocr-text`, single-key shortcut `O`, last slot in the tool strip after Crop.
- **Grab Text action:** `Ctrl+Shift+C` (menu label “Grab Text…”); works in every tool. With a selection → selection, else whole image.
- **Text areas:** fill `--accent-a12`, 2 px dashed (8/6) `--accent-bg` at 45 % opacity, radius 8 (image px). Selected area: fill `--accent-a25`, 3 px solid `--accent-bg`, four round handles. Toggle *Show text areas* (default on).
- **Grab Text dialog:** `AdwDialog`, 760 px wide (bottom sheet ≤ 700 px). Header: Cancel | “Grab Text” | Copy (suggested). Body: editable text view (`--fs-small`, line-height 1.65, view-bg, 1 px `--borders` inset), then row: language dropdown (200 px), check *Keep line breaks* (default on), counter “N lines · N words” (caption, dim). Footer note with `info` icon.
- **Toast:** “N lines copied to clipboard”, close button, no Undo.
- **Edit Text:** in-place field 2–3 px accent outline over the line; red wavy underline only for the spell-check hint. Sidebar group “Edit Text”: *Recognised* property row, Font dropdown (“Sans (matched)”), Size spin (estimated), *Text colour* and *Background* rows with small square swatch + hex (“Sampled · #7A7A7F”), buttons Revert + Done (suggested). Hint “Enter applies · Esc cancels”. Result is a normal text object (stays editable); toast “Text replaced” + Undo.
- **Text capture overlay:** standard region overlay (scrim 45 %), size label OSD under the selection, bottom pill: `ocr-text` icon, “Select text to copy”, separator, “Enter to confirm · Esc to cancel”.
- **Notification:** title “Text copied”, body “N lines from your selection are on the clipboard.” + first 50 characters in quotes, actions *Show Text* (opens the dialog standalone) and *Open in Editor*.
- **Preferences page “Text Recognition”:** groups Languages (switch *Follow system language*, checklist with `eng · installed`, not-installed rows dimmed with monospace `sudo apt install tesseract-ocr-<code>`) and Result (*Keep line breaks*, *Join words split by hyphens*, *Show text before copying* — off means `Ctrl+Shift+C` copies right away).
- **Missing Tesseract:** `AdwBanner` warning style: “Text recognition needs Tesseract. Install it with `sudo apt install tesseract-ocr`” + button *Copy Command*. Tool sidebar: compact status page “Text Recognition Unavailable” + *Try Again* (re-checks without restart).
"""),

"add-smart-redaction": dict(
title="Add smart redaction — mockups",
intro="Smart Redact lives in the Redact tool (`B`) sidebar. Sample: `e-relay-chat` (two e-mails, three phone numbers, three IP addresses, three dates, one booking reference as token); face demo uses a geometric avatar over the profile picture.",
rows=[
("Automatic detection (running)", "01", "Scan line + progress in sidebar; Cancel; note *Runs on this computer. Nothing is uploaded, and it works offline.*"),
("Automatic detection › Email found", "02", "Each hit becomes a redaction object outlined in Coral; the selected one shows its category tag (“E-mail”) and handles. Sidebar lists categories with counts and switches; expanded category lists items with checks."),
("Review and categories › Exclude dates", "03", "Dates switch off → date redactions are removed from the canvas, the badge turns neutral. Also shows a single item unchecked (token): dashed outline, pixels visible, count drops."),
("Review › choose style for all", "02/04/05", "Toggle group Solid / Pixelate / Blur applies to all smart redactions. Pixelate and Blur show the warning note."),
("Faces", "10", "Face category with one item; blur clipped to the face shape."),
("Irreversible export › Permanent", "06, 07", "Permanently Redact… → alert (06) with option to switch blur/pixelate to Solid. After saving (07): subtitle “Saved · Permanently redacted”, confirmation card, toast."),
("Irreversible export (always flatten)", "08", "Export dialog: *Flatten annotations* locked on (pin icon, disabled switch, explanation). Warning box when blur/pixelate is used, with *Use Solid for All*."),
("Nothing found (empty)", "09", "Compact status page, hint to draw manually with `B`, *Check Again*."),
],
redlines="""
- **Entry point:** Redact tool (`redact` icon, shortcut `B`). Smart detection runs automatically when the tool is first opened on an image; menu item “Find Sensitive Data Again”.
- **Detection outline:** 2.5 px `--annota-coral` rectangle, 3 image px outside the redaction. Category tag only on the selected object: Coral pill 19 px high, white 13 px bold label (“E-mail”, “Phone”, “IP address”, “Web address”, “Card”, “IBAN”, “Date”, “Token”, “Face”).
- **Solid:** `--annota-ink` fill, radius 2. **Pixelate:** 10 image px cells. **Blur:** σ ≈ 7 px; faces use the face outline (circle) as clip.
- **Sidebar order:** header “Smart Redact” + “N items found · checked on this computer”; caption “Style for all” + full-width toggle group; optional warning note (amber 18 % bg, `warning` icon: “Blur and pixelate can sometimes be undone by others. Use Solid for passwords, keys and personal data.”); category boxed list; full-width *Permanently Redact…* (destructive text on neutral button).
- **Category rows (fixed order):** E-mail addresses, Phone numbers, IP addresses, Web addresses, Payment cards, IBANs, Dates, Keys and tokens, Faces. Row 44 px: chevron (when expanded), title (`--fs-small`), count badge (accent when on, neutral when off), switch (85 %). Categories with 0 hits: dimmed, “None”, disabled switch. Item rows 36 px, monospace text, check on the right.
- **Alert:** heading “Permanently Redact?”, body names the count, check “Switch blur and pixelate to Solid” (default on), buttons Cancel / *Redact and Save* (destructive).
- **Export dialog:** warning box above the fields: “N redactions use Pixelate/Blur” + *Use Solid for All*. Flatten row: subtitle “Always on: the image contains redactions”, `pin` icon, disabled switch.
"""),

"add-screen-recording": dict(
title="Add screen recording — mockups",
intro="Recording UI floats over the GNOME desktop (1280×800 shown; same layout on 1920×1080). Captured content: `d-tidewater-analytics`. Webcam always shows a geometric avatar. The video editor uses the editor header-bar pattern with a timeline at the bottom.",
rows=[
("Start and stop (setup)", "01", "Region overlay + recording setup popover: Region/Window/Screen, Microphone and System audio with level meters, Webcam (summary row), Highlight cursor, Animate clicks, Countdown spin (0–10 s, default 3). Bottom pill switches Screenshot/Video."),
("Webcam overlay (shape, position, size)", "02", "Webcam row expanded: shape picker circle/rounded square/square, 3×3 position grid, size slider. Live preview inside the region."),
("Start and stop (countdown)", "03", "3-2-1 countdown disc centred in the region, pill “Recording starts in 3 s · Cancel”. Region frame turns red dashed."),
("Start and stop › Stop by hotkey", "04", "Floating control under the region: red dot, elapsed time, Pause, Stop (tooltip “Stop (Shift+Ctrl+Print)”), Draw, Mic (mute + meter), Swap, more. Shell top bar shows the red recording indicator with time. After stop the video editor (08) opens."),
("Cursor effects › Click ripple", "04", "Amber highlight ring around the cursor; Coral double ripple at the click."),
("Audio › Mute mic", "05", "Mic button pressed with slash, meter grey, tooltip “Microphone muted · M”. Also shows Paused state (pause glyph in amber, Play button, “Paused” OSD)."),
("Draw while recording › Arrow overlay", "06", "Draw toggled on → secondary OSD bar: arrow/rectangle/step, 3 colours (Coral default), fade dropdown (“Fade after 3 s” / “Keep until cleared”), Clear. A fading rectangle + step 1 shown at 50 % opacity."),
("Webcam overlay › Swap", "07", "Swap pressed: the region shows the full webcam, label “Webcam only”, button reads “Show Screen”."),
("Video editing (trim, combine)", "08", "Preview + timeline: video track with thumbnails for two combined clips, Mic and System waveform tracks, yellow trim handles with hatched trimmed parts, playhead. Sidebar: Trim times, Clips list (drag to reorder, Add Clip…)."),
("Video editing (cut range, frame grab)", "09", "Selected range in red “Cut 0:24.0 – 0:30.0” with the muted mic gap visible; *Cut Out Range* active; sidebar From/To + *Cut Out 6.0 s*. Toast after *Save Frame*."),
("Video editing and export › GIF", "10, 11", "Export Video dialog: MP4/WebM/GIF; GIF group with frame rate (15 fps), width (800 px, height follows), Loop forever; size estimate. 11 = progress state."),
("Export MP4/WebM", "12", "Quality dropdown (H.264 / VP9 for WebM) and Include audio."),
],
redlines="""
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
"""),

"add-step-capture": dict(
title="Add step capture — mockups",
intro="Step capture records clicks in another app (`a-harbor-backup`) and turns them into a guide edited in a step list window that follows the editor header-bar pattern. Export goes to the Step Guide template document, PDF or Markdown.",
rows=[
("Record steps (start)", "01", "Floating control: Coral counter “0 Steps”, Pause, Stop, Undo last step. Shell indicator. Hint pill until the first click."),
("Record steps › Three clicks", "02", "Each click flashes a numbered Coral marker at the click position (earlier ones fade); OSD label names the new step. Counter shows 3."),
("Record steps (Pause)", "03", "Counter label “Paused”, Play button pressed, pill “Paused · clicks are not recorded”."),
("Step descriptions › Button name", "04", 'Auto text from accessibility: `Click "Start backup"`, `Turn on "Back up on battery"`; magic-wand caption names role + container.'),
("Step descriptions (editable)", "05", "Inline entry for title + optional note; caption “Edited · was … · Restore”."),
("Guide output › edit list (reorder)", "06", "Dragged card lifts (shadow, slight tilt), 4 px accent drop line marks the target."),
("Guide output › edit list (delete)", "07", "Step removed, numbers renumber, toast with Undo."),
("Empty guide", "08", "Status page, Export disabled."),
("Guide output › Markdown", "09, 11", "Export Guide dialog with three choices; Markdown preview of the folder tree (.md + images folder). 11: toast with Open Folder."),
("Guide output › PDF", "10", "Progress state while writing."),
("Guide output › template document", "12", "Step Guide template opened in the editor shell: title, meta, numbered steps with cropped images and markers."),
],
redlines="""
- **Start:** main menu / capture popover item “Step Capture”; `annota --capture steps`. Stop with the control or `Shift+Ctrl+Print` (same global stop as recording).
- **Floating control:** `.osd-toolbar.pill`, bottom centre 24 px above the screen edge, never captured. Counter: 26 px Coral circle with white number + label “Step/Steps/Paused”. Buttons: Pause/Resume, Stop (destructive pill with label), Undo Last Step (`undo`).
- **Click marker:** Coral circle r = 22 image px, 4 px white ring, outer halo `--annota-coral-a20` (r + 14), white bold number. Shown on screen for 600 ms, always baked into the step image (toggle *Click markers* in the guide sidebar).
- **Step image:** active window, cropped “Around click” (default 1200 × 670 around the click; options Whole window, Around click).
- **Step card:** card, 12 px padding, grid: drag handle 24 · number 36 (32 px Coral disc) · thumbnail 240×134 (radius 6, 1 px border) · text · actions (Edit image in editor `pen`, Delete `trash`). Text: caption with `magic-wand` “From accessibility · <role> · <container>” or `text` “Edited · was … · Restore”; title `--fs-title-4` bold; note `--fs-small` dim (“Add a note…” placeholder). Selected card: 2 px accent ring.
- **Description rules:** button/menu item → `Click "<name>"`; switch → `Turn on/off "<name>"`; check → `Check "<name>"`; entry → `Click in "<name>"`; unknown → `Click here`.
- **Guide sidebar (300 px):** Title entry row, Template dropdown, Image crop dropdown, Click markers switch; Marker colour swatches (Coral default); *Add Steps…*; local note.
- **Export Guide dialog:** 580 px; radio rows *Step Guide document* (`template`), *PDF* (`image`), *Markdown* (`text`); Folder (`~/Documents/Guides`), Image width dropdown (800/1200/1600 px). Markdown writes `<slug>.md` + `<slug>-images/step-NN.png`.
"""),
}

for cid, d in DOCS.items():
    base = ROOT / "docs/design" / cid
    files = sorted(p.stem for p in (base / "src").glob("*.html"))
    lines = [f"# {d['title']}", "", d["intro"], "",
             "All screens: `src/NN-slug.html` → `png/NN-slug-{light,dark}@{1,2}x.png`. Render with "
             f"`python3 tools/render-mockups.py docs/design/{cid}`.", "",
             "## Scenario → mockup", "", "| Scenario / state | Mockup | Notes |", "|---|---|---|"]
    for scen, nums, note in d["rows"]:
        links = ", ".join(f"[{f}](png/{f}-light@1x.png)" for f in files if any(f.startswith(n.strip()) for n in nums.replace("/", ",").split(",")))
        lines.append(f"| {scen} | {links} | {note} |")
    lines += ["", "## Screens", ""] + [f"- [{f}](png/{f}-light@1x.png) · [dark](png/{f}-dark@1x.png)" for f in files]
    lines += ["", "## Redlines", d["redlines"].strip(), "",
              "Shared: editor shell = kit `template.html` (header 47 px, tool strip 52 px, properties 300 px), 6 px grid, Annota Coral `#ff5a4e` for annotation ink, Adwaita blue for UI accent. No accounts, cloud or network anywhere.", ""]
    (base / "index.md").write_text("\n".join(lines), encoding="utf-8")

    dm = ROOT / "openspec/changes" / cid / "design.md"
    txt = dm.read_text(encoding="utf-8")
    new = (f"## Mockups (design gate)\n\nMockups live in `docs/design/{cid}/` and were produced by the `designer` agent.\n"
           f"Scenario-to-mockup mapping and redlines: [`docs/design/{cid}/index.md`](../../../docs/design/{cid}/index.md).\n\n"
           + "\n".join(f"- [{f}](../../../docs/design/{cid}/png/{f}-light@1x.png)" for f in files)
           + "\n\nDark and 2× renders sit next to each file (`-dark@1x.png`, `@2x.png`).\n\nDesign approved: _pending_\n\n")
    txt = re.sub(r"## Mockups \(design gate\)\n.*?Design approved: _pending_\n\n?", new, txt, flags=re.S)
    dm.write_text(txt, encoding="utf-8")
print("ok")
