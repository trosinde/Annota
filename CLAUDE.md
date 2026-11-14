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
| Screen access | Only via **xdg-desktop-portal** (`ashpd` crate): Screenshot portal for region and screen stills, ScreenCast portal + PipeWire for window stills ([ADR 0002](docs/adr/0002-screencast-for-window-stills.md)) and video. No X11-only hacks, no root, no compositor-private APIs. |
| Global hotkey | GlobalShortcuts portal when available (GNOME ≥ 48); fallback: register a GNOME custom keybinding that runs `annota --capture <mode>` |
| Rendering | One renderer on **cairo-rs** used for the canvas *and* for export → pixel-identical WYSIWYG |
| Document format | `.annota` = ZIP: `original.png` + `document.json` (versioned scene graph, schema in `data/schema/`) + `thumbnail.png` |
| Video | GStreamer (`gstreamer-rs`), output MP4 (H.264) / WebM (VP9); GIF via `gifski`. GStreamer also takes the single frame of a window still (ADR 0002). |
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

**Status (2026-11-14)**
- Archived: `bootstrap-app-shell` and `add-signed-apt-repo` (2026-10-30); `add-screen-capture`,
  `add-annotation-editor`, `add-core-annotation-tools` and `add-style-system` (2026-11-10). Their
  deltas are merged into `openspec/specs/`: `app-shell` (7 requirements), `capture` (10),
  `annotation-tools` (16), `document-format` (11), `editor-canvas` (31), `style-system` (11),
  `debian-packaging` (7). `openspec validate --all --strict` passes.
- **First real-desktop run, 2026-11-10** (GNOME Shell 50.1, Wayland, xdg-desktop-portal 1.21.1):
  the screenshot permission flow, region capture and full-screen capture work; the permission
  store now holds `yes` for Annota. The "Screen Access Denied" dialog seen before that came from a
  stale `/usr/local/bin/annota` predating the 2026-11-01 permission rework — GNOME had refused to
  *show* the question ("Only the focused app is allowed to show a system access dialog"), which
  the old binary mistook for a denial. Most rows of each archived change's `manual-checks.md` are
  still unrun; they moved into the archive with their change.
- `add-export-share`: 18/19 tasks. Open: 3.3 (`.deb` on Ubuntu 24.04 — `libwebp-sys` needs exactly
  Rust 1.85, so far only read, never built) and all 18 maintainer live checks in its
  `manual-checks.md`. Its last reviewer pass ended conditionally — the toast-placement blocker was
  fixed in `b2a151c` without a further pass, and mockups 11 and 13 are known stale.
- `fix-region-pointer-inclusion` (proposed 2026-11-10): with "include pointer" on, a region
  capture never shows the pointer, although the `capture` spec requires it for region *and* screen
  captures. The overlay keeps only the first pointer position it sees, so the arrow is drawn
  outside the crop and cairo clips it away silently. Direction A of three, chosen by the
  maintainer; design gate open.
- `fix-window-capture-screencast` (2026-11-14, ADR 0002): window mode never delivered a window
  image. The Screenshot portal has no window mode, and its interactive UI always starts in
  "Selection", so the Window card showed a region overlay. Window stills now go through the
  ScreenCast portal's window chooser and one GStreamer frame, with Annota's own "Waiting for a
  window…" indicator as the chooser's parent. Code, unit and fake-portal tests are done. Open:
  `.deb` dependencies, the automated live suite (`tools/live-window-capture.py`, AT-SPI +
  `tools/portal-input.py`), the Rust 1.85 build (CI), review. Defect class: the spec described
  portal behaviour that no probe had ever measured - see §5 "Assumptions about the portal".
- `fix-portal-shortcut-bindings` (parked at 2/23): GlobalShortcuts bindings that GNOME keeps from
  an earlier run do not fire.
- `add-capture-library` is the last MVP change not started (18 tasks, design-approved). The other
  P2/P3 changes have no implementation.
- `add-screen-capture` task 4.6 (the first-launch Print-key-takeover prompt, D9) was archived
  **unbuilt** — it never had a mockup. Delivering D9 now needs its own change. On GNOME >= 48, the
  maintainer's own machine included, the GlobalShortcuts portal path applies and there is no
  takeover at all.
- **A defect class found twice on 2026-11-10, worth checking for:** a change that writes a
  MODIFIED delta against a capability whose main spec does not exist yet cannot be validated, and
  tends to carry a placeholder scenario that becomes false the moment it ships. `openspec archive`
  then refuses it. Both instances were in `add-export-share` (on `document-format` and on
  `annotation-tools`) and were fixed with REMOVED + ADDED, the pattern `add-annotation-editor`
  already used for "Command-line interface". Before archiving a change that creates a new main
  spec, check which active changes carry MODIFIED or RENAMED deltas on that capability.
- Design: kit in `docs/design/kit/`, mockups for all 18 earlier changes (267 screens) in
  `docs/design/<change>/`, cross-change decisions in `docs/design/decisions.md`. **The 7 MVP (0.1)
  changes are design-approved**; P2/P3 changes stay `_pending_`, as does
  `fix-region-pointer-inclusion` until its mockups are approved. All cross-change decisions
  (D1-D13) are settled; none block implementation.
