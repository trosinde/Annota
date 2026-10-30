# Annota

**Capture it. Mark it up. Make it obvious.**

Annota is a native GNOME screenshot, annotation and screen-recording app for Ubuntu and Debian.
Arrows, callouts, numbered steps, highlights and redaction stay editable until you export.
Built in Rust with GTK 4 and libadwaita, shipped as a native Debian package, private by default.

> Status: specified, pre-alpha. No code yet — the roadmap is fully specified in OpenSpec.

| | |
|---|---|
| Platform | Ubuntu 24.04+, Debian 13 · GNOME, Wayland first, X11 supported |
| Stack | Rust · GTK 4 · libadwaita · Cairo · GStreamer · xdg-desktop-portal |
| App ID | `io.github.trosinde.Annota` |
| Privacy | no telemetry, OCR and redaction run locally |

## Documentation

- **Visual project overview:** [`docs/index.html`](docs/index.html) (GitHub Pages ready)
- Feature catalogue (benchmark → phase): [`docs/feature-catalog.md`](docs/feature-catalog.md)
- Roadmap as OpenSpec changes: [`openspec/changes/`](openspec/changes)
- Rules for contributors and Claude Code: [`CLAUDE.md`](CLAUDE.md)

## Roadmap

| Phase | Changes |
|---|---|
| **MVP 0.1** | app shell & packaging · screen capture · editor & `.annota` format · core tools · quick styles & themes · export & share |
| **Phase 2** | advanced tools · effects · capture presets · text recognition · smart redaction · library · templates · screen recording |
| **Phase 3** | scrolling capture · step capture · smart move & simplify |

## How it is built

Design → Spec → Code → Verify → Package. Every user-visible change gets high-fidelity mockups from the
`designer` agent and maintainer approval before any code is written.

```bash
git clone https://github.com/trosinde/Annota.git && cd Annota && claude
# in Claude Code:
/design bootstrap-app-shell
/opsx:apply bootstrap-app-shell
```

## License

GPL-3.0-or-later for code, CC-BY-SA-4.0 for artwork and docs (to be confirmed before the first release).
Annota is an independent project and is not affiliated with any commercial screenshot tool.
