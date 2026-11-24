#!/usr/bin/env python3
"""Automated live tests of window capture on a real GNOME session (fix-window-capture-screencast, D9).

Drives Annota and GNOME's "Share Screen" window chooser with no human in the loop:
- Annota's own widgets (Window card, waiting indicator's Cancel) through AT-SPI actions.
- GNOME's chooser: rows through synthetic keys from tools/portal-input.py (RemoteDesktop portal,
  restore token), the "Share"/"Cancel" buttons through AT-SPI.
- Outcomes from Annota's accessible tree (toast, waiting indicator, countdown), from the
  portal's session objects on D-Bus, and from the clipboard image (after-capture "both"):
  its size, and whether the pointer is in it (presence only, D5).

Needs: a logged-in GNOME Wayland session, Annota installed from the current build
(`sudo meson install -C _build`), `wl-paste`, and the RemoteDesktop token
(`tools/portal-input.py check` once). Do not touch keyboard or mouse while it runs.

    tools/live-window-capture.py                 # every scenario, twice
    tools/live-window-capture.py --runs 1 -k delay
    tools/live-window-capture.py --annota /path/to/annota
    tools/live-window-capture.py -k whole --save /tmp/shots   # keep each captured image as PNG

Every run uses fresh, temporary settings and data directories (GSETTINGS_BACKEND=keyfile), so the
user's own Annota settings, library and recovery data are never touched. The clipboard image is
read into memory only, unless `--save DIR` asks for each captured image to be written there.

The target window is magenta with a red, green, blue and yellow square in its top-left,
top-right, bottom-left and bottom-right corner, so an image that holds only part of the window
fails (design.md D3, amendment of 2026-11-23). Every target window and Annota process the suite
starts is ended when it exits, also on a failure or Ctrl+C (`pgrep -f livetarget` and
`pgrep -x annota` then find nothing of it).
"""

import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET

import gi

gi.require_version("Atspi", "2.0")
gi.require_version("GdkPixbuf", "2.0")
gi.require_version("Gio", "2.0")
from gi.repository import Atspi, GdkPixbuf, Gio, GLib  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
portal_input = __import__("portal-input")

TARGET_TITLE = "Annota live target"
TARGET_W, TARGET_H = 640, 400
MAGENTA = (255, 0, 255)
# The target's corner squares: side in logical px, colours in reading order.
SQUARE = 48
CORNERS = {"top-left": (255, 0, 0), "top-right": (0, 255, 0),
           "bottom-left": (0, 0, 255), "bottom-right": (255, 255, 0)}
SETTINGS_GROUP = "io/github/trosinde/Annota"


class Fail(Exception):
    pass


# ---------------------------------------------------------------------------------------------
# Target window: a magenta GTK window with four coloured corner squares, in its own process.
# Undecorated by default; "deco" gives it a header bar (client-side decorations and shadow).

TARGET_SCRIPT = r"""
import sys, gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk, GLib
GLib.set_prgname("livetarget")
Gtk.init()
css = Gtk.CssProvider()
css.load_from_string("window, window * { background: #ff00ff; }"
                     " .tl { background: #ff0000; } .tr { background: #00ff00; }"
                     " .bl { background: #0000ff; } .br { background: #ffff00; }")
Gtk.StyleContext.add_provider_for_display(
    __import__("gi.repository.Gdk", fromlist=["Gdk"]).Display.get_default(), css, 800)
w = Gtk.Window(title=sys.argv[1])
if sys.argv[5] == "deco":
    w.set_titlebar(Gtk.HeaderBar())
else:
    w.set_decorated(False)
w.set_default_size(int(sys.argv[2]), int(sys.argv[3]))
overlay = Gtk.Overlay()
overlay.set_child(Gtk.Box())
for cls, h, v in (("tl", 1, 1), ("tr", 2, 1), ("bl", 1, 2), ("br", 2, 2)):
    square = Gtk.Box(css_classes=[cls], halign=Gtk.Align(h), valign=Gtk.Align(v))
    square.set_size_request(int(sys.argv[6]), int(sys.argv[6]))
    overlay.add_overlay(square)
w.set_child(overlay)
if sys.argv[4] == "max":
    w.maximize()
w.present()
def report():
    s = w.get_surface()
    if s is None or w.get_width() == 0 or (sys.argv[4] == "max" and not w.is_maximized()):
        return True
    print("ready %d %d %.4f" % (w.get_width(), w.get_height(), s.get_scale()), flush=True)
    return False
GLib.timeout_add(300, report)
w.connect("close-request", lambda *_: (loop.quit(), False)[1])
loop = GLib.MainLoop()
loop.run()
"""


