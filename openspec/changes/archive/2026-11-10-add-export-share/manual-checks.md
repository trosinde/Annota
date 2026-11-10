# Manual checks — add-export-share

Everything around the system boundaries is covered automatically: unit tests (`src/export/tests.rs`,
`src/ui/editor/{share,saving,export_dialog,redaction_warning}.rs`) and the live-GTK tests
`export_and_save_on_a_live_display`, `copy_drag_and_share_on_a_live_display`,
`object_copy_and_paste_on_a_live_display` (task 2.4a), `batch_export_on_a_live_display`
(task 2.5), `save_changes_on_a_live_display` (task 2.6),
`new_capture_toast_copies_the_current_document_on_a_live_display`,
`saves_keep_redactions_without_a_warning_on_a_live_display` and
`typed_names_ask_before_replacing_on_a_live_display` (task 2.8) in `src/ui/editor/mod.rs` (`#[ignore]`d; one per process, see below). Those live tests replace the
portals and the file chooser with stand-ins (`Inner::sharer`, `Inner::save_chooser`), never open a
real file chooser, and write only to temporary folders. What they cannot cover is the other side of each boundary — a real mail app, a real app
chooser, a real drop target, a real paste — which needs the maintainer's real desktop session.
Nothing here was run against a live xdg-desktop-portal from a development session.

Run the live tests on a private bus where nothing can be activated (plain `dbus-run-session`
uses the session config, which lets GTK's own settings lookup start `xdg-desktop-portal` on the
private bus — isolated from the real session, but not needed):

```
XDG_CONFIG_HOME=<empty folder> GDK_BACKEND=x11 xvfb-run -a dbus-run-session --config-file=<no-servicedir.conf> \
  cargo test --bin annota -- --ignored --exact ui::editor::tests::copy_drag_and_share_on_a_live_display
```

where `<no-servicedir.conf>` is a session bus config (`<type>session</type>`, a `unix:tmpdir`
listen address, `EXTERNAL` auth, an allow-all policy) **without** any `<servicedir>` /
`<standard_session_servicedirs/>`. `XDG_CONFIG_HOME` points at an empty folder so that a
personal `~/.config/gtk-4.0/gtk.css` (GTK loads it above Annota's own stylesheet) does not
restyle the window: with the maintainer's Everforest theme, `quick_styles_on_a_live_display`
fails and screenshots (`ANNOTA_SHOTS`) do not show Annota's CSS (task 2.5, design.md).

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
| 8 | Each of 1, 3, 4, 5, 7, 9 on a document with a blur or pixelate redaction | "Make Redactions Solid?" first, with "Share Anyway" (1, 3, 4), "Drag Anyway" (5) or "Copy Anyway" (7, 9). For a drag the drag does not start; after either button, the toast "You can drag the image out now" and the next drag goes through. |
| 9 | Select an arrow and an inserted picture, Ctrl+C, then open a second capture and press Ctrl+V; close the first document and press Ctrl+V again (Wayland and X11) | Toast "Image and 2 objects copied to clipboard" (no redaction warning in a document without blur/pixelate redactions; with them, see 8). Each Ctrl+V inserts both, 10 px down and right of where they were, selected, one "Undo Paste 2 Objects". Pasting the same copy into another app (a chat, an email) gives the whole annotated image, as in 7 — also right after drawing an arrow (it stays selected) and pressing Ctrl+C. Shift+Ctrl+V still opens a clipboard image as a new document, and Ctrl+V in a sidebar text field pastes text. |
| 10 | On Wayland: copy objects (as in 9), close Annota's window, start Annota again and press Ctrl+V in a new capture | The objects paste (Annota was held running while it owned the clipboard, like the image copy in 7). |
| 11 | Tray → Select, select two open documents and a recent capture, Export 3… → in "Export 3 Images" press the folder button (and, with an unwritable export folder, the banner's Choose Folder…) | GTK's folder chooser (on a sandboxed or portal-configured desktop the FileChooser portal) opens at the export folder; the chosen folder shows in the Folder row and the "→ …" lines; Export writes the three files there; the toast "3 images exported to <folder>" opens that folder with Show in Files. |
| 12 | The same in the maintainer's own theme (Everforest `gtk.css`) | The tray's Select / selection bar and the batch dialog stay usable; colours, button sizes and the "Open"/"Recent" labels fading out follow the personal theme where it overrides Annota's stylesheet (known, not a bug — see the note above). |
| 13 | Edit a saved capture and take a new one, then press Ctrl+W (Wayland and X11); repeat with the window's close button, with Ctrl+Q and with the main menu's Quit | "Save Changes?" (372 px, Cancel · Discard · Save, Save focused) over the edited capture, then over the new one ("has never been saved"), each shown behind its alert. Esc keeps everything open; Save writes like Ctrl+S and closes that document; after the last answer the window closes / Annota quits. Restarting Annota offers nothing to restore. |
| 14 | Quit Annota from GNOME Shell (right-click the dash icon → Quit) with an unsaved capture | The same "Save Changes?" as with Ctrl+Q. |
| 15 | Change an unsaved capture, wait over 30 s, then log out (or `kill -TERM` Annota) and start Annota again | No alert could be shown; "Restore Unsaved Captures?" offers the capture with that change (the 30 s autosave; changes after the last one are lost, as after any crash). |
| 16 | On Ubuntu 24.04 (libadwaita 1.5): the alert of 13 | Responses side by side (not stacked), Discard as red text on the neutral fill as in mockup 15. |
| 17 | Press Ctrl+Shift+S, type `X` in GTK's chooser with `X.annota` in that folder; the same with Share → Copy to Folder…, typing `Shot` with `Shot.png` there; then type a full name that exists | First two: the chooser asks nothing, then the alert "A file with this name already exists" naming the file with its extension (Cancel · Replace; Enter and Esc cancel, the file stays). Full name: only the chooser's own question. |
| 18 | Take a capture, draw a blur redaction while its toast is shown, press the toast's Copy and paste into another app (Wayland and X11) | "Make Redactions Solid?" with "Copy Anyway"; after either button the pasted image has the redaction (solid or blurred). |
