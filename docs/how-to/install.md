# How to install Annota

Annota runs on Ubuntu 24.04 or newer and Debian 13 with GNOME (Wayland first, X11 supported).

## Signed APT repository (recommended)

Every release is signed with Annota's GPG release key, so `apt` verifies it on install and every
later upgrade:

```bash
curl -fsSL https://trosinde.github.io/Annota/annota-archive-keyring.asc \
  | sudo tee /usr/share/keyrings/annota-archive-keyring.asc
echo "deb [signed-by=/usr/share/keyrings/annota-archive-keyring.asc] \
  https://trosinde.github.io/Annota/apt noble main" \
  | sudo tee /etc/apt/sources.list.d/annota.list
sudo apt update && sudo apt install annota
```

New releases then arrive with `sudo apt update && sudo apt upgrade`.

## Direct download

Grab the `.deb` from the [latest release](https://github.com/trosinde/Annota/releases/latest) and
install it without adding a repository (not verified by `apt` itself, and no automatic updates):

```bash
sudo apt install ./annota_*.deb
```

Every release tag (`vX.Y.Z`) publishes both paths: the unsigned `.deb` on the GitHub Release and
the signed APT repository on GitHub Pages. The raw public key is
[`packaging/annota-archive-keyring.asc`](../../packaging/annota-archive-keyring.asc); how the key
is generated, rotated and revoked is in [ADR 0001](../adr/0001-signed-apt-repo.md).

## Build from source

On Ubuntu 24.04:

```bash
sudo apt install -y meson ninja-build pkgconf cargo-1.85 rustc-1.85 \
  libgtk-4-dev libadwaita-1-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  blueprint-compiler gettext desktop-file-utils \
  appstream libglib2.0-bin gstreamer1.0-pipewire
git clone https://github.com/trosinde/Annota.git && cd Annota
export PATH=/usr/lib/rust-1.85/bin:$PATH
meson setup _build -Dprofile=development && meson compile -C _build
meson devenv -C _build ./src/annota
```

On Ubuntu 26.04 or Debian 13, install `cargo rustc` instead of the `-1.85` packages and skip the
`export` line. To build a Debian package yourself, see
[Build the Debian package](../development/building.md).
