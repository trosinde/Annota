# Annota — Project Guide for Claude Code

Annota is a native GNOME screenshot, annotation and screen-recording app for Ubuntu and Debian.
Its functional scope is benchmarked against the feature set of commercial capture tools such as
TechSmith Snagit, but Annota is an **independent, original product**: its own UI, icons, stamps,
wording and visual identity. Never copy another product's UI layout, artwork, icons, stamp sets,
templates or text.

Read this file completely before doing any work. It defines the stack, the repository layout,
how the project is bootstrapped, and the mandatory workflow **Design → Spec → Code → Package**.

---

## 1. Product in one paragraph

Press a hotkey, select a region (or a window, or the whole screen), and the capture opens in the
Annota editor. Mark it up with arrows, callouts, numbered steps, shapes, highlighter, magnifier,
spotlight, stamps and redaction. Every annotation stays an editable object until export. Save as
`.annota` (editable) or export to PNG/JPG/WebP/PDF, copy to clipboard, or drag into another app.
Every capture lands automatically in the capture library, where it can be browsed, previewed,
renamed, dragged out and later searched, tagged and grouped into collections.
Later phases add screen recording, scrolling capture, step capture, OCR, smart redaction and templates.

Feature catalogue and phasing: `docs/feature-catalog.md`. Behaviour contracts: `openspec/`.

---

## 2. Fixed technical decisions

Do not change these without an approved OpenSpec change that says so (record it in `docs/adr/`).

| Area | Decision |
|---|---|
| Language | Rust, edition 2021, **MSRV 1.85** (Ubuntu 24.04 package `rustc-1.85`, Debian 13 default). Keep `Cargo.lock` compatible with 1.85. |
| UI toolkit | GTK 4 (`gtk4` crate) + libadwaita (`libadwaita` crate), GNOME HIG |
| Minimum platform | Ubuntu 24.04 LTS (GNOME 46, GTK 4.14, libadwaita 1.5); also Ubuntu 26.04 LTS and Debian 13 |
| Display server | Wayland first, X11 must keep working |
| Screen access | Only via **xdg-desktop-portal** (`ashpd` crate): Screenshot portal for stills, ScreenCast portal + PipeWire for video. No X11-only hacks, no root, no compositor-private APIs. |
| Global hotkey | GlobalShortcuts portal when available (GNOME ≥ 48); fallback: register a GNOME custom keybinding that runs `annota --capture <mode>` |
| Rendering | One renderer on **cairo-rs** used for the canvas *and* for export → pixel-identical WYSIWYG |
| Document format | `.annota` = ZIP: `original.png` + `document.json` (versioned scene graph, schema in `data/schema/`) + `thumbnail.png` |
| Video | GStreamer (`gstreamer-rs`), output MP4 (H.264) / WebM (VP9); GIF via `gifski` |
| OCR (phase 2) | Tesseract (`tesseract-ocr` package, eng + deu), runs locally, no cloud |
| Build | Meson + Cargo (GNOME app template layout) |
| Packaging | Native Debian package (`debian/`, debhelper-compat 13, vendored crates for offline build). Flatpak manifest is secondary. |
| App ID | `io.github.trosinde.Annota` |
| i18n | gettext; ship `en` and `de` from day one |
| License | MIT (code), CC-BY-SA-4.0 (artwork, docs) — confirmed by the maintainer 2026-10-30 |
| Local only | No accounts, no sign-in, no cloud, no telemetry, no update check. Annota never opens a network connection itself; the library may live on the user's own server via a mounted share. |

---

## 3. Repository layout (target)

