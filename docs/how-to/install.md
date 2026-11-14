# How to install Annota

Annota runs on Ubuntu 24.04 or newer and Debian 13 with GNOME (Wayland first, X11 supported).

## Today: build from source

There is no release yet. Follow [Get started](../../README.md#get-started) in the README, or
build a Debian package yourself: [Build the Debian package](../development/building.md).

## From v0.1.0: signed APT repository (recommended)

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

## From v0.1.0: direct download

Grab the `.deb` from the [latest release](https://github.com/trosinde/Annota/releases) and install
it without adding a repository (not verified by `apt` itself):

```bash
sudo apt install ./annota_*.deb
```

Every release tag (`vX.Y.Z`) publishes both paths: the unsigned `.deb` on the GitHub Release and
the signed APT repository on GitHub Pages. The raw public key is
[`packaging/annota-archive-keyring.asc`](../../packaging/annota-archive-keyring.asc); how the key
is generated, rotated and revoked is in [ADR 0001](../adr/0001-signed-apt-repo.md).
