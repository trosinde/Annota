# Building Annota

## Develop

On Ubuntu 24.04 (or Debian 13):

```bash
sudo apt install -y debhelper meson ninja-build pkgconf cargo-1.85 rustc-1.85 \
  libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  blueprint-compiler gettext desktop-file-utils \
  appstream libglib2.0-bin lintian devscripts gstreamer1.0-pipewire
export PATH=/usr/lib/rust-1.85/bin:$PATH          # Ubuntu 24.04 only
meson setup _build -Dprofile=development           # also generates src/config.rs
meson compile -C _build && meson test -C _build
meson devenv -C _build ./src/annota
```

On Debian 13 install `cargo rustc` instead of the versioned `-1.85` packages.

## Build the Debian package

```bash
build-aux/make-deb.sh            # vendors crates, builds offline, runs lintian
sudo apt install ../annota_*.deb
```

Every push to `main` also builds the `.deb` in GitHub Actions (artifact `annota-deb-ubuntu-24.04`).
After adding or updating crates: `cargo update` with Rust 1.85, then `build-aux/update-copyright.py`.

Full rules for packaging: [`CLAUDE.md`](../../CLAUDE.md) §8.