```
annota/
├── CLAUDE.md                  ← this file
├── README.md
├── Cargo.toml / Cargo.lock
├── meson.build / meson_options.txt
├── src/
│   ├── main.rs                ← CLI parsing (`annota --capture region|window|screen --delay N`)
│   ├── application.rs         ← AdwApplication, actions, single instance via D-Bus
│   ├── capture/               ← portal access, region overlay, delay, window/screen modes
│   ├── document/              ← scene graph model, .annota load/save, undo/redo (command pattern)
│   ├── render/                ← cairo renderer shared by canvas and export
│   ├── tools/                 ← one module per annotation tool (arrow.rs, callout.rs, step.rs …)
│   ├── styles/                ← quick styles, themes (palette + fonts), tool presets
│   ├── export/                ← PNG/JPG/WebP/PDF, clipboard, drag-and-drop
│   ├── library/               ← automatic storage, index (SQLite), folder watcher, library window
│   ├── ui/                    ← windows, panels, widgets (Blueprint .blp files in data/ui/)
│   └── i18n.rs
├── data/
│   ├── ui/*.blp               ← Blueprint UI definitions
│   ├── icons/                 ← app icon (scalable + symbolic), tool icons (symbolic SVG)
│   ├── stamps/                ← original stamp sets (SVG), CC-BY-SA-4.0
│   ├── schema/                ← JSON schema of document.json
│   ├── io.github.trosinde.Annota.desktop.in
│   ├── io.github.trosinde.Annota.metainfo.xml.in
│   └── io.github.trosinde.Annota.gschema.xml
├── po/                        ← translations
├── tests/
│   ├── render/golden/*.png    ← golden images for renderer tests
│   └── *.rs
├── debian/                    ← Debian packaging
├── build-aux/
│   ├── vendor.sh              ← vendors crates into vendor/ (+ .cargo/config.toml)
│   ├── make-deb.sh            ← vendor + dpkg-buildpackage + lintian → ../annota_*.deb
│   ├── update-copyright.py    ← refreshes crate licenses in debian/copyright
│   └── flatpak/               ← Flatpak manifest (secondary, later)
├── tools/
│   ├── render-mockups.py      ← renders design mockups to PNG (Playwright)
│   ├── build-review-gallery.py← one-file gallery of all mockups for design review
│   └── design-gen/            ← archived generator scripts (roadmap, mockups)
├── docs/
│   ├── index.html             ← visual project documentation (GitHub Pages)
│   ├── feature-catalog.md     ← benchmark feature → capability → phase
│   ├── adr/                   ← architecture decision records
│   └── design/
│       ├── kit/               ← Annota design kit (tokens, Adwaita-faithful CSS, icons)
│       └── <change-id>/       ← mockups per OpenSpec change (HTML source + PNG renders)
├── openspec/                  ← specs and changes (see §6)
└── .claude/                   ← agents/, commands/ (opsx/*, design), skills/
```

---

## 4. Project status and build

