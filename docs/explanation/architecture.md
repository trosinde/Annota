# Architecture

Why Annota is built the way it is. The binding list of technical decisions is
[`CLAUDE.md`](../../CLAUDE.md) §2; decisions with a history have a record in [`adr/`](../adr/).

## At a glance

| | |
|---|---|
| Platform | Ubuntu 24.04+, Debian 13 · GNOME, Wayland first, X11 supported |
| Stack | Rust · GTK 4 · libadwaita · Cairo · GStreamer · xdg-desktop-portal |
| App ID | `io.github.trosinde.Annota` |
| Privacy | no telemetry, OCR and redaction run locally |

## Key ideas

- **Screen access only through xdg-desktop-portal.** No X11-only hacks, no root, no
  compositor-private APIs – the same code works on Wayland and X11 and respects the desktop's
  permission model.
- **One renderer for canvas and export.** The canvas and every export use the same cairo
  renderer, so what you see is pixel-identical to what you share.
- **Annotations are objects, not pixels.** A document is a versioned scene graph; `.annota` is a
  ZIP with `original.png`, `document.json` ([schema](../../data/schema/document-1.schema.json)) and
  a thumbnail. Undo/redo runs through commands, never direct mutation.
- **Local only.** No accounts, no sign-in, no cloud, no update check. Annota never opens a network
  connection itself.
- **Native Debian package first**, built offline from vendored crates; Flatpak is secondary.

## Code layout

`src/capture/` portal access and region overlay · `src/document/` scene graph and `.annota` ·
`src/render/` shared cairo renderer · `src/tools/` one module per annotation type ·
`src/styles/` quick styles and themes · `src/export/` PNG/JPG/WebP/PDF, clipboard, drag and drop ·
`src/ui/` windows and panels (Blueprint files in `data/ui/`). Details: `CLAUDE.md` §3.
