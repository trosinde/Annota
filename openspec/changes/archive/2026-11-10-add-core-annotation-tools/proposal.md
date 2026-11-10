# Add core annotation tools

**Roadmap phase:** MVP (milestone 0.1)

## Why
Annotation tools are the reason people use Annota. The MVP ships the tools needed for everyday documentation screenshots; advanced tools follow in Phase 2.

## What Changes
- Arrow, line, pen, shape (rectangle, ellipse, polygon), text, callout, step, highlighter
- Basic redaction: blur, pixelate, solid box
- Single-key tool shortcuts and tool properties sidebar

## Benchmark features covered
- Arrow (end styles, curves, width)
- Line
- Pen/freehand
- Shape incl. polygon
- Text
- Callout with multiple tails
- Step numbering (numbers/letters, restart, manual value)
- Highlighter
- Blur/pixelate/solid redaction

## Out of scope
- Magnify, spotlight, stamps, cut-out, fill, eraser, magic wand (Phase 2)
- Smart redaction (Phase 2)

## Capabilities

### New Capabilities
- `annotation-tools`

### Modified Capabilities
- none

## Impact
- New module `src/tools/`