# Every process the suite starts, so that main() can end what a failed scenario left behind.
STARTED = []


def end(proc):
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(5)


class Target:
    def __init__(self, maximized=False, decorated=False):
        self.decorated = decorated
        self.proc = subprocess.Popen(
            [sys.executable, "-I", "-c", TARGET_SCRIPT, TARGET_TITLE, str(TARGET_W),
             str(TARGET_H), "max" if maximized else "normal",
             "deco" if decorated else "plain", str(SQUARE)],
            stdout=subprocess.PIPE, text=True)
        STARTED.append(self.proc)
        line = self.proc.stdout.readline().split()
        if not line or line[0] != "ready":
            end(self.proc)
            raise Fail("target window did not start")
        time.sleep(0.5)
        self.width, self.height, self.scale = int(line[1]), int(line[2]), float(line[3])

    def close(self):
        end(self.proc)


# ---------------------------------------------------------------------------------------------
# Annota under test, with private settings and data.

class Annota:
    def __init__(self, binary, args=(), delay=0, pointer=False):
        self.home = tempfile.mkdtemp(prefix="annota-live-")
        conf = os.path.join(self.home, "config", "glib-2.0", "settings")
        os.makedirs(conf)
        with open(os.path.join(conf, "keyfile"), "w") as f:
            f.write("[%s]\nafter-capture='both'\ncapture-delay=%d\ninclude-pointer=%s\n"
                    % (SETTINGS_GROUP, delay, "true" if pointer else "false"))
        env = dict(os.environ, GSETTINGS_BACKEND="keyfile", LANGUAGE="en", LC_ALL="C.UTF-8",
                   XDG_CONFIG_HOME=os.path.join(self.home, "config"),
                   XDG_DATA_HOME=os.path.join(self.home, "data"),
                   XDG_STATE_HOME=os.path.join(self.home, "state"),
                   XDG_CACHE_HOME=os.path.join(self.home, "cache"))
        self.env = env
        self.proc = subprocess.Popen([binary, *args], env=env, stdout=subprocess.DEVNULL,
                                     stderr=subprocess.DEVNULL)
        STARTED.append(self.proc)

    def app(self, timeout=10):
        return wait(lambda: atspi_app(self.proc.pid), timeout, "Annota on the accessibility bus")

    def stop(self):
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        shutil.rmtree(self.home, ignore_errors=True)


# ---------------------------------------------------------------------------------------------
# AT-SPI helpers.

def wait(probe, timeout, what, interval=0.25):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = probe()
        if value:
            return value
        time.sleep(interval)
    raise Fail("timed out waiting for " + what)


def absent(probe, timeout):
    """True when probe() stays falsy for the whole timeout."""
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if probe():
            return False
        time.sleep(0.25)
    return True


def apps():
    desktop = Atspi.get_desktop(0)
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        if app is not None:
            yield app


def atspi_app(pid):
    for app in apps():
        try:
            if app.get_process_id() == pid:
                return app
        except GLib.Error:
            pass
    return None


def walk(node, depth=0):
    if node is None or depth > 60:
        return
    yield node
    try:
        n = node.get_child_count()
    except GLib.Error:
        return
    for i in range(n):
        try:
            child = node.get_child_at_index(i)
        except GLib.Error:
            continue
        yield from walk(child, depth + 1)


def name(node):
    try:
        return node.get_name() or ""
    except GLib.Error:
        return ""


def role(node):
    try:
        return node.get_role_name()
    except GLib.Error:
        return ""


def has_state(node, state):
    try:
        return node.get_state_set().contains(state)
    except GLib.Error:
        return False


def press(node):
    """Runs the first action of the node or of its nearest actionable ancestor."""
    while node is not None:
        action = node.get_action_iface()
        if action and action.get_n_actions() and role(node) not in ("label", "frame", "window"):
            action.do_action(0)
            return True
        node = node.get_parent()
    return False


def find(root, predicate):
    for node in walk(root):
        if predicate(node):
            return node
    return None


