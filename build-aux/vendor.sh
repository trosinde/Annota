#!/bin/sh
# Vendor all crates into vendor/ so the Debian build runs offline.
# Writes .cargo/config.toml with a source replacement. Both are git-ignored.
set -eu
cd "$(dirname "$0")/.."
RUST_BIN=/usr/lib/rust-1.85/bin
[ -x "$RUST_BIN/cargo" ] && PATH="$RUST_BIN:$PATH"
rm -rf vendor .cargo
mkdir -p .cargo
cargo vendor --locked --versioned-dirs vendor > .cargo/config.toml
echo "vendored $(ls vendor | wc -l) crates"
