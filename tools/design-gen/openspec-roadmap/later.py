from oslib import change, R, S

P2 = "Phase 2 (milestone 0.2–0.4)"
P3 = "Phase 3 (milestone 0.5+)"
VERIFY_NOTE = "Depends on the MVP changes being archived first."

# ---------------------------------------------------------------- advanced tools
change(
    "add-advanced-annotation-tools", P2, "Add advanced annotation tools",
    "Beyond arrows and boxes, good documentation screenshots zoom into details, dim distractions and remove clutter.",
    ["Magnifier with offset lens", "Spotlight (dim and/or blur outside a shape)", "Stamps with original Annota stamp sets and custom stamps",
     "Cut-out (remove a horizontal/vertical strip and join)", "Selection tool with move/delete and auto-fill",
     "Fill (flood fill) with tolerance", "Eraser", "Magic wand selection"],
    ["Magnify", "Spotlight", "Stamps incl. custom stamps", "Cut out with edge styles", "Selection (rect/ellipse/polygon/lasso) with auto-fill",
     "Fill with tolerance and global fill", "Eraser", "Magic wand", "Text-only highlight/blur"],
    ["Smart Move / Simplify (Phase 3)"],
    [
        ("annotation-tools", None, [
            R("Magnifier",
              "The magnifier SHALL enlarge a source area by 125–800 % into a lens (circle or rounded rectangle) with outline width/colour. The lens SHALL be movable away from its source, drawing an optional connector line.",
              S("Offset lens", "the user drags the lens away from the source area", "the lens shows the source area enlarged at its new position with a connector")),
            R("Spotlight",
              "The spotlight SHALL keep one or more shapes (rectangle, rounded, ellipse, polygon) clear and dim (0–90 %) and/or blur (0–30 px) everything else.",
              S("Two spots", "the user draws two spotlight shapes", "both areas are clear and the rest is dimmed")),
            R("Stamps",
              "The stamp tool SHALL offer searchable categories of original Annota SVG stamps (status marks, numbers, UI pointers, emoji-style faces, keyboard keys) and user stamps imported from PNG/SVG or saved from the current selection.",
              S("Custom stamp", "the user imports company-logo.png as a stamp", "it appears under My Stamps and can be placed")),
            R("Cut-out",
              "The cut-out tool SHALL remove a horizontal or vertical strip and join the remaining parts with an edge style (straight, zigzag, wave, torn).",
              S("Remove middle", "the user drags a horizontal strip and confirms", "the image becomes shorter by the strip height with a zigzag seam")),
            R("Selection with auto-fill",
              "The selection tool SHALL select pixels by rectangle, ellipse, polygon or lasso; moving or deleting the selection SHALL fill the hole with transparency or an auto-fill derived from surrounding pixels.",
              S("Auto-fill", "the user deletes a selected button on a flat background with auto-fill", "the hole is filled with the background colour")),
            R("Fill",
              "The fill tool SHALL flood-fill with tolerance 0–100; `Shift`-click SHALL replace that colour globally.",
              S("Global replace", "the user Shift-clicks a blue pixel with red", "all matching blue pixels become red")),
            R("Eraser and magic wand",
              "The eraser SHALL erase image pixels to the canvas colour with a round or square brush; the magic wand SHALL select contiguous similar pixels with tolerance.",
              S("Wand", "the user clicks a uniform area with the magic wand", "that area is selected")),
            R("Text-only mode",
              "Highlighter, blur and spotlight SHALL offer `Apply to text only`, using text detection to restrict the effect to text regions.",
              S("Highlight text only", "text-only highlight is drawn over a paragraph with icons", "only the text lines are highlighted")),
        ]),
    ],
    ["Text detection for text-only mode reuses the text-recognition engine", "Stamp sets are designer-made, CC-BY-SA-4.0"],
    [("Tools", ["Magnifier", "Spotlight", "Stamps + stamp manager", "Cut-out", "Selection + auto-fill", "Fill", "Eraser", "Magic wand", "Text-only mode"]),
     ("Assets", ["Designer creates 6 original stamp sets"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- effects
change(
    "add-image-effects", P2, "Add image effects",
    "Borders, edges and shadows make screenshots fit polished documentation; capture info adds traceability.",
    ["Effects panel: border, edges, shadow, perspective, page curl, reflection, colour adjustment, filters, watermark, capture info",
     "Effects apply to the whole image or a selection and stay editable", "Quick effects (saved effect combinations)"],
    ["Border", "Edges (torn, wave, zigzag, fade)", "Shadow", "Perspective", "Page curl", "Reflection",
     "Colour adjustment (brightness, contrast, hue, saturation, gamma)", "Filters (grayscale, sepia, invert, sharpen, soften)",
     "Colour replacement", "Watermark", "Capture info (date, app, OS, custom text)"],
    ["Animated backgrounds"],
    [
        ("image-effects", "Applies non-destructive visual effects to a capture or a selected part of it.", [
            R("Non-destructive effects",
              "Effects SHALL be stored as an ordered, editable effect stack on the document (or on a selection) and rendered at export; each effect can be toggled and edited later.",
              S("Edit later", "the user reopens a saved document with a border effect", "the border width can still be changed")),
            R("Effect catalogue",
              "The system SHALL provide: border (colour, width, radius), edges (torn, wave, zigzag, fade; sides; size), shadow (colour, offset, blur, opacity), perspective (tilt, rotation), page curl (corner), reflection (size, opacity), colour adjustment (brightness, contrast, hue, saturation, gamma), filters (grayscale, sepia, invert, sharpen, soften), colour replacement (from, to, tolerance) and watermark (image, position, scale, opacity, overlay/emboss).",
              S("Torn bottom edge", "the user applies edges=torn to the bottom side only", "only the bottom edge is torn")),
            R("Capture info",
              "The capture-info effect SHALL add a footer with any of: date/time, application name, window title, OS name/version, image size, custom text; fields come from capture metadata.",
              S("Footer", "capture info with date and application is applied to a window capture", "a footer shows the capture time and the application name")),
            R("Quick effects",
              "Users SHALL save the current effect stack as a named quick effect and apply it with one click or automatically after every capture.",
              S("Auto-apply", "a quick effect is set to apply after capture", "every new capture opens with that effect stack")),
        ]),
    ],
    ["Effects are pure functions on cairo surfaces; GPU not required"],
    [("Implementation", ["Effect stack model + serialization", "Each effect + properties UI", "Capture-info metadata collection", "Quick effects"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- capture extras
change(
    "add-capture-presets", P2, "Add capture presets and advanced capture modes",
    "Power users repeat the same captures; presets with their own hotkeys save time.",
    ["Capture presets (mode, delay, cursor, effects, output) with own hotkeys", "Multi-area capture", "Time-lapse (interval) capture",
     "Capture info metadata (app, window title, URL where available)"],
    ["Custom capture presets", "Custom keyboard shortcuts per preset", "Multiple areas", "Time-lapse capture"],
    ["Webcam still capture"],
    [
        ("capture", None, [
            R("Capture presets",
              "Users SHALL define presets combining mode, fixed size, delay, cursor inclusion, quick effect and output action (editor, clipboard, file); each preset MAY have its own global shortcut.",
              S("Preset hotkey", "a preset `800x600 to clipboard` has Ctrl+Print", "pressing Ctrl+Print captures 800×600 and copies it without opening the editor")),
            R("Multi-area capture",
              "In region mode, holding `Ctrl` while selecting SHALL add further areas; the result is one image with the areas arranged in their screen positions on a transparent canvas.",
              S("Two areas", "the user selects two separate areas with Ctrl", "one image contains both areas")),
            R("Time-lapse capture",
              "Users SHALL capture the same region every N seconds (1–3600) until stopped, saving into a folder.",
              S("Interval", "time-lapse with 10 s is started for 1 minute", "6 images are saved")),
        ]),
    ],
    ["Presets stored in GSettings as JSON"],
    [("Implementation", ["Preset model + editor", "Per-preset shortcuts", "Multi-area selection", "Time-lapse runner"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- OCR
change(
    "add-text-recognition", P2, "Add text recognition (grab and edit text)",
    "Users often need the text in a screenshot, or need to fix a typo in it without recapturing.",
    ["Grab text from a whole capture or a selection to the clipboard", "Edit text in place: replace recognised text with matching font size and colour",
     "Text capture mode directly from the screen"],
    ["Grab Text", "Edit Text", "Text capture mode"],
    ["Cloud OCR (never)"],
    [
        ("text-recognition", "Recognises text in captures locally and lets users copy or replace it.", [
            R("Local recognition",
              "Text recognition SHALL run locally with Tesseract (languages: system locale + English, configurable) and SHALL never send image data over the network.",
              S("Offline", "the computer is offline and the user grabs text", "recognition succeeds")),
            R("Grab text",
              "Users SHALL copy recognised text of the whole image or a selection (`Ctrl+Shift+C`), preserving line breaks; a dialog shows the text for review before copying.",
              S("Copy selection", "the user selects a paragraph and presses Ctrl+Shift+C", "the paragraph text is shown and copied")),
            R("Edit text",
              "Users SHALL double-click recognised text to edit it; the original text is covered with the sampled background colour and replaced with a text object matching estimated size and colour.",
              S("Fix typo", "the user edits `Recieve` to `Receive`", "the image shows `Receive` in a similar font size and colour")),
            R("Text capture mode",
              "A capture mode `text` SHALL recognise the selected region and copy the text without opening the editor.",
              S("Direct", "the user runs `annota --capture text` and selects a region", "the text is on the clipboard and a notification confirms it")),
        ]),
    ],
    ["`leptess` bindings or tesseract CLI; package recommends tesseract-ocr-eng/deu"],
    [("Implementation", ["OCR service + language settings", "Grab text dialog", "Edit text flow", "Text capture mode"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- smart redaction
change(
    "add-smart-redaction", P2, "Add smart redaction",
    "Screenshots leak e-mail addresses, IPs and tokens. Automatic detection reduces the risk of sharing sensitive data.",
    ["Detect e-mail, phone, IP, URL, IBAN/credit card patterns, dates, API-key-like strings and faces", "Review list with per-category toggles",
     "Safe export: redactions are always flattened; blur/pixelate warning"],
    ["Smart Redact", "Solid redaction", "Flatten before share"],
    ["Cloud-based detection"],
    [
        ("privacy-redaction", "Finds and hides sensitive information in captures locally and guarantees that hidden data cannot be recovered from exports.", [
            R("Automatic detection",
              "Smart redaction SHALL detect e-mail addresses, phone numbers, IPv4/IPv6 addresses, URLs, credit-card numbers (Luhn-checked), IBANs, dates, API-key-like tokens and faces, locally.",
              S("Email found", "a capture shows `jane@example.com`", "a redaction object covers it, tagged as category email")),
            R("Review and categories",
              "Detected items SHALL be listed by category with toggles per category and per item; the user chooses blur, pixelate or solid for all.",
              S("Exclude dates", "the user disables category dates", "date redactions are removed from the canvas")),
            R("Irreversible export",
              "Exports SHALL always flatten redactions; `.annota` files SHALL store the redacted image only, discarding the original pixels under solid redactions when the user selects `Permanently redact`.",
              S("Permanent", "the user chooses Permanently redact and saves", "the saved original.png no longer contains the hidden pixels")),
        ]),
    ],
    ["Pattern detection on OCR output; faces with a bundled small ONNX/OpenCV model (license checked)"],
    [("Implementation", ["Detectors per category", "Review panel", "Permanent redaction in document format"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- library
change(
    "add-capture-library", P2, "Add capture library",
    "After weeks of use, users need to find old captures quickly.",
    ["Library view of all captures with grid, search, tags, filters (date, application, type)", "Metadata stored per capture",
     "Configurable retention"],
    ["Library", "Search", "Filter and sort by date/app", "Tags"],
    ["Cloud sync"],
    [
        ("capture-library", "Keeps every capture findable through a searchable, taggable library.", [
            R("Library view",
              "The library SHALL show all captures as a thumbnail grid grouped by day, with sort by date, name and size.",
              S("Open from library", "the user double-clicks a thumbnail", "the capture opens in the editor")),
            R("Search and filter",
              "Users SHALL search by file name, tags, application name, window title and recognised text, and filter by date range, application and type (image/video).",
              S("Search by app", "the user searches `Firefox`", "captures taken from Firefox windows are shown")),
            R("Tags",
              "Users SHALL add and remove free-text tags on one or many captures.",
              S("Bulk tag", "the user tags 5 selected captures `release-notes`", "all 5 show the tag")),
            R("Retention",
              "Users SHALL configure automatic deletion of untagged captures older than N days (off by default); deleted items go to the system trash.",
              S("Cleanup", "retention is 30 days", "untagged captures older than 30 days are moved to trash on startup")),
        ]),
    ],
    ["SQLite index (`rusqlite`) next to files in `~/Pictures/Annota`; files remain the source of truth"],
    [("Implementation", ["Index + metadata extraction", "Library window", "Search/filter", "Tags", "Retention"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- templates
change(
    "add-templates", P2, "Add templates and image combining",
    "Step-by-step guides and comparisons need several captures on one page with consistent layout.",
    ["Original Annota templates: step guide, side-by-side comparison, before/after, grid, timeline", "Combine selected captures into a template",
     "Placeholders, titles, captions, numbering, theme colours", "Save own layouts as templates"],
    ["Templates", "Combine images", "Numbered captions", "Custom templates"],
    ["Premium template store"],
    [
        ("templates", "Arranges multiple captures into consistent, reusable page layouts.", [
            R("Built-in templates",
              "The system SHALL ship original templates: step guide (2–8 steps), comparison (2–3 columns), before/after, grid (2×2, 3×2) and timeline.",
              S("New from template", "the user chooses New → From template → Step guide", "an empty step guide with placeholders opens")),
            R("Combine captures",
              "Users SHALL select captures in the tray and choose Combine; a dialog lets them pick a template, order images, enter title and captions and toggle numbering.",
              S("Combine three", "three captures are combined into a step guide", "each is placed in a numbered slot with its caption")),
            R("Placeholders",
              "Template slots SHALL accept images by drag and drop and scale them by fit or fill.",
              S("Drop into slot", "the user drops a capture onto an empty slot", "the slot shows the image fitted")),
            R("Custom templates",
              "Users SHALL save any document as a template with marked slots; it then appears in the template chooser.",
              S("Reuse", "the user saves a layout as template", "it is offered under My templates")),
        ]),
    ],
    ["Templates are `.annota` documents with slot objects and theme tokens"],
    [("Implementation", ["Slot object type", "Template chooser", "Combine dialog", "Built-in templates (designer)", "Save as template"])],
    [VERIFY_NOTE],
)

# ---------------------------------------------------------------- recording
change(
    "add-screen-recording", P2, "Add screen recording",
    "Short videos and GIFs explain flows better than stills.",
    ["Record region, window or screen via ScreenCast portal + PipeWire", "Microphone and system audio, webcam picture-in-picture",
     "Cursor highlight, click animation, on-screen drawing while recording", "Trim, cut, combine clips, export MP4/WebM/GIF, grab frame"],
    ["Video capture", "Audio (mic/system)", "Webcam PiP incl. shape", "Cursor highlight", "Animate clicks", "Draw on screen while recording",
     "Trim/cut", "Combine clips", "GIF export", "Capture frame"],
    ["Webcam background blur (later)", "Multi-track editing"],
    [
        ("screen-recording", "Records the screen with optional audio and webcam and provides basic video editing and export.", [
            R("Start and stop",
              "Recording SHALL start after a 3 s countdown (configurable 0–10) for region, window or screen; a floating control shows elapsed time, pause, stop and draw. `Shift+Ctrl+Print` stops recording.",
              S("Stop by hotkey", "a recording runs and the user presses Shift+Ctrl+Print", "recording stops and the video opens in the video editor")),
            R("Audio",
              "Users SHALL record microphone and/or system audio with level meters and mute toggles during recording.",
              S("Mute mic", "the user mutes the mic during recording", "the mic track is silent for that segment")),
            R("Webcam overlay",
              "Users SHALL add a webcam overlay (circle, rounded square, square) with position, size, and switch between screen and full webcam during recording.",
              S("Swap", "the user clicks Swap during recording", "the recording shows the full webcam until swapped back")),
            R("Cursor effects",
              "Recordings SHALL optionally show a cursor highlight ring and a click ripple.",
              S("Click ripple", "click animation is on and the user clicks", "the video shows a ripple at the click position")),
            R("Draw while recording",
              "Users SHALL draw arrows, rectangles and step numbers on screen during recording; drawings fade after a configurable time or stay until cleared.",
              S("Arrow overlay", "the user draws an arrow during recording", "the arrow is visible in the video")),
            R("Video editing and export",
              "The video editor SHALL trim start/end, cut out a range, combine clips, save a frame as image, and export MP4 (H.264), WebM (VP9) and GIF (fps, width, loop).",
              S("GIF", "a 10 s clip is exported as GIF at 15 fps and 800 px", "a looping GIF of that size is written")),
        ]),
    ],
    ["GStreamer pipeline: pipewiresrc → videoconvert → x264enc/vp9enc; overlay compositing for cursor/drawings/webcam",
     "GIF via gifski", "Wayland: portal provides cursor metadata for highlight"],
    [("Recording", ["ScreenCast portal + PipeWire", "Countdown + floating control", "Audio", "Webcam", "Cursor effects", "Screen draw"]),
     ("Editing", ["Timeline trim/cut", "Combine", "Frame grab", "Exporters"])],
    [VERIFY_NOTE, "New dependencies: gstreamer-rs, gifski"],
)

# ---------------------------------------------------------------- phase 3
change(
    "add-scrolling-capture", P3, "Add scrolling capture",
    "Long web pages, chats and tables do not fit on one screen.",
    ["Scroll a window or region automatically or manually and stitch frames into one image"],
    ["Scrolling capture (vertical/horizontal)"],
    ["Browser-extension based full-page capture"],
    [
        ("scrolling-capture", "Captures content larger than the screen by scrolling and stitching.", [
            R("Manual scrolling capture",
              "After the user selects a region and scrolls manually, the system SHALL capture frames via the ScreenCast portal and SHALL stitch overlapping frames by image matching until the user presses Enter.",
              S("Long page", "the user scrolls through a 5-screen page and presses Enter", "one image contains the full page without duplicate or missing rows")),
            R("Automatic scrolling",
              "Where input injection is allowed (RemoteDesktop portal), the system SHALL scroll automatically and stop at the end of content or at a 20 000 px limit.",
              S("Auto stop", "auto scrolling reaches the end of a page", "capture stops and the stitched image opens")),
            R("Sticky header handling",
              "Fixed headers/footers SHALL be detected and kept only once in the result.",
              S("Sticky header", "a page with a fixed header is captured", "the header appears only at the top")),
        ]),
    ],
    ["Stitching by row-hash matching with fallback to normalized cross-correlation"],
    [("Implementation", ["Frame grabbing", "Stitcher", "Auto-scroll via RemoteDesktop portal", "Sticky header detection"])],
    ["Depends on add-screen-recording (ScreenCast pipeline)"],
)

change(
    "add-step-capture", P3, "Add step capture (automatic how-to guides)",
    "Writing click-by-click instructions is tedious; recording them automatically produces guides in minutes.",
    ["Record a sequence of clicks; each click produces a cropped capture with a click marker and an auto-generated step description",
     "Edit, reorder, delete steps; export as step-guide template, PDF or Markdown"],
    ["Step capture", "Export step guide"],
    ["Narrated video from steps (later)"],
    [
        ("step-capture", "Turns a recorded click sequence into an editable step-by-step guide.", [
            R("Record steps",
              "During step capture, each mouse click SHALL produce a capture of the active window with a step marker at the click position; `Pause` and `Stop` are available in a floating control.",
              S("Three clicks", "the user clicks three times during step capture", "three numbered steps are created")),
            R("Step descriptions",
              "Each step SHALL get a default description from accessibility information of the clicked element where available (e.g. `Click \"Save\"`), editable by the user.",
              S("Button name", "the user clicks a button labelled Save", "the step text reads Click \"Save\"")),
            R("Guide output",
              "Steps SHALL be editable in a step list and exportable as step-guide template document, PDF and Markdown with images.",
              S("Markdown", "the user exports as Markdown", "a .md file and an images folder are written")),
        ]),
    ],
    ["Click events via AT-SPI on GNOME (accessibility bus); Wayland limitations documented"],
    [("Implementation", ["Click listener via AT-SPI", "Step model + list editor", "Exporters"])],
    ["Depends on add-templates"],
)

change(
    "add-smart-editing", P3, "Add smart move and simplify",
    "Changing a screenshot of a UI (move a button, anonymise layout) normally needs a recapture.",
    ["Smart move: detect UI elements and text blocks and make them movable/deletable with background fill",
     "Simplify: replace UI details with abstract shapes for clean diagrams"],
    ["Smart Move", "Simplify"],
    ["AI image generation"],
    [
        ("smart-editing", "Treats flat screenshots as editable UI layouts.", [
            R("Smart move",
              "Activating smart move SHALL detect UI elements and text blocks (edge/contour analysis locally) and convert them into movable objects; vacated areas are filled from surrounding background. A detail slider controls granularity.",
              S("Move button", "smart move is on and the user drags a detected button", "the button moves and its old place shows background")),
            R("Simplify",
              "Simplify SHALL replace text lines with rounded bars and UI elements with flat shapes in theme colours; a detail slider and custom palettes are available; results remain editable objects.",
              S("Simplify screenshot", "the user applies simplify to a settings dialog", "text is replaced by bars and the layout is preserved")),
        ]),
    ],
    ["OpenCV (`opencv` crate) for contour detection; heavy work in a worker thread"],
    [("Implementation", ["Element detection", "Background fill", "Smart move interaction", "Simplify renderer"])],
    ["Depends on add-text-recognition and add-advanced-annotation-tools"],
)
print("later ok")
