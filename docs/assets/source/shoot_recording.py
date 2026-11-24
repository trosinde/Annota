#!/usr/bin/env python3
"""Takes the README/docs screenshots of the welcome page and the screen-recording flow.

Real screenshots of the installed Annota (never mockups), on a live GNOME Wayland session:

    welcome      the start window with the Region, Window, Screen and Record cards
    setup        the region overlay in Video mode with the recording setup next to a selection
    countdown    "Recording starts in N s"
    control      the Recording control window
    paused       the same window, paused
    export       the Export Recording window
    saved        the Export Recording window after Export ("Saved")

Each shot is a full-screen grab through the Screenshot portal (non-interactive), saved raw into
the work folder; `crop` then cuts each one down to the Annota UI (see README-recording.md).

Behind Annota a full-screen backdrop window shows base.html (the neutral demo page also used
for the editor shots), so no real desktop content ends up in a raw grab. Annota runs with
throwaway settings and data folders (GSETTINGS_BACKEND=keyfile, XDG_*_HOME in the work folder):
the user's settings, library and ~/Videos are never touched; the few seconds of recording and
the exported video land in the work folder, which `shoot` deletes again except for the raw
grabs.

Reuses tools/live-recording.py (AT-SPI helpers, GNOME's chooser) and tools/portal-input.py
(synthetic pointer/keys through the RemoteDesktop portal; needs its stored token).

    python3 docs/assets/source/shoot_recording.py shoot --work /tmp/shots
    python3 docs/assets/source/shoot_recording.py crop  --work /tmp/shots --out docs/assets
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from urllib.parse import unquote, urlparse

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
sys.path.insert(0, os.path.join(REPO, "tools"))
lr = __import__("live-recording")
lwc, lrp = lr.lwc, lr.lrp
import gi  # noqa: E402

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

wait, find, walk, name, role, press = lr.wait, lr.find, lr.walk, lr.name, lr.role, lr.press

# Region selection in logical px (top-left, bottom-right) over the backdrop's page content.
# The selection holds the page title, the cards and the buttons below them; its size label sits
# in the free band between the top bar and the title (PAGE_CSS makes that band).
SEL_FROM, SEL_TO = (250, 110), (1110, 815)
# base.html is rendered at this device scale, so its text is about as big as Annota's UI.
PAGE_SCALE = 1.25
# Room above the page title for the selection's size label (base.html itself has 28 px).
PAGE_CSS = "main { padding-top: 72px; }"

# Behind the window shots: GNOME's light neutral grey.
PLAIN = "#deddda"

BACKDROP = r"""
import sys, threading
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk
GLib.set_prgname("annotabackdrop")
Gtk.init()
w = Gtk.Window(title="Annota backdrop")
css = Gtk.CssProvider()
css.load_from_string("window, .plain { background: %s; }" % sys.argv[2])
Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 800)
stack = Gtk.Stack()
pic = Gtk.Picture.new_for_filename(sys.argv[1])
pic.set_content_fit(Gtk.ContentFit.COVER)
stack.add_named(pic, "page")
plain = Gtk.Box()
plain.add_css_class("plain")
stack.add_named(plain, "plain")
w.set_child(stack)
w.fullscreen()
w.present()
loop = GLib.MainLoop()
def ready():
    if w.get_surface() is None or not w.is_fullscreen():
        return True
    print("ready %d %d %.4f" % (w.get_width(), w.get_height(), w.get_surface().get_scale()),
          flush=True)
    return False
GLib.timeout_add(300, ready)
def cmd(line):
    if line in ("page", "plain"):
        stack.set_visible_child_name(line)
    else:
        loop.quit()
    return False
def reader():
    for line in sys.stdin:
        GLib.idle_add(cmd, line.strip())
    GLib.idle_add(cmd, "quit")
threading.Thread(target=reader, daemon=True).start()
w.connect("close-request", lambda *_: (loop.quit(), False)[1])
loop.run()
"""


def render_backdrop(work, width, height):
    """base.html rendered at the screen's physical size, at PAGE_SCALE."""
    from playwright.sync_api import sync_playwright

    out = os.path.join(work, "backdrop.png")
    vw, vh = round(width / PAGE_SCALE), round(height / PAGE_SCALE)
    with sync_playwright() as p:
        b = p.chromium.launch()
        pg = b.new_page(viewport={"width": vw, "height": vh}, device_scale_factor=PAGE_SCALE)
        pg.goto("file://" + os.path.join(HERE, "base.html"))
        pg.add_style_tag(content=PAGE_CSS)
        pg.wait_for_timeout(300)
        pg.screenshot(path=out)
        b.close()
    return out