def chooser():
    for app in apps():
        if name(app) == "xdg-desktop-portal-gnome":
            for i in range(app.get_child_count()):
                frame = app.get_child_at_index(i)
                if frame is not None and name(frame) == "Share Screen":
                    return frame
    return None


def label_starting(app, prefix):
    node = find(app, lambda n: role(n) == "label" and name(n).startswith(prefix))
    return name(node) if node else None


def waiting_indicator(app):
    return find(app, lambda n: role(n) in ("alert", "label") and name(n) == "Waiting for a window…")


def countdown(app):
    return label_starting(app, "Capturing window in")


def toast(app):
    return label_starting(app, "Window captured · ")


# ---------------------------------------------------------------------------------------------
# Chooser driving.

def chooser_rows(dialog):
    return [n for n in walk(dialog) if role(n) == "list item"]


def choose(inp, title=TARGET_TITLE, before_share=None):
    dialog = wait(chooser, 15, "GNOME's window chooser")
    time.sleep(0.4)
    rows = wait(lambda: chooser_rows(dialog), 5, "chooser rows")
    if not any(title in name(r) for r in rows):
        raise Fail("target not listed in the chooser: %s" % [name(r) for r in rows])
    inp.keys("Tab")
    for _ in range(len(rows) + 2):
        focused = [r for r in chooser_rows(dialog) if has_state(r, Atspi.StateType.FOCUSED)]
        if focused and title in name(focused[0]):
            break
        inp.keys("Down")
    else:
        raise Fail("could not focus the target row")
    inp.keys("space")
    time.sleep(0.3)
    if before_share:
        before_share()
    share = find(dialog, lambda n: role(n) == "label" and name(n) == "Share")
    if not share or not press(share):
        raise Fail("no Share button")


def close_chooser():
    dialog = wait(chooser, 15, "GNOME's window chooser")
    cancel = find(dialog, lambda n: role(n) == "label" and name(n) == "Cancel")
    if not cancel or not press(cancel):
        raise Fail("no Cancel button in the chooser")


# ---------------------------------------------------------------------------------------------
# D-Bus: does Annota still hold a portal session?

def portal_sessions(pid):
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

    def call(dest, path, iface, method, args, reply):
        return bus.call_sync(dest, path, iface, method, args, GLib.VariantType(reply),
                             Gio.DBusCallFlags.NONE, 3000, None).unpack()

    names = call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                 "ListNames", None, "(as)")[0]
    found = []
    for unique in (n for n in names if n.startswith(":")):
        try:
            owner = call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                         "GetConnectionUnixProcessID", GLib.Variant("(s)", (unique,)), "(u)")[0]
        except GLib.Error:
            continue
        if owner != pid:
            continue
        path = "/org/freedesktop/portal/desktop/session/" + unique[1:].replace(".", "_")
        try:
            xml = call("org.freedesktop.portal.Desktop", path,
                       "org.freedesktop.DBus.Introspectable", "Introspect", None, "(s)")[0]
        except GLib.Error:
            continue
        found += [path + "/" + n.get("name") for n in ET.fromstring(xml).findall("node")]
    return found


def chooser_sessions(annota):
    """Annota's portal sessions while GNOME's chooser is shown: the ScreenCast session among them.
    Annota also holds long-lived sessions (GTK's own, GlobalShortcuts) that stay open."""
    wait(chooser, 15, "GNOME's window chooser")
    return set(portal_sessions(annota.proc.pid))


def session_closes(annota, during, timeout=3):
    wait(lambda: during - set(portal_sessions(annota.proc.pid)), timeout,
         "the screen-sharing session to close")


# ---------------------------------------------------------------------------------------------
# Clipboard image (in memory only).

def clipboard_image():
    try:
        data = subprocess.run(["wl-paste", "--no-newline", "--type", "image/png"],
                              capture_output=True, timeout=10).stdout
    except subprocess.TimeoutExpired:
        return None
    if not data:
        return None
    loader = GdkPixbuf.PixbufLoader.new_with_type("png")
    loader.write(data)
    loader.close()
    return loader.get_pixbuf()


