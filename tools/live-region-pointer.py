#!/usr/bin/env python3
"""Live check of the drawn pointer in delayed region and in screen captures on a real GNOME session.

fix-region-pointer-inclusion, live check 8 (manual-checks.md): with "Include Pointer" on and a
toolbar delay, the arrow is drawn where the pointer stood when the countdown ended. The position
after the countdown comes from `overlay::probe_pointer`, whose windows GNOME Shell animates in;
only a real compositor can show whether the probe reports the real position, so this runs
against the logged-in session, with no human in the loop:

- the pointer and keys through tools/portal-input.py (RemoteDesktop portal, restore token),
- Annota's overlay toolbar (Delay, Capture) through AT-SPI,
- the image from the editor: Ctrl+S into a private folder, then `original.png` of the .annota.

A magenta, maximised target window fills the screen, so every non-magenta pixel in the image is
the drawn arrow; the check compares the arrow's top-left pixel with the expected hot spot. Screen
mode probes the pointer the same way (with nothing to select), so it is checked too: there the
image also holds the top bar, and only a window around the expected hot spot is searched.

Needs: a logged-in GNOME Wayland session with a single monitor, the gresource of the binary
under test installed (its PKGDATADIR), and the RemoteDesktop token (`tools/portal-input.py
check` once). Do not touch keyboard or mouse while it runs.

    tools/live-region-pointer.py --annota _build/src/annota --runs 3

Every run uses fresh, temporary settings and data directories (GSETTINGS_BACKEND=keyfile), so the
user's own Annota settings, library and recovery data are never touched. Annota and the target
window are stopped on every way out.
"""

import argparse
import glob
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
lwc = __import__("live-window-capture")
from gi.repository import Atspi, GdkPixbuf  # noqa: E402

Fail = lwc.Fail
BTN_LEFT = 0x110
# Selection and pointer positions, logical px from the top-left of the screen.
SELECT_FROM, SELECT_TO = (1000, 400), (1600, 800)
OUTSIDE, INSIDE = (1800, 600), (1200, 500)
# How far the arrow's top-left pixel may lie from the expected hot spot (antialiasing).
TOLERANCE = 3


class Region:
    """Annota in region mode, include pointer on, with private settings and data."""

    def __init__(self, binary, mode="region"):
        self.home = tempfile.mkdtemp(prefix="annota-live-region-")
        try:
            self._start(binary, mode)
        except BaseException:
            # No Region object reaches the caller's cleanup, so remove the folder here.
            shutil.rmtree(self.home, ignore_errors=True)
            raise

    def _start(self, binary, mode):
        self.save = os.path.join(self.home, "saved")
        conf = os.path.join(self.home, "config", "glib-2.0", "settings")
        os.makedirs(conf)
        with open(os.path.join(conf, "keyfile"), "w") as f:
            f.write("[%s]\nafter-capture='editor'\ncapture-delay=0\ninclude-pointer=true\n"
                    "save-folder='%s'\n" % (lwc.SETTINGS_GROUP, self.save))
        env = dict(os.environ, GSETTINGS_BACKEND="keyfile", LANGUAGE="en", LC_ALL="C.UTF-8",
                   XDG_CONFIG_HOME=os.path.join(self.home, "config"),
                   XDG_DATA_HOME=os.path.join(self.home, "data"),
                   XDG_STATE_HOME=os.path.join(self.home, "state"),
                   XDG_CACHE_HOME=os.path.join(self.home, "cache"))
        self.proc = subprocess.Popen([binary, "--capture", mode], env=env,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def app(self):
        return lwc.wait(lambda: lwc.atspi_app(self.proc.pid), 10, "Annota on AT-SPI")

    def stop(self):
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        shutil.rmtree(self.home, ignore_errors=True)


class Pointer:
    """Tracks the pointer from the top-right corner (not GNOME's hot corner) by relative moves."""

    def __init__(self, inp, width):
        self.inp = inp
        inp.move(100000, -100000)
        time.sleep(0.1)
        self.x, self.y = width - 1.0, 0.0

    def to(self, x, y, steps=6):
        dx, dy = (x - self.x) / steps, (y - self.y) / steps
        for _ in range(steps):
            self.inp.move(dx, dy)
            time.sleep(0.02)
        self.x, self.y = x, y
        time.sleep(0.15)

    def button(self, down):
        self.inp._notify("NotifyPointerButton", "(oa{sv}iu)", BTN_LEFT, 1 if down else 0)
        time.sleep(0.05)


def labels(app):
    return [lwc.name(n) for n in lwc.walk(app) if lwc.role(n) == "label" and lwc.name(n)]


def saved_image(region, app, inp):
    """Waits for the editor, saves with Ctrl+S, returns the capture inside the .annota file."""
    lwc.wait(lambda: any(lwc.has_state(app.get_child_at_index(i), Atspi.StateType.ACTIVE)
                         and lwc.name(app.get_child_at_index(i)).startswith("Annota ")
                         for i in range(app.get_child_count())), 15, "the editor")
    time.sleep(0.5)
    sym = lwc.portal_input.keysym
    inp._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym("Control_L"), 1)
    inp.keys("s")
    inp._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym("Control_L"), 0)
    path = lwc.wait(lambda: glob.glob(os.path.join(region.save, "*.annota")), 10, "a saved file")
    time.sleep(0.5)
    with zipfile.ZipFile(path[0]) as archive:
        loader = GdkPixbuf.PixbufLoader.new_with_type("png")
        loader.write(archive.read("original.png"))
        loader.close()
        return loader.get_pixbuf()