class Backdrop:
    def __init__(self, png, colour=PLAIN):
        self.proc = subprocess.Popen([sys.executable, "-I", "-c", BACKDROP, png, colour],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        line = self.proc.stdout.readline().split()
        if not line or line[0] != "ready":
            self.close()
            raise lr.Fail("backdrop window did not start")
        self.width, self.height, self.scale = int(line[1]), int(line[2]), float(line[3])
        time.sleep(1.0)

    def show(self, what):
        """'page' (base.html) or 'plain' (one neutral colour behind window shots)."""
        self.proc.stdin.write(what + "\n")
        self.proc.stdin.flush()
        time.sleep(0.6)

    def close(self):
        if self.proc.poll() is None:
            try:
                self.proc.stdin.close()
                self.proc.wait(3)
            except (subprocess.TimeoutExpired, OSError):
                self.proc.kill()
                self.proc.wait(5)


def grab(path):
    """One full-screen grab through the Screenshot portal (no dialog), moved to `path`."""
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    token = "annotashot%d" % int(time.monotonic() * 1000)
    unique = bus.get_unique_name().lstrip(":").replace(".", "_")
    handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (unique, token)
    loop, answer = GLib.MainLoop(), []

    def on_response(_c, _s, _p, _i, _sig, params):
        answer.append(params.unpack())
        loop.quit()

    sub = bus.signal_subscribe(None, "org.freedesktop.portal.Request", "Response", handle, None,
                               Gio.DBusSignalFlags.NONE, on_response)
    bus.call_sync("org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                  "org.freedesktop.portal.Screenshot", "Screenshot",
                  GLib.Variant("(sa{sv})", ("", {"handle_token": GLib.Variant("s", token),
                                                 "interactive": GLib.Variant("b", False)})),
                  GLib.VariantType("(o)"), Gio.DBusCallFlags.NONE, 5000, None)
    GLib.timeout_add_seconds(15, loop.quit)
    loop.run()
    bus.signal_unsubscribe(sub)
    if not answer or answer[0][0] != 0:
        raise lr.Fail("screenshot portal answered %r" % (answer,))
    src = unquote(urlparse(answer[0][1]["uri"]).path)
    shutil.move(src, path)
    print("grabbed", os.path.basename(path), flush=True)
    return path


class Facts(dict):
    def __init__(self):
        super().__init__()
        self.t0 = time.monotonic()

    def event(self, what, **extra):
        t = round(time.monotonic() - self.t0, 2)
        print("%6.2f %s %s" % (t, what, extra or ""), flush=True)
        return t


class Ctx:
    dev = None


def shoot(args):
    if subprocess.run(["pgrep", "-x", "annota"], capture_output=True).returncode == 0:
        print("Annota is running: quit it first", file=sys.stderr)
        return 2
    work = os.path.abspath(args.work)
    raw = os.path.join(work, "raw")
    os.makedirs(raw, exist_ok=True)
    os.environ["XDG_DATA_DIRS"] = "/usr/share"  # the release's schema, not a stale /usr/local one
    real_home = os.environ["HOME"]
    probe = grab(os.path.join(work, "probe.png"))
    from PIL import Image

    sw, sh = Image.open(probe).size
    os.remove(probe)
    backdrop = Backdrop(render_backdrop(work, sw, sh))
    with open(os.path.join(raw, "meta.json"), "w") as f:
        json.dump({"screen": [sw, sh], "logical": [backdrop.width, backdrop.height],
                   "scale": sw / backdrop.width}, f)
    ctx = Ctx()
    ctx.binary = args.annota
    ctx.inp = lwc.portal_input.Input().__enter__()
    facts = Facts()
    annota = None
    try:
        pointer = lrp.Pointer(ctx.inp, backdrop.width)
        pointer.to(backdrop.width - 40, backdrop.height - 40)
        grab(os.path.join(raw, "reference-page.png"))
        backdrop.show("plain")
        grab(os.path.join(raw, "reference.png"))

        # The start window.
        if not args.only_setup:
            run_dir = os.path.join(work, "welcome")
            annota = lr.Annota(ctx, run_dir, [], {"record-countdown": "3"})
            app = annota.app()
            lr.dismiss_shortcut_dialogs(facts)
            wait(lambda: find(app, lambda n: role(n) == "button"
                              and n.get_description() == "Capture a window"), 10,
                 "the welcome page")
            time.sleep(1.5)
            grab(os.path.join(raw, "welcome.png"))
            annota.stop()
            annota = None
        backdrop.show("page")
        time.sleep(1.0)

        # The recording flow.
        run_dir = os.path.join(work, "record")
        # HOME points into the run folder, so the Export window's Folder row reads
        # "~/Videos/Annota" instead of a real home path.
        home = os.path.join(run_dir, "home")
        os.environ["HOME"] = home
        videos = "'%s'" % os.path.join(home, "Videos", "Annota")
        annota = lr.Annota(ctx, run_dir, ["--record", "region"],
                           {"record-countdown": "3", "record-microphone": "true",
                            "record-system-audio": "true", "include-pointer": "true",
                            "record-folder": videos, "record-export-folder": videos})
        app = annota.app()
        lr.dismiss_shortcut_dialogs(facts)
        wait(lambda: lr.button(app, "Start Recording"), 20, "the recording setup")
        time.sleep(0.8)
        pointer = lrp.Pointer(ctx.inp, backdrop.width)
        pointer.to(*SEL_FROM)
        pointer.button(True)
        try:
            pointer.to(*SEL_TO, steps=12)
        finally:
            pointer.button(False)
        time.sleep(0.5)
        pointer.to(backdrop.width - 40, backdrop.height - 40)
        time.sleep(1.0)
        grab(os.path.join(raw, "setup.png"))
        if args.only_setup:
            return 0  # nothing recorded: Annota is quit in `finally`
        lr.press_button(app, "Start Recording")
        wait(lwc.chooser, 15, "GNOME's chooser")
        lr.choose_first(ctx.inp)
        wait(lambda: lr.label_text(app, lambda t: t.startswith("Recording starts in")), 15,
             "the countdown")
        grab(os.path.join(raw, "countdown.png"))
        ctrl = wait(lambda: lr.control(app), 15, "the recording control")
        time.sleep(1.0)
        # The first recorded second shows the page (the Export window's preview frame), the
        # window shots from here on a plain background.
        backdrop.show("plain")
        time.sleep(0.5)
        grab(os.path.join(raw, "control.png"))
        lr.press_button(ctrl, "Pause")
        time.sleep(1.0)
        grab(os.path.join(raw, "paused.png"))
        lr.press_button(ctrl, "Resume")
        time.sleep(1.0)
        lr.press_button(ctrl, "Stop")
        win = wait(lambda: lr.frame_named(app, "Export Recording"), 15, "Export Recording")
        time.sleep(1.5)
        grab(os.path.join(raw, "export.png"))
        lr.press_button(win, "Export")
        wait(lambda: lr.label_text(win, lambda t: t == "Saved"), 120, "'Saved'")
        time.sleep(0.8)
        grab(os.path.join(raw, "saved.png"))
        lr.press_button(win, "Close")
        time.sleep(0.5)
    finally:
        if annota:
            annota.stop()
        os.environ["HOME"] = real_home
        backdrop.close()
        try:
            ctx.inp.__exit__(None, None, None)
        except GLib.Error:
            pass
        # Recordings, exports and settings of the run: only the raw grabs stay.
        for d in ("welcome", "record"):
            shutil.rmtree(os.path.join(work, d), ignore_errors=True)
    return 0


# Crop boxes in fractions of the screen are not stable across monitors; instead every window
# shot is cut to the area where it differs from the backdrop-only reference grab.
def diff_box(img, ref, threshold=24, skip_top=0):
    from PIL import ImageChops

    d = ImageChops.difference(img.convert("RGB"), ref.convert("RGB")).convert("L")
    d = d.point(lambda v: 255 if v > threshold else 0)
    if skip_top:
        d.paste(0, (0, 0, d.width, skip_top))
    return d.getbbox()


def crop(args):
    from PIL import Image

    raw = os.path.join(os.path.abspath(args.work), "raw")
    ref = Image.open(os.path.join(raw, "reference.png"))
    with open(os.path.join(raw, "meta.json")) as f:
        meta = json.load(f)
    os.makedirs(args.out, exist_ok=True)
    for shot in args.shots:
        img = Image.open(os.path.join(raw, shot + ".png"))
        box = setup_box(meta["scale"]) if shot == "setup" else None
        if box is None:
            box = diff_box(img, ref, skip_top=args.skip_top)
            pad = args.pad
            box = (max(0, box[0] - pad), max(0, box[1] - pad),
                   min(img.width, box[2] + pad), min(img.height, box[3] + pad))
        out = os.path.join(args.out, args.prefix + shot + ".png")
        img.crop(box).convert("RGB").save(out, optimize=True)
        print(out, box, img.crop(box).size)
    return 0


# Fixed crop for the full-screen overlay shot: the selection with its size label above it and
# the recording setup to its right (physical px; `scale` is the screen's, from meta.json).
# Window shots use diff_box instead.
def setup_box(scale):
    # Top: just below the page's top bar (56 logical px), so the bar is not cut in half.
    return (round(SEL_FROM[0] * scale) - 40, round(57 * scale),
            round(SEL_TO[0] * scale) + 560, round(SEL_TO[1] * scale) + 50)


def main():
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("shoot")
    s.add_argument("--work", required=True)
    s.add_argument("--annota", default="/usr/bin/annota")
    s.add_argument("--only-setup", action="store_true",
                   help="only the recording setup shot (no welcome page, nothing recorded)")
    c = sub.add_parser("crop")
    c.add_argument("--work", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--prefix", default="")
    c.add_argument("--pad", type=int, default=0)
    c.add_argument("--skip-top", type=int, default=0)
    c.add_argument("shots", nargs="*", default=["welcome", "setup", "control", "paused",
                                                  "export", "saved"])
    a = p.parse_args()
    return shoot(a) if a.cmd == "shoot" else crop(a)


if __name__ == "__main__":
    sys.exit(main())