def captured_image(app, inp, text):
    """The captured image, read from the clipboard. The toast's claim must match the clipboard:
    "Copied to clipboard" only when the image is really there. Without the claim (a capture with
    no earlier key or click in Annota cannot copy on Wayland), one real key into the focused
    editor gives GTK an input serial, and the toast's Copy then copies."""
    time.sleep(0.5)
    types = subprocess.run(["wl-paste", "--list-types"], capture_output=True, text=True,
                           timeout=5).stdout.split()
    claims = "Copied to clipboard" in text
    if claims != ("image/png" in types):
        raise Fail("toast %r, but clipboard types %s" % (text, types[:3]))
    if not claims:
        wait(lambda: any(has_state(app.get_child_at_index(i), Atspi.StateType.ACTIVE)
                         for i in range(app.get_child_count())), 5, "the editor to be active")
        inp.keys("Shift_L")
        time.sleep(0.4)  # the key event has to reach GTK before Copy runs
        copy = find(app, lambda n: role(n) == "button" and name(n) == "Copy")
        if not copy or not press(copy):
            raise Fail("no Copy on the toast")
    try:
        return wait(clipboard_image, 5, "the image on the clipboard")
    except Fail:
        types = subprocess.run(["wl-paste", "--list-types"], capture_output=True, text=True,
                               timeout=5).stdout.split()
        active = [name(app.get_child_at_index(i)) for i in range(app.get_child_count())
                  if has_state(app.get_child_at_index(i), Atspi.StateType.ACTIVE)]
        raise Fail("no image on the clipboard (types %s, active %s, toast %r)"
                   % (types[:3], active, text))


def foreign_pixels(pixbuf, target):
    """Pixels inside the image that are not the target's magenta (pointer presence, D5)."""
    raw = pixbuf.get_pixels()
    stride, step = pixbuf.get_rowstride(), pixbuf.get_n_channels()
    w, h = pixbuf.get_width(), pixbuf.get_height()
    # The corner squares and their antialiased edges are the target's own, not the pointer.
    zone = (SQUARE + 8) * w / target.width
    count = 0
    for y in range(2, h - 2, 2):
        row = y * stride
        for x in range(2, w - 2, 2):
            if (x < zone or x >= w - zone) and (y < zone or y >= h - zone):
                continue
            p = row + x * step
            if (raw[p], raw[p + 1], raw[p + 2]) != MAGENTA:
                count += 1
    return count


def toast_size(text):
    # "Window captured · 800 × 500"
    w, h = text.split("·")[1].replace("×", "x").split("x")
    return int(w), int(h)


def expect_size(text, target):
    """The toast's size must be the window's size on screen in monitor pixels: its logical size
    times the monitor's scale, within 2 px (design.md D3, maintainer decision 2026-11-24). The
    window stream's buffer can be at a higher scale than the monitor (2 for a 1.25 monitor on
    GNOME 50.1); Annota scales it down, so a region capture of the window would be the same."""
    w, h = toast_size(text)
    want = round(target.width * target.scale), round(target.height * target.scale)
    if abs(w - want[0]) > 2 or abs(h - want[1]) > 2:
        raise Fail("captured %d × %d, the window is %d × %d on screen (%d × %d logical x %.2f)"
                   % (w, h, *want, target.width, target.height, target.scale))
    return w, h


def near(pixel, colour, tolerance=40):
    return all(abs(a - b) <= tolerance for a, b in zip(pixel, colour))


def colour_box(pixbuf, colour):
    """Bounding box (left, top, right, bottom) of the pixels close to colour, or None."""
    raw = pixbuf.get_pixels()
    stride, step = pixbuf.get_rowstride(), pixbuf.get_n_channels()
    box = None
    # Every second pixel: exact enough for the checks, and a 6144 × 3360 image stays quick.
    for y in range(0, pixbuf.get_height(), 2):
        row = y * stride
        for x in range(0, pixbuf.get_width(), 2):
            p = row + x * step
            if near((raw[p], raw[p + 1], raw[p + 2]), colour):
                box = (x, y, x, y) if box is None else (
                    min(box[0], x), min(box[1], y), max(box[2], x), max(box[3], y))
    return box


