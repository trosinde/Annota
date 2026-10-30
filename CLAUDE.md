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
| Language | Rust (edition 2021, stable toolchain, MSRV = the `rustc` in Ubuntu 24.04 `rustc-1.80` package or newer) |
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
| License | GPL-3.0-or-later (code), CC-BY-SA-4.0 (artwork, docs) — confirm with the maintainer before first release |
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
├── build-aux/flatpak/         ← Flatpak manifest (secondary)
├── tools/
│   └── render-mockups.py      ← renders design mockups to PNG (Playwright)
├── docs/
│   ├── index.html             ← visual project documentation (GitHub Pages)
│   ├── feature-catalog.md     ← benchmark feature → capability → phase
│   ├── adr/                   ← architecture decision records
│   └── design/
│       ├── kit/               ← Annota design kit (tokens, Adwaita-faithful CSS, icons)
│       └── <change-id>/       ← mockups per OpenSpec change (HTML source + PNG renders)
├── openspec/                  ← specs and changes (see §6)
└── .claude/
    ├── agents/                ← designer, gtk-engineer, reviewer, debian-packager
    ├── commands/opsx/         ← OpenSpec slash commands
    └── skills/                ← OpenSpec skills
```

---

## 4. Bootstrapping the project (first session only)

Run these steps in order. Each step ends with a commit. Stop and report if a step fails.

1. **Toolchain check** (Ubuntu 24.04 host or container):
   ```bash
   sudo apt install -y build-essential meson ninja-build pkg-config gettext \
     libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
     libpipewire-0.3-dev blueprint-compiler desktop-file-utils appstream-util \
     debhelper devscripts dh-cargo lintian tesseract-ocr tesseract-ocr-deu \
     cargo rustc python3-playwright
   rustup component add clippy rustfmt   # if rustup is used
   ```
2. **Design kit first**: run the `designer` agent with the task *"Create the Annota design kit"*
   (see `.claude/agents/designer.md` §Design kit). Output lands in `docs/design/kit/`.
3. **First change**: `/opsx:apply bootstrap-app-shell` — but only after the designer has produced
   and the maintainer has approved `docs/design/bootstrap-app-shell/` (design gate, §5).
4. Skeleton must build and launch: `meson setup build && meson compile -C build && ./build/src/annota`.
5. **Packaging smoke test**: `dpkg-buildpackage -us -uc -b` produces `../annota_*.deb`;
   `lintian` shows no errors; installing it adds Annota to the GNOME app grid.
6. CI: `.github/workflows/ci.yml` runs fmt, clippy (`-D warnings`), tests, `openspec validate --all`,
   and builds the `.deb` on `ubuntu-24.04`. Upload the `.deb` as a workflow artifact.

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
3. **Approve** — show the PNGs to the maintainer (send them as files). Do not start code until
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
- Build offline: crates vendored into `debian/vendor` tarball via `cargo vendor`, referenced by
  `.cargo/config.toml` during build. `debian/rules` uses `dh $@ --buildsystem=meson`.
- Runtime deps from `${shlibs:Depends}` plus `xdg-desktop-portal`, `xdg-desktop-portal-gnome`,
  `gstreamer1.0-plugins-good`, `gstreamer1.0-pipewire`; `Recommends: tesseract-ocr`.
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
- Ask before: changing a fixed decision (§2), adding a dependency with a non-GPL-compatible
  license, anything that sends data over the network.
