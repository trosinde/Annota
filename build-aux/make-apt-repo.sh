#!/bin/sh
# Assemble a minimal signed APT repository for a built annota .deb.
#
# Usage: build-aux/make-apt-repo.sh <path-to-annota_*.deb> <output-dir> <gpg-key-id>
#
# Lays out <output-dir>/{pool/main,dists/noble/main/binary-amd64} and signs
# dists/noble/Release with the given GPG key (must already be imported into
# the calling user's keyring) producing Release.gpg and InRelease.
set -eu

deb="${1:?usage: make-apt-repo.sh <deb> <output-dir> <gpg-key-id>}"
outdir="${2:?usage: make-apt-repo.sh <deb> <output-dir> <gpg-key-id>}"
keyid="${3:?usage: make-apt-repo.sh <deb> <output-dir> <gpg-key-id>}"
suite=noble
component=main
arch=amd64

[ -f "$deb" ] || { echo "make-apt-repo.sh: no such file: $deb" >&2; exit 1; }

rm -rf "$outdir"
pooldir="$outdir/pool/$component"
bindir="$outdir/dists/$suite/$component/binary-$arch"
mkdir -p "$pooldir" "$bindir"
cp "$deb" "$pooldir/"

( cd "$outdir" && apt-ftparchive packages "pool/$component" ) > "$bindir/Packages"
gzip -9c "$bindir/Packages" > "$bindir/Packages.gz"

cat > "$outdir/apt-ftparchive.conf" <<EOF
APT::FTPArchive::Release::Origin "Annota";
APT::FTPArchive::Release::Label "Annota";
APT::FTPArchive::Release::Suite "$suite";
APT::FTPArchive::Release::Codename "$suite";
APT::FTPArchive::Release::Architectures "$arch";
APT::FTPArchive::Release::Components "$component";
APT::FTPArchive::Release::Description "Annota release repository";
EOF

( cd "$outdir" && apt-ftparchive -c apt-ftparchive.conf release "dists/$suite" ) \
  > "$outdir/dists/$suite/Release"
rm -f "$outdir/apt-ftparchive.conf"

gpg --local-user "$keyid" --detach-sign --armor \
  --output "$outdir/dists/$suite/Release.gpg" "$outdir/dists/$suite/Release"
gpg --local-user "$keyid" --clearsign \
  --output "$outdir/dists/$suite/InRelease" "$outdir/dists/$suite/Release"

echo "APT repo assembled at $outdir"
echo "  $(find "$outdir" -type f | wc -l) files"