def arrow_box(pixbuf, area=None):
    """Bounding box (x0, y0, x1, y1) of the non-magenta pixels in `area` (x0, y0, x1, y1;
    default the whole image), or None."""
    raw, stride, step = pixbuf.get_pixels(), pixbuf.get_rowstride(), pixbuf.get_n_channels()
    x0, y0, x1, y1 = area or (0, 0, pixbuf.get_width(), pixbuf.get_height())
    box = None
    for y in range(max(0, y0), min(pixbuf.get_height(), y1)):
        for x in range(max(0, x0), min(pixbuf.get_width(), x1)):
            p = y * stride + x * step
            if (raw[p], raw[p + 1], raw[p + 2]) != lwc.MAGENTA:
                box = (x, y, x, y) if box is None else (
                    min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y))
    return box


def delayed_capture(binary, inp, start, keep_dir):
    """Selects, sets the 3 s delay, puts the pointer at `start`, captures, moves the pointer to
    INSIDE during the countdown and checks the arrow in the image."""
    target = region = None
    try:
        target = lwc.Target(maximized=True)
        region = Region(binary)
        app = region.app()
        lwc.wait(lambda: "Drag to select" in labels(app), 20, "the overlay")
        time.sleep(0.8)
        pointer = Pointer(inp, target.width)
        pointer.to(*SELECT_FROM)
        pointer.button(True)
        try:
            pointer.to(*SELECT_TO, steps=10)
        finally:
            pointer.button(False)  # never leave the session's button held
        time.sleep(0.3)
        delay = lwc.find(app, lambda n: lwc.role(n) == "toggle button"
                         and n.get_description() == "Delay")
        if delay is None or not lwc.press(delay):
            raise Fail("no Delay button on the overlay toolbar")
        time.sleep(0.6)
        # Off, 1, 2, 3, 5, 10 s, custom: the fourth item is 3 s.
        items = [n for n in lwc.walk(app) if lwc.role(n) == "radio menu item"]
        if len(items) != 7:
            raise Fail("delay menu has %d items, expected 7" % len(items))
        items[3].get_action_iface().do_action(0)
        time.sleep(0.4)
        if "3 s" not in [lwc.name(n) for n in lwc.walk(app) if lwc.role(n) == "toggle button"]:
            raise Fail("the delay is not 3 s")
        pointer.to(*start)
        capture = lwc.find(app, lambda n: lwc.role(n) == "button" and lwc.name(n) == "Capture")
        if capture is None or not lwc.press(capture):
            raise Fail("no Capture button")
        time.sleep(0.8)
        pointer.to(*INSIDE)  # during the countdown; it stays there until the image is taken
        image = saved_image(region, app, inp)
        if keep_dir:
            image.savev(os.path.join(keep_dir, "delay-%d.png" % int(time.time() * 1000)),
                        "png", [], [])
        scale = target.scale
        want = ((INSIDE[0] - SELECT_FROM[0]) * scale, (INSIDE[1] - SELECT_FROM[1]) * scale)
        box = arrow_box(image)
        if box is None:
            raise Fail("no arrow in the image, expected its hot spot at %.0f,%.0f" % want)
        if abs(box[0] - want[0]) > TOLERANCE or abs(box[1] - want[1]) > TOLERANCE:
            raise Fail("arrow at %d,%d, expected its hot spot at %.0f,%.0f"
                       % (box[0], box[1], want[0], want[1]))
        return "arrow at %d,%d (expected %.0f,%.0f), image %d × %d" % (
            box[0], box[1], want[0], want[1], image.get_width(), image.get_height())
    finally:
        if region is not None:
            region.stop()
        if target is not None:
            target.close()