def expect_corners(pixbuf, target):
    """Each of the target's four corner squares is in the image, whole, at its own corner, so the
    image is the whole window and not a part of it."""
    w, h = pixbuf.get_width(), pixbuf.get_height()
    k = w / target.width
    side = SQUARE * k
    # The window's outline (and a decorated window's rounded corner) may lie between a square
    # and the image edge; the header bar lies between the top squares and the top edge.
    edge = 4 * k + 2
    for corner, colour in CORNERS.items():
        box = colour_box(pixbuf, colour)
        if box is None:
            raise Fail("the %s corner square is missing: only part of the window" % corner)
        left, top, right, bottom = box
        if abs((right - left + 1) - side) > edge or abs((bottom - top + 1) - side) > edge:
            raise Fail("the %s corner square is %d × %d, expected about %d: cut off"
                       % (corner, right - left + 1, bottom - top + 1, side))
        at_left, at_right = left <= edge, right >= w - 1 - edge
        at_bottom = bottom >= h - 1 - edge
        at_top = top <= edge or (target.decorated and top < h / 4)
        horizontal = at_left if corner.endswith("left") else at_right
        vertical = at_top if corner.startswith("top") else at_bottom
        if not (horizontal and vertical):
            raise Fail("the %s corner square is at (%d, %d)-(%d, %d) in a %d × %d image"
                       % (corner, left, top, right, bottom, w, h))
    return k


def save(ctx, pixbuf, label):
    if ctx.save:
        ctx.saved += 1
        path = os.path.join(ctx.save, "%02d-%s.png" % (ctx.saved, label))
        pixbuf.savev(path, "png", [], [])
        return path
    return None


def pointer_over_target(inp):
    # Bottom-left corner (top-left is GNOME's hot corner, which opens the overview), then up into
    # the maximised target, clear of the centred chooser.
    inp.move(-10000, 10000)
    inp.move(160, -400)


# ---------------------------------------------------------------------------------------------
# Scenarios. Each returns a short note; a Fail is a failed scenario.

def s_window_on_wayland(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary, ["--capture", "window"])
    try:
        app = annota.app()
        during = chooser_sessions(annota)
        choose(inp)
        text = wait(lambda: toast(app), 10, "the toast 'Window captured · W × H'")
        w, h = expect_size(text, target)
        image = captured_image(app, inp, text)
        if (image.get_width(), image.get_height()) != (w, h):
            raise Fail("clipboard image %d × %d, toast %d × %d"
                       % (image.get_width(), image.get_height(), w, h))
        if image.get_has_alpha() and image.get_pixels()[3] != 255:
            raise Fail("transparent margin left in the image")
        save(ctx, image, "window-on-wayland")
        expect_corners(image, target)
        session_closes(annota, during)
        return "%d × %d" % (w, h)
    finally:
        annota.stop()
        target.close()


def s_whole_window(ctx, inp):
    """A small undecorated, a small decorated and a maximised decorated window, each whole."""
    notes = []
    for label, maximized, decorated in (("small", False, False),
                                        ("small-decorated", False, True),
                                        ("maximised-decorated", True, True)):
        subprocess.run(["wl-copy", "--clear"], check=False)
        target = Target(maximized=maximized, decorated=decorated)
        try:
            annota = Annota(ctx.binary, ["--capture", "window"])
            try:
                app = annota.app()
                choose(inp)
                text = wait(lambda: toast(app), 10, "the toast 'Window captured · W × H'")
                w, h = expect_size(text, target)
                image = captured_image(app, inp, text)
                path = save(ctx, image, label)
                if (image.get_width(), image.get_height()) != (w, h):
                    raise Fail("%s: clipboard image %d × %d, toast %d × %d"
                               % (label, image.get_width(), image.get_height(), w, h))
                try:
                    k = expect_corners(image, target)
                except Fail as err:
                    raise Fail("%s: %s" % (label, err)) from None
                notes.append("%s %d × %d (x%.2f of %d × %d logical)%s"
                             % (label, w, h, k, target.width, target.height,
                                " " + path if path else ""))
            finally:
                annota.stop()
        finally:
            target.close()
        if chooser() is not None:
            close_chooser()
        time.sleep(1)
    return "; ".join(notes)


def s_welcome_page(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary)
    try:
        app = annota.app()
        card = wait(lambda: find(app, lambda n: role(n) == "button"
                                 and n.get_description() == "Capture a window"),
                    10, "the Window card")
        press(card)
        choose(inp)
        text = wait(lambda: toast(app), 10, "the toast 'Window captured · W × H'")
        expect_size(text, target)
        return text
    finally:
        annota.stop()
        target.close()


