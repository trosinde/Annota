# Manual checks — add-export-share

Everything around the system boundaries is covered automatically: unit tests (`src/export/tests.rs`,
`src/ui/editor/{share,saving,export_dialog,redaction_warning}.rs`) and the live-GTK tests
`export_and_save_on_a_live_display` and `copy_drag_and_share_on_a_live_display` in
`src/ui/editor/mod.rs` (`#[ignore]`d; one per process, see below). Those live tests replace the
portals with a stand-in (`Inner::sharer`), never open a file chooser, and write only to temporary
folders. What they cannot cover is the other side of each boundary — a real mail app, a real app
chooser, a real drop target, a real paste — which needs the maintainer's real desktop session.
Nothing here was run against a live xdg-desktop-portal from a development session.

Run the live tests on a private bus where nothing can be activated (plain `dbus-run-session`
uses the session config, which lets GTK's own settings lookup start `xdg-desktop-portal` on the
private bus — isolated from the real session, but not needed):

```
GDK_BACKEND=x11 xvfb-run -a dbus-run-session --config-file=<no-servicedir.conf> \
  cargo test --bin annota -- --ignored --exact ui::editor::tests::copy_drag_and_share_on_a_live_display
```

where `<no-servicedir.conf>` is a session bus config (`<type>session</type>`, a `unix:tmpdir`
listen address, `EXTERNAL` auth, an allow-all policy) **without** any `<servicedir>` /
`<standard_session_servicedirs/>`.

## Pending (maintainer, real GNOME session; Wayland and X11 where marked)

| # | Check | Expected |
|---|---|---|
| 1 | Share → Email… on a capture named "Checkout flow" (Wayland and X11) | Toast "Opening your mail app with “Checkout flow.png” attached…" while the portal is asked; the default mail app opens a new message with `Checkout flow.png` attached; the toast goes when the portal returns. Sending the mail later (after Annota quit) still attaches the image. |
| 2 | Share → Email… with no mail app configured | A toast "Could not open your mail app" (or the portal's own message); nothing else happens. |
| 3 | Share → Open With… | The system's app chooser lists image apps; the chosen app opens `Checkout flow.png`. Closing the chooser shows no toast. |
| 4 | Share → Copy to Folder… | GTK's file chooser (title "Copy to Folder", button "Copy", name `Checkout flow.png`, the export folder); after Copy, the PNG is there and the toast "Copied as “Checkout flow.png”" offers Show in Files. |
| 5 | Drag the header's title area into a Files window (Wayland and X11) | Hover: rounded fill behind title + subtitle, tooltip "Drag image to another app"; the rest of the header still moves the window. While dragging: 160 × 100 thumbnail with a "PNG" badge, title area pressed. Drop: `Checkout flow.png` appears in the folder. |
| 6 | Drag the title area into an app that takes image data but no files (e.g. a chat or a document editor) | The image is inserted (`image/png`). |
| 7 | Ctrl+C with nothing selected, paste into another app (Wayland and X11) | Toast "Image copied to clipboard" for 3 s; the annotated image is pasted. Quit Annota after copying on Wayland: pasting still works until another app takes the clipboard. |
| 8 | Each of 1, 3, 4, 5, 7 on a document with a blur or pixelate redaction | "Make Redactions Solid?" first, with "Share Anyway" (1, 3, 4), "Drag Anyway" (5) or "Copy Anyway" (7). For a drag the drag does not start; after either button, the toast "You can drag the image out now" and the next drag goes through. |