def screen_capture(binary, inp, _start, keep_dir):
    """Puts the pointer at INSIDE, captures the screen, checks the arrow in the image."""
    target = region = None
    try:
        target = lwc.Target(maximized=True)
        pointer = Pointer(inp, target.width)
        pointer.to(*INSIDE)
        region = Region(binary, "screen")
        app = region.app()
        image = saved_image(region, app, inp)
        if keep_dir:
            image.savev(os.path.join(keep_dir, "screen-%d.png" % int(time.time() * 1000)),
                        "png", [], [])
        want = (INSIDE[0] * target.scale, INSIDE[1] * target.scale)
        margin = 60
        box = arrow_box(image, (int(want[0]) - margin, int(want[1]) - margin,
                                int(want[0]) + margin, int(want[1]) + margin))
        if box is None:
            raise Fail("no arrow near %.0f,%.0f" % want)
        if abs(box[0] - want[0]) > TOLERANCE or abs(box[1] - want[1]) > TOLERANCE:
            raise Fail("arrow at %d,%d, expected its hot spot at %.0f,%.0f"
                       % (box[0], box[1], want[0], want[1]))
        return "arrow at %d,%d (expected %.0f,%.0f), image %d × %d" % (
            box[0], box[1], want[0], want[1], image.get_width(), image.get_height())
    finally:
        if region is not None:
            region.stop()
        if target is not None:
            target.close()


SCENARIOS = [
    ("pointer moved into the selection during the countdown", delayed_capture, OUTSIDE),
    ("pointer inside the selection throughout", delayed_capture, INSIDE),
    ("screen capture, pointer at rest", screen_capture, INSIDE),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--annota", default=shutil.which("annota") or "annota")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--keep", default="", help="directory to keep the captured images in")
    args = parser.parse_args()
    print("Annota:", subprocess.run([args.annota, "--version"], capture_output=True,
                                    text=True).stdout.strip())
    failed = False
    with lwc.portal_input.Input() as inp:
        for run in range(1, args.runs + 1):
            for title, scenario, start in SCENARIOS:
                try:
                    outcome = ("pass", scenario(args.annota, inp, start, args.keep))
                except Exception as err:  # noqa: BLE001 - report and go on
                    outcome = ("FAIL", "%s: %s" % (type(err).__name__, err))
                failed |= outcome[0] != "pass"
                print("run %d  %-4s  %-55s %s" % (run, outcome[0], title, outcome[1]),
                      flush=True)
                time.sleep(1)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