- Code: `bootstrap-app-shell` - AdwApplication, CLI, GSettings-backed preferences dialog, keyboard
  shortcuts window, About dialog. `add-screen-capture` - `src/capture/`: Screenshot-portal access,
  the region overlay (selection, handles, loupe, snapping, nudging), window/screen/delayed
  capture, the GlobalShortcuts portal and GNOME-custom-keybinding fallback (D9 Print-key takeover,
  GNOME-keybinding path only). `add-annotation-editor` - `src/document/`, `src/render/`,
  `src/editing/`, `src/tools/image.rs`+`group.rs`, `src/ui/editor/`: full scene graph, undo/redo,
  `.annota` format with forward-compatible unknown-data preservation, a viewport-sized canvas,
  selection/handles/rotation/snapping/grouping, the real editor window (header bar, sidebar, tray,
  zoom, autosave/recovery), crop/resize/rotate/flip/canvas-extension/flatten. `--open <file>` and
  captures/paste/drop really open in this editor. `add-core-annotation-tools` -
  `src/tools/{arrow,pen,shape,text,callout,step,highlighter,redaction}.rs`: all 9 concrete
  annotation types against the `Annotation` trait, single-key tool shortcuts, a property sidebar
  per type, a colour picker with a real-portal eyedropper, a shadow editor. `add-style-system` -
  `src/styles/`, `src/ui/editor/{quick_styles,theme_editor,styles}.rs`: quick styles (save/
  rename/move/delete, applied through the same property path the sidebar uses), favourites, a
  theme editor + generator (2-8 colours + fonts -> 8 quick styles per tool), import/export and the
  system theme directory. Each of these changes took multiple implementation phases and review
  rounds - see each change's `design.md` "Decided during apply" history before touching its code
  again, it's long for a reason.
- Packaging: `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean, verified only on
  the maintainer's Ubuntu 26.04 desktop (cargo/rustc 1.93, not the pinned 1.85; never on 24.04 -
  several real bugs surfaced and got fixed here regardless, see each change's `tasks.md`). CI
  should be green (its known step-order bug is fixed) but hasn't been confirmed on an actual
  ubuntu-24.04 runner - check the Actions tab before trusting it blindly.
- **Telling builds apart:** the version is `0.1.0` until the next release, so it never identifies a
  build. A development build (`PROFILE=debug`) therefore carries its commit and build time, from
  `build.rs` through `application::version_string()` into `annota --version` and the About dialog;
  a release build and the `.deb` carry the bare version. If About shows `0.1.0` alone on a
  locally installed binary, it predates 2026-11-10 and is stale. A locally installed Annota also
  has to be quit before reinstalling, or the D-Bus single instance forwards every new launch into
  the old process.
- Next step: the reviewer pass on `add-export-share`'s unreviewed fixes, then archive it; the
  design gate for `fix-region-pointer-inclusion`; then `add-capture-library`, the remaining MVP
  change. The maintainer's live checks in the archived changes' `manual-checks.md` stay open.

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

Bugfixes without visible behaviour change skip step 2–3 but still need a test, tagged with the
scenario the bug violated (add the scenario first if the spec had none).

**Scenario gate (quality gate at the end of every implementation):**
- **Every test references a scenario.** Directly above each test:
  `// spec: <capability> / <Requirement> / <Scenario>`, titles exactly as in the spec headings,
  several tags allowed. A test that proves no scenario is either missing its scenario (add it to
  the spec first) or is not needed.
- **No implemented scenario without a test.** Every scenario in `openspec/specs/` and every
  ADDED/MODIFIED scenario of the change being finished has a green tagged test. Only where
  automation is impossible: a row in the change's `manual-checks.md`
  (`Spec | Check | Why not automated | Result | Date`) with Result `pass` and a date. Never invent a
  result.
- `python3 tools/scenario_gate.py check` runs in CI (static). `python3 tools/scenario_gate.py
  verify <change-id>` runs the tagged tests, live-GTK tests included (`openspec/scenario-gate.toml`),
  is the last step of the Verify task group and writes `openspec/changes/<id>/scenario-report.md`.
  A change is archived only when it exits 0.
- `openspec/scenario-baseline.txt` lists the violations that predate the gate (2026-11-14: all 604
  tests untagged, all 215 scenarios uncovered). It may only shrink; never add entries, never put
  the current change's scenarios there.

**Verification rules, each paid for with a shipped defect:**
- A requirement whose only evidence is a row in `manual-checks.md` must have that row run before
  the change is archived. `add-screen-capture` was archived with its window-capture row unrun;
  the mode had never once delivered an image, and nothing in the repository could have said so.
- Never change what the user is told about a failure without evidence of which failure it is.
  Where the causes cannot be told apart — the Screenshot portal answers response code 2 both for
  a picker the user closed and for one that never opened — the message must be true for all of
  them. Turning a wrong message into no message is not a fix.
- Assumptions about the portal are *measured*, with `tools/portal-probe.py`, never believed. The
  live tests in `src/capture/live_tests.rs` answer every portal call themselves, so they prove
  what Annota sends and how it sorts the answers, and can never catch a wrong assumption about
  what the real portal does. Every sentence in a spec that describes portal behaviour needs one
  probe run behind it.

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
  add `gstreamer1.0-pipewire`, `gstreamer1.0-plugins-base` for window stills (ADR 0002) and
  `gstreamer1.0-plugins-good` with screen recording;
  `Recommends: tesseract-ocr`.
- Install desktop file, metainfo, icons, GSettings schema, locale files; `appstreamcli validate`
  and `desktop-file-validate` pass.
- Version scheme `0.MINOR.PATCH` until 1.0; `debian/changelog` updated with each release tag.
- The `debian-packager` agent owns `debian/` and the CI job that builds the `.deb`.

---

## 9. Subagents

| Agent | Use for |
|---|---|
| `designer` | Design kit, mockups for every change, app icon, tool icons, stamp art |

README and metainfo images are not mockups: every UI screenshot is taken fresh from the current
build (never from `docs/design/`), framing graphics are made with Claude Design, and the demo
sources that rebuild them live in `docs/assets/source/` (skill `github-repo-showcase`).
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

