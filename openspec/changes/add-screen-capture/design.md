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

Design approved: 2026-10-30; screen 08 redrawn with Close / Ask Again and approved 2026-11-01, also
the "could not ask" toast text

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

## Decided 2026-11-01: the screenshot permission
Found on the maintainer's GNOME Shell 50.1 / xdg-desktop-portal 1.21.1 (Wayland, `.deb`): every
frozen capture ended in "Screen Access Denied", whose Settings switch does not exist, while the
permission store had no entry at all. Journal: `Failed to associate portal window with parent
window` and `Only the focused app is allowed to show a system access dialog`.

**Cause, read from the sources of the installed versions.** This chain is read, not observed:
1. `check_non_interactive_permission_in_thread` (`src/screenshot.c`, xdg-desktop-portal 1.21.1):
   `yes` passes, `no` refuses, anything else shows the access dialog. Interactive requests skip all
   of this. A failed dialog logs `Failed to show access dialog`, stores nothing and answers response
   code 2, the same code as a stored `no`. So code 2 does not mean "denied".
2. `handle_access_dialog` (`src/access.c`, xdg-desktop-portal-gnome 50.0): Annota's empty parent
   handle gives the `Failed to associate` warning. With a valid parent it shows its own dialog on
   that window. Without one it delegates to GNOME Shell.
3. `AccessDialogAsync` (`js/ui/accessDialog.js`, extracted from the installed GNOME Shell 50.1):
   refuses unless `<appId>.desktop` is the focused app. Annota had just hidden all its windows.

**The Settings switch.** `screenshots_row` in gnome-control-center 50.0's `cc-applications-panel.c`
shows only if `get_portal_app_id` finds `X-Flatpak` or `X-SnapInstanceName` in the desktop file, and
an entry exists. An unsandboxed app never gets it, which matches what the maintainer saw (50.3 is
installed; its binary has the row).

**Decisions.**
- **Ask first, with a window.** `permission::first_step` keeps an Annota window on screen when the
  store holds no answer and the mode is region or screen. It uses the active window, else any
  visible one, else a new main window (shortcut and cold `--capture`). `portal::ask_permission`
  passes it as parent through `WindowIdentifier::from_native`. Focus is waited for up to 2 s, but
  with a parent handle GNOME doesn't need it; it matters only if no handle could be made, where the
  Shell's check applies.
- **The asking request's image is discarded unread.** On Allow the portal grabs at once, with
  Annota's window in the picture, so that file is deleted. The capture is a second request with the
  windows hidden, now without a question. The question comes before any countdown.
- **A window opened only to ask is destroyed** before the capture, so `window_was_open` and
  "Copy to clipboard opens no window" keep their meaning. Only that window: see "Review follow-ups".
- **Only the store decides what code 2 means.** `map_portal_error` gives `CaptureError::Refused`;
  `permission::after_refusal` sorts it by a fresh `Lookup` into denied (stored `no`), ask once more,
  could not ask, or plain failure. Window mode is always a plain failure, since the portal doesn't
  check it. `PortalError::NotAllowed` is sorted the same way.
- **The store is read on every capture, and written only by Ask Again.** Reading is `Lookup` on
  `org.freedesktop.impl.portal.PermissionStore` over GIO, 2 s timeout. `NotFound` means undecided;
  any other error means unreadable. An unreadable store (a sandbox) grabs first, so an existing
  permission never flashes a window, and asks once if refused. The only write is
  `DeletePermission(screenshot, screenshot, io.github.trosinde.Annota)` when the user clicks Ask
  Again in the "Screen Access Denied" dialog: Annota's own entry, no other app's. It grants nothing
  and captures nothing; the next region or screen capture finds no answer, so GNOME asks.
- **New crates:** `ashpd`'s `gtk4` feature adds `gdk4-wayland`, `gdk4-x11` and their `-sys` crates
  (MIT; `gtk4-wayland.pc` and `gtk4-x11.pc` are in `libgtk-4-dev`). Regenerate `vendor/` with
  `build-aux/vendor.sh`.

**Dialog text: approved by the maintainer.** Mockup 08 was redrawn with Close and Ask Again, and the
"could not ask" toast text was approved as is. It names no Settings switch and no
command to copy: GNOME Settings shows no such switch for an unsandboxed app.

**Ask Again, maintainer's decisions (2026-11-01).**
- Success closes the dialog with no toast. An entry that is already gone (`NotFound`) is a success.
- Failure keeps the dialog open with one more line, "The answer could not be reset" / "Die Antwort
  konnte nicht zurückgesetzt werden", no new screen. Mockup 08 doesn't draw this line. The error is
  logged; it holds no capture content. A new try is possible at once and clears the line.
