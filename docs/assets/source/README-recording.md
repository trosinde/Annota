# Rebuilding the welcome and screen-recording screenshots

`welcome.png` and `recording-*.png` in `docs/assets/` are real screenshots of the installed
Annota, taken on a live GNOME Wayland session by `shoot_recording.py`. They are not mockups.

| File | Shows |
|---|---|
| `welcome.png` | the start window with the Region, Window, Screen and Record cards |
| `recording-setup.png` | the region overlay in Video mode, the recording setup next to a selection |
| `recording-control.png` | the Recording control while recording |
| `recording-paused.png` | the same control, paused |
| `recording-export.png` | the Export Recording window |
| `recording-saved.png` | the Export Recording window after Export ("Saved") |

## Needs

- A logged-in GNOME Wayland session with AT-SPI. Do not touch the keyboard or mouse during the run.
- The Annota build you want to show, installed (default `/usr/bin/annota`); Annota must **not**
  be running (D-Bus single instance).
- The RemoteDesktop token of `tools/portal-input.py` (run `tools/portal-input.py keys space`
  once and allow it), used for the pointer drag and GNOME's screen-sharing dialog.
- Python 3 with PyGObject, Pillow and Playwright (`playwright install chromium`).

## Steps

To retake only `recording-setup.png` (no welcome page, nothing recorded), add `--only-setup`
to `shoot` and pass `setup` to `crop`.

```bash
W=$(mktemp -d)
python3 docs/assets/source/shoot_recording.py shoot --work "$W"
python3 docs/assets/source/shoot_recording.py crop --work "$W" --out "$W/out" --pad 40
# look at every image in $W/out, then:
cp "$W/out/welcome.png" docs/assets/welcome.png
for n in setup control paused export saved; do
  cp "$W/out/$n.png" "docs/assets/recording-$n.png"
done
rm -rf "$W"
```

`shoot` does this:

1. Renders `base.html` at the screen's size and shows it in a full-screen backdrop window, so
   no real desktop content is ever behind Annota.
2. Takes every shot as a full-screen grab through the Screenshot portal (non-interactive).
3. Starts Annota with throwaway settings and data folders (`GSETTINGS_BACKEND=keyfile`,
   `XDG_*_HOME` and, for the recording, `HOME` inside the work folder), so the user's settings,
   library and `~/Videos` stay untouched and the Export window shows `~/Videos/Annota`.
   `XDG_DATA_DIRS=/usr/share` makes the release find its own GSettings schema.
4. Welcome page: `annota`, one grab over a plain grey backdrop.
5. Recording: `annota --record region` (countdown 3 s, microphone and system audio on), drags a
   selection over the page, grabs the setup, presses Start Recording, picks the first screen in
   GNOME's dialog, then grabs the countdown, the control, the paused control, the Export
   Recording window and, after Export, its Saved state. The recording lasts about 4 seconds.
6. Quits Annota and deletes the run folders with the recording and the exported video; only the
   raw grabs stay in `<work>/raw`.

`crop` cuts each window shot to where it differs from a backdrop-only reference grab (plus
`--pad` px for the shadow). The setup shot uses a fixed box around the selection and the setup
panel, computed from the selection and the screen scale in `raw/meta.json`.

Framing of the setup shot: the backdrop page gets `main { padding-top: 72px; }` (`PAGE_CSS`), so
the selection's orange size label sits in a free band between the page's top bar and its title.
The selection (`SEL_FROM`/`SEL_TO`) holds the title, the cards and the Back Up Now/Save Changes
buttons. The crop starts just below the top bar and ends 50 px below the selection, so nothing
in it is cut in half.

The images are at the screen's physical size; the 2026-11-24 set came from a 3840×2160 screen
at 125 % scale in the dark style. On another screen, check the setup crop and adjust
`SEL_FROM`/`SEL_TO` or `setup_box()`.

No GIF: on a 4K screen the setup, countdown, control and Export window sit far apart, and a
common frame scaled down to ≤ 960 px makes their text unreadable.
