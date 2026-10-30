# Generator scripts (archive)

These Python scripts produced the first OpenSpec roadmap and the first mockup set
(October 2026). They are kept for reference and reuse.

**Source of truth is not here:** the HTML mockups in `docs/design/<change>/src/` and the
OpenSpec files in `openspec/` are what counts. Edit those directly; if you prefer to
regenerate a whole set, adapt the script first.

| Folder | Produced |
|---|---|
| `openspec-roadmap/` | the 18 changes in `openspec/changes/` (`oslib.py` helper, `mvp.py`, `later.py`, `library.py`) |
| `mockups/shell-styles-export/` | bootstrap-app-shell, add-style-system, add-export-share |
| `mockups/editor-core-tools/` | add-annotation-editor, add-core-annotation-tools |
| `mockups/capture-presets-scrolling/` | add-screen-capture, add-capture-presets, add-scrolling-capture |
| `mockups/advanced-effects-smart/` | add-advanced-annotation-tools, add-image-effects, add-smart-editing |
| `mockups/capture-ocr-redaction-recording-steps/` | add-text-recognition, add-smart-redaction, add-screen-recording, add-step-capture |
| (in repo) `docs/design/add-capture-library/tools/` | add-capture-library, add-library-organisation, add-templates |
| `review-gallery/template.html` | template for `tools/build-review-gallery.py` |

Known limitation: the scripts were written in a temporary workspace and contain absolute
paths like `/tmp/.../scratchpad/...` and `/home/claude/annota/...`. Replace them with the
repository root before running.
