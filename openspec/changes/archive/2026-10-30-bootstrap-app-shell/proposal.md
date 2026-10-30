# Bootstrap app shell and Debian packaging

**Roadmap phase:** MVP (milestone 0.1)

## Why
Annota needs a runnable, installable GNOME application before any feature can be built. Packaging is set up first so every later change is verified as an installed .deb.

## What Changes
- Rust/GTK4/libadwaita application skeleton with Meson + Cargo build
- Single-instance application with D-Bus activation and CLI entry points
- Preferences window, About dialog, keyboard shortcuts window
- Native Debian package with desktop file, metainfo, icons, GSettings schema
- CI pipeline that builds and lints the .deb

## Benchmark features covered
- Launcher integration (app grid, search)
- Command-line capture entry points

## Out of scope
- Any capture or editing functionality
- Flatpak publishing (manifest only)

## Capabilities

### New Capabilities
- `app-shell`
- `debian-packaging`

### Modified Capabilities
- none

## Impact
- New repository structure, CI, debian/ directory