- Decided during apply: while the call runs (5 s timeout at most, since the store replies after
  writing its file) Ask Again and Close are insensitive and the dialog can't be closed, because once
  the delete is sent, Close and Esc could no longer keep the stored answer. The UI doesn't block.
  A timeout counts as a failure, although the store may still have deleted the entry; a new try then
  ends in success.
- Decided during apply: `Window::set_focus_visible` shows the focus ring mockup 08 draws, which GTK
  otherwise holds back until the first key press.

**Review follow-ups (2026-11-01), after an independent review.**
- **The question can take long, so the windows are looked at when it ends.** The window opened to ask
  is remembered by `with_permission`, and `run_hidden` destroys that one and nothing else. An editor
  opened during the question used to be destroyed as "not open at the start", skipping "Save
  Changes?". Now it hides for the grab and comes back. What comes back is what was hidden, if it still
  exists, so a window closed meanwhile isn't shown again. `window_was_open` is whatever is on the
  screen after that.
- **The question dismissed without an answer** (response code 1) also destroys the window opened for
  it. After a cold-start Deny it stays open, as built: that is with the maintainer as a separate
  question. A toast's window stays too, because the toast is on it.
- **A closed parent can't panic.** `ashpd` 0.11.1's `Gtk4WindowIdentifier::drop` unwraps
  `native.surface()` when it exported a Wayland handle, and the request builder keeps the identifier
  until the response, so closing the window during the question would have panicked on the answer.
  `portal::ParentWindow` keeps the identifier itself and hands the request a plain copy of the handle
  (the `WindowIdentifier::X11` variant, public but `doc(hidden)`, holds either kind; check this when
  updating `ashpd`). The identifier is released on `unrealize`, ahead of GTK's handler that destroys
  the surface, or when the request ends, which keeps the export alive for as long as the portal needs
  it. If the window closed while `from_native` waited for the compositor there is no surface and no
  signal to come: the identifier is forgotten, leaking one reference to a closed window.
- **A stored "no" makes no request.** The portal would refuse, so `FirstStep::Denied` shows the dialog
  at once: no countdown first, and no window hides and returns. If the store says `no` under Annota's
  ID, the portal reads the same entry. Other refusals still go through `after_refusal`.
- **Messages go to the editor when it is on the screen** (`application::present_for_message`), as
  mockup 08 draws it: the denied dialog, the could-not-ask and failure toasts, and two toasts the
  review didn't name but which had the same bug (the clipboard confirmation of a capture, and
  `import`'s messages). If main window and editor are both open it's the editor, even if the main
  window started the capture, because that's where the capture would have gone.
- **One denied dialog.** `permission::present` raises the open one instead of stacking another.
- **A grab waits for the registration.** `ensure_registered` set its flag before `register_host_app`
  finished, so a cold `--capture` could overtake the startup's registration and reach the portal
  without Annota's ID. It's now one shared future everybody awaits. A registry that never answers
  would hold the capture, as would a Screenshot request that never answers: neither has a timeout.

**Known limits.**
- The Wayland branch of `ParentWindow` has not run at all: `live_tests` run on X11, where `ashpd`'s
  drop does nothing. They prove the identifier is released on `unrealize` while the surface exists, and
  that the flow survives the closed window. They don't prove the export, GNOME's reaction to a parent
  that goes away during its question, or what it answers then. See `manual-checks.md`, G.
- An unreadable store with a capture delay counts down, is refused, asks, and counts down again. Not
  changed: the store is only unreadable in a sandbox, and Annota isn't shipped in one.
- Nothing here has run against a real portal or store. The tests in `src/capture/live_tests.rs` use
  a stand-in written from the sources above, so they can't show a misreading of them. That includes
  `DeletePermission`, whose stand-in follows `handle_delete_permission` from memory of
  xdg-permission-store, not from its source. See `manual-checks.md`.
- A stored `ask`, which nothing in GNOME writes, asks on every request, so the second request
  would be refused and end in "could not ask".
- Before xdg-desktop-portal 1.19 (Ubuntu 24.04 has 1.18) there is no host app registry. If the portal
  can't derive Annota's ID from the systemd unit, it stores the answer under the empty ID, which
  Annota doesn't read: it would ask first on every capture, and a stored `no` would show as "could
  not ask". Check this on 24.04.

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- Nothing in this change was run against a live xdg-desktop-portal (development happened on the
  maintainer's own desktop). The portal calls follow `ashpd` 0.11's API; `manual-checks.md` lists
  what to verify on a real session before archiving.
- Whether the first-use question really appears over the parent window on GNOME 46 and 50: see
  "Decided 2026-11-01: the screenshot permission".
- Mixed-scale multi-monitor layouts assume one scale factor for the whole portal image.
