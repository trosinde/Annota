# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-core-annotation-tools/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Tools
Phase A (2026-10-31): the object types, drawing them on the canvas, their handles and the tool
strip with single-key shortcuts; each type's `properties`/`set_property` is complete (the
contract task 3.1 builds the sidebar on). See design.md, "Decided during apply — phase A".
- [x] 2.1 Arrow + line (`src/tools/arrow.rs`: end styles, sizes, line styles, quadratic curve
      handle, Shift 0/45/90°)
- [x] 2.2 Pen (`src/tools/pen.rs`: vector path re-rendered at any width, smoothing 0–100,
      Shift straight segments)
- [x] 2.3 Shapes incl. polygon (`src/tools/shape.rs`, polygon drafting in
      `src/editing/tool.rs`; the shape-type switch to reach polygons is part of task 3.1 — until
      then only through the tool defaults' `set_property("shape", "polygon")`)
- [x] 2.4 Text (`src/tools/text.rs`, in-place editing `src/editing/text_edit.rs`; Pango text
      layout, new crates `pango`/`pangocairo` 0.20, MIT)
- [x] 2.5 Callout with multi-tail (`src/tools/callout.rs`)
- [x] 2.6 Step markers (`src/tools/step.rs`, sequences and renumbering in
      `src/editing/steps.rs`; Restart Sequence and Next value exist on the canvas
      (`Canvas::restart_sequence`, `set_next_step`) and get their buttons with task 3.1)
- [x] 2.7 Highlighter (`src/tools/highlighter.rs`, multiply blending)
- [x] 2.8 Blur/pixelate/solid (`src/tools/redaction.rs`, `src/render/redact.rs`). The export
      warning ("Redaction export warning", mockup 19, D7) guards export, copy, drag and share,
      none of which exist yet: it moved to add-export-share task 2.7, which builds those paths.

## 3. Properties
- [ ] 3.1 Property sidebar per tool
- [ ] 3.2 Colour picker with eyedropper (portal PickColor)
- [ ] 3.3 Shadow editor

## 4. Tests
- [ ] 4.1 Golden images for every tool and property combination (phase A added one golden per
      type with its defaults and key variants — `tests/render/main.rs`: arrow-objects,
      pen-objects, shape-objects, text-objects, callout-objects, step-objects,
      highlighter-objects, redaction-objects — as CLAUDE.md §7 asks of every new type; the full
      property matrix comes with phase B)

## 5. Verify
- [ ] 5.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 5.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 5.4 German translation updated
