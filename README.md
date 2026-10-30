# Annota

**Capture it. Mark it up. Make it obvious.**

Annota is a native GNOME screenshot, annotation and screen-recording app for Ubuntu and Debian.
Arrows, callouts, numbered steps, highlights and redaction stay editable until you export.
Built in Rust with GTK 4 and libadwaita, shipped as a native Debian package, private by default.

> Status: pre-alpha. Roadmap fully specified (18 OpenSpec changes), 267 mockups awaiting review,
> application skeleton and lintian-clean Debian package in place.

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
| **MVP 0.1** | app shell & packaging · screen capture · editor & `.annota` format · core tools · quick styles & themes · export & share · capture library |
| **Phase 2** | advanced tools · effects · capture presets · text recognition · smart redaction · library search, tags & collections · templates · screen recording |
| **Phase 3** | scrolling capture · step capture · smart move & simplify |

## Install

**From the signed APT repository (recommended)** — every release is signed with Annota's GPG
release key, so `apt` verifies it on install and every later upgrade:

```bash
curl -fsSL https://trosinde.github.io/Annota/annota-archive-keyring.asc \
  | sudo tee /usr/share/keyrings/annota-archive-keyring.asc
echo "deb [signed-by=/usr/share/keyrings/annota-archive-keyring.asc] \
  https://trosinde.github.io/Annota/apt noble main" \
  | sudo tee /etc/apt/sources.list.d/annota.list
sudo apt update && sudo apt install annota
```

**Direct download** — grab the `.deb` from the [latest release](https://github.com/trosinde/Annota/releases)
and install it without adding a repository (not verified by `apt` itself):

```bash
sudo apt install ./annota_*.deb
```

Every release tag (`vX.Y.Z`) publishes both paths: the unsigned `.deb` on the GitHub Release and
the signed APT repository on GitHub Pages. See `packaging/annota-archive-keyring.asc` for the
raw public key, and [`docs/adr/0001-signed-apt-repo.md`](docs/adr/0001-signed-apt-repo.md) for how
the key is generated, rotated and revoked.

## Build the Debian/Ubuntu package

On Ubuntu 24.04 (or Debian 13):

```bash
sudo apt install -y debhelper meson ninja-build pkgconf cargo-1.85 rustc-1.85 \
  libgtk-4-dev libadwaita-1-dev blueprint-compiler gettext desktop-file-utils \
  appstream libglib2.0-bin lintian devscripts
build-aux/make-deb.sh            # vendors crates, builds offline, runs lintian
sudo apt install ../annota_*.deb
```

On Debian 13 install `cargo rustc` instead of the versioned `-1.85` packages.
Every push to `main` also builds the `.deb` in GitHub Actions (artifact `annota-deb-ubuntu-24.04`).

## Continue with Claude Code

`CLAUDE.md` is the rule book. Next change: `/opsx:apply <id>`, design gate: `/design <id>`,
agents: `designer`, `gtk-engineer`, `reviewer`, `debian-packager`.
Review all mockups in one page: `python3 tools/build-review-gallery.py` → `docs/design/review.html`.

## How it is built

Design → Spec → Code → Verify → Package. Every user-visible change gets high-fidelity mockups from the
`designer` agent and maintainer approval before any code is written.

```bash
git clone https://github.com/trosinde/Annota.git && cd Annota && claude
# then: /opsx:apply bootstrap-app-shell   (after the design is approved)
```

## License

MIT for code, CC-BY-SA-4.0 for artwork and docs.
Annota is an independent project and is not affiliated with any commercial screenshot tool.
