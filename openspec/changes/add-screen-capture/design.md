# Design: Add screen capture (region, window, screen)

## Mockups (design gate)

Produced by the `designer` agent. Scenario-to-mockup mapping and redlines: [`docs/design/add-screen-capture/index.md`](../../../docs/design/add-screen-capture/index.md).
Source HTML: `docs/design/add-screen-capture/src/`; dark and 2× renders sit next to each light@1x file in `png/`.

- [01-capture-launcher](../../../docs/design/add-screen-capture/png/01-capture-launcher-light@1x.png)
- [02-region-drag](../../../docs/design/add-screen-capture/png/02-region-drag-light@1x.png)
- [03-region-adjust-fixed-size](../../../docs/design/add-screen-capture/png/03-region-adjust-fixed-size-light@1x.png)
- [04-region-snap-window-edge](../../../docs/design/add-screen-capture/png/04-region-snap-window-edge-light@1x.png)
- [05-window-wayland-portal-picker](../../../docs/design/add-screen-capture/png/05-window-wayland-portal-picker-light@1x.png)
- [06-window-x11-hover](../../../docs/design/add-screen-capture/png/06-window-x11-hover-light@1x.png)
- [07-delay-countdown](../../../docs/design/add-screen-capture/png/07-delay-countdown-light@1x.png)
- [08-permission-denied](../../../docs/design/add-screen-capture/png/08-permission-denied-light@1x.png)
- [09-prefs-shortcuts-fallback](../../../docs/design/add-screen-capture/png/09-prefs-shortcuts-fallback-light@1x.png)
- [10-prefs-shortcuts-created](../../../docs/design/add-screen-capture/png/10-prefs-shortcuts-created-light@1x.png)
- [11-shortcut-capture-dialog](../../../docs/design/add-screen-capture/png/11-shortcut-capture-dialog-light@1x.png)
- [12-region-result-in-editor](../../../docs/design/add-screen-capture/png/12-region-result-in-editor-light@1x.png)
- [13-paste-from-clipboard](../../../docs/design/add-screen-capture/png/13-paste-from-clipboard-light@1x.png)
- [14-drop-image-file](../../../docs/design/add-screen-capture/png/14-drop-image-file-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- `ashpd` for Screenshot portal (non-interactive full-screen grab, then own overlay)
- Overlay is a fullscreen undecorated GtkWindow per monitor showing the frozen image
- HiDPI: capture at device pixels, store scale factor in document metadata (the capture is kept
  at device pixels; the document metadata itself arrives with `add-annotation-editor`)
- ~~X11 window picking via overlay hit-test on window geometry from `_NET_CLIENT_LIST_STACKING`~~ —
  superseded during apply, see below.

### Decided during apply (2026-10-30)
- **`ashpd` 0.11** (`default-features = false`, `async-std` = zbus on async-io, no `gtk4` feature):
  0.12+ needs gtk4-rs 0.10 and 0.13 needs Rust 1.87; 0.11 matches gtk4-rs 0.9 and MSRV 1.85.
  Futures run on the GLib main loop (`glib::spawn_future_local`). The lockfile was resolved with
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` so every crate stays ≤ Rust 1.85.
- **Window picking** uses the portal's interactive picker on Wayland *and* X11 (GNOME's
  xdg-desktop-portal-gnome serves both). Reading `_NET_CLIENT_LIST_STACKING` would need a direct X11
  connection, which CLAUDE.md §2 ("only via xdg-desktop-portal, no X11-only hacks") rules out
  without an ADR. Mockup 06 is therefore not implemented.
- **Window-edge snapping** cannot use window geometry (Wayland exposes none). Edges are detected in
  the frozen image instead: long straight runs (≥ 48 px) of strong luma contrast between adjacent
  columns/rows within 8 px of the pointer (`src/capture/snap.rs`). Same behaviour on X11. The dashed
  window outline of mockup 04 is not drawn (no window rectangle is known); the violet guide follows
  the detected edge.
- **Monitor under the pointer** (screen mode, include-pointer): Wayland gives no global pointer
  position, so Annota briefly shows the frozen image full-screen on every monitor and takes the
  first pointer enter/motion event (`overlay::probe_pointer`, 400 ms timeout, first monitor as
  fallback). Nothing visibly changes on screen.
- **Include pointer**: the Screenshot portal has no pointer option for non-interactive grabs, so
  Annota draws a standard arrow pointer at the pointer position (region and screen modes). In window
  mode GNOME's picker has its own pointer toggle.
- **Portal file**: the portal writes its PNG to a location of its choosing; Annota loads it and
  deletes that file, so captures only persist where the user asked (CLAUDE.md §7).
- **Countdown OSD**: a transparent, maximised, undecorated window with an empty input region (clicks
  pass through), destroyed 200 ms before the grab.
- **Global shortcuts**: GNOME custom keybindings are written through GSettings
  (`org.gnome.settings-daemon.plugins.media-keys`), merging into — never replacing — the user's
  `custom-keybindings` list. D9's "moved, not deleted" is implemented by removing Annota's keys from
  `org.gnome.shell.keybindings` `show-screenshot-ui` / `screenshot-window` / `screenshot`, recording
  them in Annota's `moved-gnome-keys` key and restoring them when the Annota shortcut is disabled or
  changed. Portal shortcuts are bound at startup when the portal exists; they work while Annota runs
  (no background service or autostart in this change).
- **Editor destination**: no editor exists yet, so "Open in Editor" quick-saves PNG into the save
  folder (`preferences::resolved_save_folder`, `filename-pattern`) with a toast; see `tasks.md`.

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- Nothing in this change was run against a live xdg-desktop-portal (development happened on the
  maintainer's own desktop). The portal calls follow `ashpd` 0.11's API; `manual-checks.md` lists
  what to verify on a real session before archiving.
- Response code 2 ("other") from the Screenshot portal is treated as "permission denied"; confirm
  on GNOME 46 and 50 that a denied permission really answers that way.
- Mixed-scale multi-monitor layouts assume one scale factor for the whole portal image.