**Status (2026-10-31)**
- Roadmap: 18 OpenSpec changes in `openspec/changes/`, all validated. `bootstrap-app-shell` is
  archived. `add-screen-capture`, `add-annotation-editor` and `add-core-annotation-tools` are all
  **fully implemented and reviewer-verified, but not archived yet** — the first two need live
  testing against a real xdg-desktop-portal/GNOME session (screenshot/window capture, global
  shortcuts, the Print-key takeover; `add-core-annotation-tools` adds one more such check, the
  eyedropper's PickColor call) that was deliberately left to the maintainer rather than run from
  a dev session; see `openspec/changes/add-screen-capture/manual-checks.md` (covers the
  eyedropper row too) and `add-core-annotation-tools/manual-checks.md`. **Archive order:
  add-screen-capture, then add-annotation-editor** (its spec deltas build on the first — task 5.6
  in its `tasks.md`); `add-core-annotation-tools` has no technical ordering dependency on either
  (new `annotation-tools` capability only) but logically assumes the editor exists, so archive it
  third. `add-screen-capture` task 4.6 (a first-launch Print-key-takeover prompt) also still needs
  a designer mockup before it can be built. The other 15 MVP/P2/P3 changes have no implementation
  yet.
- Design: kit in `docs/design/kit/`, mockups for all 18 changes (267 screens) in `docs/design/<change>/`,
  cross-change decisions in `docs/design/decisions.md`. **The 7 MVP (0.1) changes are
  design-approved** (bootstrap-app-shell, add-screen-capture, add-annotation-editor,
  add-core-annotation-tools, add-style-system, add-export-share, add-capture-library);
  P2/P3 changes stay `_pending_`. All cross-change decisions (D1–D13) are settled; none block
  implementation.
- Code: `bootstrap-app-shell` (archived) — AdwApplication, CLI, GSettings-backed preferences
  dialog, keyboard shortcuts window, About dialog. `add-screen-capture` — `src/capture/`:
  Screenshot-portal access, the region overlay (selection, handles, loupe, snapping, nudging),
  window/screen/delayed capture, the GlobalShortcuts portal and GNOME-custom-keybinding fallback
  (D9 Print-key takeover, GNOME-keybinding path only). `add-annotation-editor` — `src/document/`,
  `src/render/`, `src/editing/`, `src/tools/image.rs`+`group.rs`, `src/ui/editor/`: full scene
  graph, undo/redo, `.annota` format with forward-compatible unknown-data preservation, a
  viewport-sized canvas, selection/handles/rotation/snapping/grouping, the real editor window
  (header bar, sidebar, tray, zoom, autosave/recovery), crop/resize/rotate/flip/canvas-extension/
  flatten. `--open <file>` and captures/paste/drop now really open in this editor (the previous
  PNG-quick-save and placeholder-toast stand-ins are gone). `add-core-annotation-tools` —
  `src/tools/{arrow,pen,shape,text,callout,step,highlighter,redaction}.rs`: all 9 concrete
  annotation types against the `Annotation` trait, single-key tool shortcuts, a property sidebar
  per type, a colour picker with a real-portal eyedropper, a shadow editor. Each of these three
  changes took multiple implementation phases and review rounds — see each change's `design.md`
  "Decided during apply" history before touching its code again, it's long for a reason.
- Packaging: `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean for all four
  implemented changes, verified repeatedly in this dev container (cargo/rustc 1.93, not the
  pinned 1.85 — several real bugs surfaced and got fixed here regardless, see each change's
  `tasks.md`). CI should be green (its known step-order bug is fixed) but hasn't been confirmed
  on an actual ubuntu-24.04 runner — check the Actions tab before trusting it blindly.
- Next step: the maintainer runs the live portal/hotkey checks tracked across
  `add-screen-capture/manual-checks.md` and `add-core-annotation-tools/manual-checks.md`, then
  all three changes archive in order (screen-capture, annotation-editor, core-annotation-tools).
  After that, `add-style-system` is next in the roadmap (quick styles/favourites, themes) — the
  last 3 MVP changes after it are `add-export-share`, `add-capture-library`.

**Build dependencies (Ubuntu 24.04)**
```bash
sudo apt install -y debhelper meson ninja-build pkgconf cargo-1.85 rustc-1.85 \
  libgtk-4-dev libadwaita-1-dev blueprint-compiler gettext desktop-file-utils \
  appstream libglib2.0-bin lintian devscripts
```

**Develop**
```bash
export PATH=/usr/lib/rust-1.85/bin:$PATH          # Ubuntu 24.04 only
meson setup _build -Dprofile=development           # also generates src/config.rs
meson compile -C _build && meson test -C _build
_build/src/annota                                  # needs the gresource installed: use meson devenv
meson devenv -C _build ./src/annota
```

**Package**
```bash
build-aux/make-deb.sh                              # → ../annota_<version>_amd64.deb, lintian-checked
sudo apt install ../annota_*.deb
```
After adding or updating crates: `cargo update` with Rust 1.85, then `build-aux/update-copyright.py`.

**Mockups**: `python3 tools/render-mockups.py docs/design/<change>` (needs `pip install playwright`
and `playwright install chromium`).

---

## 5. Mandatory workflow: Design → Spec → Code → Package

Every user-visible feature goes through these gates. No gate may be skipped.

```
 ┌──────────┐   ┌────────────┐   ┌───────────┐   ┌──────────┐   ┌───────────┐
 │ 1 Propose│ → │ 2 Design   │ → │ 3 Approve │ → │ 4 Apply  │ → │ 5 Verify  │ → archive
 │ /opsx:   │   │ designer   │   │ maintainer│   │ gtk-     │   │ reviewer  │
 │ propose  │   │ mockups    │   │ (human)   │   │ engineer │   │ + .deb    │
 └──────────┘   └────────────┘   └───────────┘   └──────────┘   └───────────┘
```

1. **Propose** — `/opsx:propose <idea>` creates `openspec/changes/<id>/` with proposal, spec deltas,
   design.md, tasks.md. Changes that already exist in `openspec/changes/` are the roadmap: pick the
   next one in phase order (see `docs/feature-catalog.md`).
2. **Design** — delegate to the `designer` subagent. It produces high-fidelity mockups for every
   screen, state and scenario touched by the change in `docs/design/<change-id>/`
   (HTML source + PNG in light and dark, 1× and 2×) and an `index.md` that maps each spec scenario
   to a mockup. `design.md` of the change must link these files.
3. **Approve** — show the PNGs to the maintainer (send files or build the review gallery). Do not start code until
   the maintainer has said the design is approved. Record approval as a line in the change's
   `design.md` (`Design approved: <date>`).
4. **Apply** — `/opsx:apply <id>`; implementation follows the mockups exactly (spacing, icons,
   labels, shortcuts). The `gtk-engineer` agent may be used for implementation chunks.
   Cross-change design decisions live in `docs/design/decisions.md` and override single redlines.
5. **Verify** — the `reviewer` agent (fresh context, did not write the code) checks: every
   scenario has a test or a documented manual check, UI matches mockups (screenshot comparison),
   clippy/fmt/tests green, `.deb` builds, lintian clean. Then `/opsx:archive <id>`.

Bugfixes without visible behaviour change skip step 2–3 but still need a test.

---

## 6. OpenSpec conventions

- Specs and changes are written in **English**.
- Capability names are stable, behaviour-oriented, flat: `capture`, `editor-canvas`,
  `annotation-tools`, `style-system`, `export-share`, `document-format`, `app-shell`,
  `debian-packaging`, `privacy-redaction`, `image-effects`, `capture-library`, `templates`,
  `text-recognition`, `screen-recording`, `scrolling-capture`, `step-capture`, `smart-editing`.
- Requirements use SHALL/MUST; every requirement has at least one `#### Scenario:` with
  WHEN/THEN. Scenarios must be testable.
- `openspec validate --all --strict` must pass before every commit that touches `openspec/`.
- Planned roadmap changes live in `openspec/changes/` with the phase in their `proposal.md`.

---

## 7. Coding rules

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` clean. No `unwrap()` outside tests;
  use `anyhow` in the app layer and `thiserror` in library modules.
- UI is declared in Blueprint (`data/ui/*.blp`), not built imperatively, except for the canvas.
- All user-facing strings go through gettext (`gettext!`/`i18n()`), German translation updated
  in the same commit.
- Every annotation tool is a type implementing a common `Annotation` trait
  (`hit_test`, `bounds`, `render(cairo)`, `handles`, `properties`) and is serialisable with serde.
  New tools must add a golden-image test in `tests/render/`.
- Undo/redo: every model mutation is a `Command`. No direct mutation from UI code.
- Keyboard: every tool has a single-key shortcut (shown in tooltips and the shortcuts window);
  all actions are reachable without a mouse; accessibility labels on every control.
- Performance budget: editor opens a 4K capture in < 500 ms; canvas stays at 60 fps with
  200 objects; idle CPU 0 %.
- Never log or persist capture contents outside the user's chosen locations.

---

## 8. Debian packaging rules

- Package name `annota`, section `graphics`, `Architecture: any`.
- Source format `3.0 (native)`. Build offline: `build-aux/vendor.sh` vendors crates into `vendor/`
  and writes `.cargo/config.toml` (both git-ignored); `debian/rules` refuses to build without them
  and uses `dh $@ --buildsystem=meson` with the Rust 1.85 toolchain.
- `debian/copyright` lists all vendored crate licenses; refresh with `build-aux/update-copyright.py`.
- `lintian -I --pedantic` must show no errors or warnings.
- Runtime deps from `${shlibs:Depends}` plus `xdg-desktop-portal`, `xdg-desktop-portal-gnome`;
  add `gstreamer1.0-plugins-good`, `gstreamer1.0-pipewire` with screen recording;
  `Recommends: tesseract-ocr`.
- Install desktop file, metainfo, icons, GSettings schema, locale files; `appstreamcli validate`
  and `desktop-file-validate` pass.
- Version scheme `0.MINOR.PATCH` until 1.0; `debian/changelog` updated with each release tag.
- The `debian-packager` agent owns `debian/` and the CI job that builds the `.deb`.

---

## 9. Subagents

| Agent | Use for |
|---|---|
| `designer` | Design kit, mockups for every change, app icon, tool icons, stamp art, marketing screenshots for README/metainfo |
| `gtk-engineer` | Implementation chunks of an approved change (Rust/GTK/libadwaita) |
| `reviewer` | Independent verification of a change against specs, mockups and quality gates |
| `debian-packager` | `debian/`, Flatpak manifest, CI packaging jobs, release builds |

Delegate with a precise brief: change id, scenario list, file paths, acceptance criteria.

---

## 10. Communication with the maintainer

- The maintainer (Thorsten) writes in German; answer in German, keep artefacts in English.
- Short status, no filler. Show mockups as images, not as descriptions.
- Ask before: changing a fixed decision (§2), adding a dependency whose license would force a
  stronger copyleft onto the MIT-licensed binary, anything that sends data over the network.