def s_waiting_for_chooser(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary, ["--capture", "window"])
    try:
        app = annota.app()
        wait(chooser, 15, "GNOME's window chooser")
        if not wait(lambda: waiting_indicator(app), 5, "'Waiting for a window…'"):
            raise Fail("no waiting indicator")
        choose(inp)
        wait(lambda: not waiting_indicator(annota.app()), 5, "the indicator to disappear")
        wait(lambda: toast(app), 10, "the toast")
        return "indicator shown with the chooser, gone after the choice"
    finally:
        annota.stop()
        target.close()


def s_cancel_while_waiting(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary, ["--capture", "window"])
    try:
        app = annota.app()
        during = chooser_sessions(annota)
        indicator = wait(lambda: waiting_indicator(app), 5, "'Waiting for a window…'")
        frame = indicator
        while frame is not None and role(frame) not in ("frame", "window", "dialog"):
            frame = frame.get_parent()
        cancel = find(frame, lambda n: role(n) == "button" and name(n) == "Cancel")
        if not cancel:
            raise Fail("no Cancel button on the indicator")
        press(cancel)
        session_closes(annota, during, 5)
        wait(lambda: not waiting_indicator(app), 5, "the indicator to disappear")
        if not absent(lambda: toast(app) or label_starting(app, "The capture failed")
                      or label_starting(app, "Nothing was captured"), 2):
            raise Fail("a message or image appeared after Cancel")
        wait(lambda: chooser() is None, 5, "GNOME's chooser to close")
        # A following capture starts normally.
        STARTED.append(subprocess.Popen([ctx.binary, "--capture", "window"], env=annota.env,
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
        wait(chooser, 15, "the chooser of the following capture")
        close_chooser()
        return "session closed, no message, next capture opened the chooser"
    finally:
        annota.stop()
        target.close()


def s_chooser_closed(ctx, inp):
    annota = Annota(ctx.binary, ["--capture", "window"])
    try:
        app = annota.app()
        during = chooser_sessions(annota)
        close_chooser()
        wait(lambda: not waiting_indicator(app), 5, "the indicator to disappear")
        if not absent(lambda: toast(app) or label_starting(app, "The capture failed")
                      or label_starting(app, "Nothing was captured"), 2):
            raise Fail("a message or image appeared")
        session_closes(annota, during)
        return "silent"
    finally:
        annota.stop()


def s_delayed(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary, ["--capture", "window", "--delay", "3"])
    try:
        app = annota.app()
        wait(chooser, 15, "GNOME's window chooser")
        if countdown(app):
            raise Fail("countdown runs before a window is chosen")
        choose(inp)
        chosen = time.monotonic()
        text = wait(lambda: countdown(app), 5, "the countdown")
        if not text.startswith("Capturing window in 3 s") and not text.startswith(
                "Capturing window in 2 s"):
            raise Fail("countdown text %r" % text)
        wait(lambda: toast(app), 10, "the toast")
        took = time.monotonic() - chosen
        if took < 2.5:
            raise Fail("frame taken %.1f s after the choice, before the countdown ended" % took)
        return "chooser, countdown, frame after %.1f s" % took
    finally:
        annota.stop()
        target.close()


def s_countdown_cancelled(ctx, inp):
    target = Target()
    annota = Annota(ctx.binary, ["--capture", "window", "--delay", "3"])
    try:
        app = annota.app()
        during = chooser_sessions(annota)
        choose(inp)
        wait(lambda: countdown(app), 5, "the countdown")
        inp.keys("Escape")
        wait(lambda: not countdown(app), 3, "the countdown to close")
        session_closes(annota, during)
        if not absent(lambda: toast(app), 5):
            raise Fail("an image was created after cancelling")
        return "no image, session closed"
    finally:
        annota.stop()
        target.close()


def s_pointer_in_window(ctx, inp):
    target = Target(maximized=True)
    annota = Annota(ctx.binary, ["--capture", "window"], pointer=True)
    try:
        app = annota.app()
        wait(chooser, 15, "GNOME's window chooser")
        choose(inp, before_share=lambda: pointer_over_target(inp))
        text = wait(lambda: toast(app), 10, "the toast")
        image = captured_image(app, inp, text)
        save(ctx, image, "pointer-in-window")
        expect_corners(image, target)
        foreign = foreign_pixels(image, target)
        if foreign < 20:
            raise Fail("no pointer in the image (%d non-target pixels)" % foreign)
        return "pointer present (%d sampled pixels)" % foreign
    finally:
        annota.stop()
        target.close()


def s_cursor_excluded(ctx, inp):
    target = Target(maximized=True)
    annota = Annota(ctx.binary, ["--capture", "window"], pointer=False)
    try:
        app = annota.app()
        wait(chooser, 15, "GNOME's window chooser")
        choose(inp, before_share=lambda: pointer_over_target(inp))
        text = wait(lambda: toast(app), 10, "the toast")
        image = captured_image(app, inp, text)
        save(ctx, image, "cursor-excluded")
        expect_corners(image, target)
        foreign = foreign_pixels(image, target)
        if foreign:
            raise Fail("%d non-target pixels: pointer drawn although excluded" % foreign)
        return "image is the plain window"
    finally:
        annota.stop()
        target.close()


CAP = "capture / Window capture / "
CUR = "capture / Cursor inclusion / "
SCENARIOS = [
    (CAP + "Window on Wayland", s_window_on_wayland),
    (CAP + "The whole window at any display scale", s_whole_window),
    (CAP + "Window mode from the welcome page", s_welcome_page),
    (CAP + "Waiting for the chooser", s_waiting_for_chooser),
    (CAP + "Cancel while waiting", s_cancel_while_waiting),
    (CAP + "Chooser closed", s_chooser_closed),
    (CAP + "Delayed window capture", s_delayed),
    (CAP + "Countdown cancelled", s_countdown_cancelled),
    (CUR + "Pointer in a window capture", s_pointer_in_window),
    (CUR + "Cursor excluded", s_cursor_excluded),
]
# Not driveable on a working GNOME desktop; proven by the fake portal in src/capture/live_tests.rs.
ELSEWHERE = [
    (CAP + "No frame arrives", "src/capture/live_tests.rs (fake portal)"),
    (CAP + "No window source", "src/capture/live_tests.rs (fake portal)"),
]


class Context:
    pass


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--annota", default=shutil.which("annota") or "annota")
    parser.add_argument("--runs", type=int, default=2)
    parser.add_argument("-k", default="", help="only scenarios whose title contains this")
    parser.add_argument("--save", metavar="DIR", help="write each captured image there as PNG")
    args = parser.parse_args()
    if not shutil.which("wl-paste"):
        print("wl-paste (wl-clipboard) is needed", file=sys.stderr)
        return 2
    ctx = Context()
    ctx.binary = args.annota
    ctx.save, ctx.saved = args.save, 0
    if ctx.save:
        os.makedirs(ctx.save, exist_ok=True)
    try:
        return run_suite(ctx, args)
    finally:
        # Whatever a scenario left behind: no target window, no Annota of this suite stays.
        for proc in STARTED:
            end(proc)


def run_suite(ctx, args):
    print("Annota:", subprocess.run([ctx.binary, "--version"], capture_output=True,
                                    text=True).stdout.strip())
    selected = [s for s in SCENARIOS if args.k.lower() in s[0].lower()]
    results = {title: [] for title, _ in selected}
    with portal_input.Input() as inp:
        for run in range(1, args.runs + 1):
            for title, scenario in selected:
                start = time.monotonic()
                # An image left from an earlier scenario or run must never pass for this one's.
                subprocess.run(["wl-copy", "--clear"], check=False)
                try:
                    note = scenario(ctx, inp)
                    outcome = ("pass", note)
                except Fail as err:
                    outcome = ("FAIL", str(err))
                except Exception as err:  # noqa: BLE001 - report and go on
                    outcome = ("FAIL", "%s: %s" % (type(err).__name__, err))
                results[title].append(outcome)
                print("run %d  %-4s  %-60s %4.1fs  %s" % (run, outcome[0], title,
                                                          time.monotonic() - start, outcome[1]),
                      flush=True)
                if chooser() is not None:
                    close_chooser()
                time.sleep(1)
    print("\n| Scenario | " + " | ".join("Run %d" % r for r in range(1, args.runs + 1)) + " |")
    print("|---|" + "---|" * args.runs)
    for title, outcomes in results.items():
        print("| %s | %s |" % (title, " | ".join(o[0] for o in outcomes)))
    for title, where in ELSEWHERE:
        print("| %s | %s |" % (title, " | ".join(["covered by " + where] * args.runs)))
    failed = any(o[0] != "pass" for outs in results.values() for o in outs)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
