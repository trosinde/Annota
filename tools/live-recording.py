#!/usr/bin/env python3
"""Automated live recordings on a real GNOME session (add-screen-recording, task 6.2).

Drives whole recordings with no human in the loop, the way tools/live-window-capture.py drives
window stills:
- Annota's own widgets (recording setup's Start Recording, the control's Pause/Resume, mutes and
  Stop, the Export Recording window's Export, Close and Discard) through AT-SPI actions;
  `annota --record screen|window|region|toggle` as a second process, forwarded over D-Bus.
- GNOME's "Share Screen" chooser: rows through synthetic keys from tools/portal-input.py
  (RemoteDesktop portal, restore token), "Share" through AT-SPI; GNOME's ■ stop in the panel
  through gnome-shell's AT-SPI tree (scenario `desktop-stop`).
- A region selection by a synthetic pointer drag over a full-screen pattern window; target
  windows this tool starts itself (magenta, commanded over stdin: maximize, unmaximize, quit).
- A quiet test tone (gst-launch-1.0 audiotestsrc ! pulsesink, volume 0.05) while system audio is
  recorded.
- `dbus-monitor` (own session bus, eavesdropping on the portal's signals only) for
  `Session.Closed` and the GlobalShortcuts bind answer.

Each run writes its facts (timings, outcomes, paths) to `<out>/<scenario>.json` and Annota's
stderr to `<out>/<scenario>.log`; `analyze FILE` prints streams, audio levels per 0.5 s and writes
1 fps frames of a recording.

Needs: a logged-in GNOME Wayland session, AT-SPI, gst-launch-1.0/gst-discoverer-1.0, the
RemoteDesktop token (`tools/portal-input.py check` once), and Annota NOT running (the D-Bus
single instance would forward into it). Do not touch keyboard or mouse while it runs.

    tools/live-recording.py --dev run screen window region desktop-stop
    tools/live-recording.py --dev run shortcut          # presses the reported Record trigger
    tools/live-recording.py analyze _build/scratch/live-recording/screen/x.mp4 --frames DIR

Safety: every run uses fresh, temporary settings and data directories (GSETTINGS_BACKEND=keyfile,
XDG_*_HOME in the work folder), so the user's own Annota settings, library and recordings are
never touched; recordings and exports go to the work folder (default `_build/scratch/
live-recording`, git-ignored; delete it after use, the videos show the screen). `--dev` runs
`_build/src/annota` with the build's GSettings schema (GSETTINGS_SCHEMA_DIR) and resources
(G_RESOURCE_OVERLAYS, icons via XDG_DATA_HOME), so nothing needs installing. Annota, target
windows, the tone and dbus-monitor are stopped on every way out.
"""

import argparse
import glob
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)
lwc = __import__("live-window-capture")
lrp = __import__("live-region-pointer")
from gi.repository import Atspi, GLib  # noqa: E402

Fail = lwc.Fail
wait, find, walk, name, role, press = lwc.wait, lwc.find, lwc.walk, lwc.name, lwc.role, lwc.press
RESOURCE_PREFIX = "/io/github/trosinde/Annota"
TITLE = "Annota live target"
# Region scenario: selection in logical px (top-left, bottom-right).
REGION_FROM, REGION_TO = (1050, 450), (1850, 1050)


# ---------------------------------------------------------------------------------------------
# Target windows.

TARGET = r"""
import sys, threading
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk
GLib.set_prgname("livetarget")
Gtk.init()
mode = sys.argv[2]
w = Gtk.Window(title=sys.argv[1])
if mode == "pattern":
    # Full screen, CSS only: a black 2 px grid line every 100 logical px (at 100 k .. 100 k + 2)
    # over a red (left to right) and green (top to bottom) gradient.
    css = Gtk.CssProvider()
    css.load_from_string(
        "window { background-color: #000080; background-image: "
        "repeating-linear-gradient(to right, black 0px, black 2px, transparent 2px, transparent 100px), "
        "repeating-linear-gradient(to bottom, black 0px, black 2px, transparent 2px, transparent 100px), "
        "linear-gradient(to bottom, rgba(0,255,0,0), rgba(0,255,0,0.6)), "
        "linear-gradient(to right, #000080, #ff0080); }")
    Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 800)
    w.fullscreen()
else:
    css = Gtk.CssProvider()
    css.load_from_string("window, window * { background: #ff00ff; }")
    Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 800)
    w.set_default_size(640, 400)
    # A small counter label: changing it after a resize makes the window really redraw.
    w.set_child(Gtk.Label(label="0"))
    if mode == "max":
        w.maximize()
w.present()
loop = GLib.MainLoop()
def ready():
    if w.get_surface() is None or w.get_width() == 0 or (mode == "pattern" and not w.is_fullscreen()) \
            or (mode == "max" and not w.is_maximized()):
        return True
    print("ready %d %d %.4f" % (w.get_width(), w.get_height(), w.get_surface().get_scale()),
          flush=True)
    return False
GLib.timeout_add(300, ready)
def report(line):
    print("size %s %d %d" % (line, w.get_width(), w.get_height()), flush=True)
    return False
def redraw():
    # Real apps redraw after a resize; mutter streams GNOME's maximize/unmaximize animation
    # frame until the window's next redraw (measured 2026-11-24, design.md), so the target
    # redraws once 1.5 s after each size change.
    child = w.get_child()
    if isinstance(child, Gtk.Label):
        child.set_label(str(int(child.get_label()) + 1))
    w.queue_draw()
    return False
def cmd(line):
    line = line.strip()
    if line == "max":
        w.maximize()
    elif line == "unmax":
        w.unmaximize()
    elif line == "quit":
        w.destroy()
        loop.quit()
        return False
    GLib.timeout_add(500, report, line)
    if line in ("max", "unmax"):
        GLib.timeout_add(1500, redraw)
    return False
def reader():
    for line in sys.stdin:
        GLib.idle_add(cmd, line)
threading.Thread(target=reader, daemon=True).start()
w.connect("close-request", lambda *_: (loop.quit(), False)[1])
loop.run()
"""


class Target:
    def __init__(self, mode="window"):
        self.proc = subprocess.Popen([sys.executable, "-I", "-c", TARGET, TITLE, mode],
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        line = self.proc.stdout.readline().split()
        if not line or line[0] != "ready":
            self.close()
            raise Fail("target window did not start")
        self.width, self.height, self.scale = int(line[1]), int(line[2]), float(line[3])
        self.sizes = []
        threading.Thread(target=self._read, daemon=True).start()
        time.sleep(0.5)

    def _read(self):
        for line in self.proc.stdout:
            self.sizes.append((round(time.monotonic(), 2), line.strip()))

    def send(self, command):
        if self.proc.poll() is None:
            self.proc.stdin.write(command + "\n")
            self.proc.stdin.flush()

    def close(self):
        if self.proc.poll() is None:
            try:
                self.send("quit")
                self.proc.wait(3)
            except (subprocess.TimeoutExpired, BrokenPipeError, OSError):
                self.proc.kill()
                self.proc.wait(5)


# ---------------------------------------------------------------------------------------------
# Annota under test.

def prepare_dev(work):
    """The build's schema and resources, unpacked into the work folder (no install needed)."""
    dev = os.path.join(work, "dev")
    schemas, res = os.path.join(dev, "schemas"), os.path.join(dev, "res")
    shutil.rmtree(dev, ignore_errors=True)
    os.makedirs(schemas)
    subprocess.run(["glib-compile-schemas", os.path.join(REPO, "data"), "--targetdir", schemas],
                   check=True)
    bundle = os.path.join(REPO, "_build", "data", "resources.gresource")
    for path in subprocess.run(["gresource", "list", bundle], capture_output=True, text=True,
                               check=True).stdout.split():
        out = os.path.join(res, path[len(RESOURCE_PREFIX) + 1:])
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "wb") as f:
            f.write(subprocess.run(["gresource", "extract", bundle, path], capture_output=True,
                                   check=True).stdout)
    return {"binary": os.path.join(REPO, "_build", "src", "annota"), "schemas": schemas,
            "res": res}


class Annota:
    def __init__(self, ctx, run_dir, args=(), settings=None):
        self.ctx = ctx
        self.home = os.path.join(run_dir, "home")
        shutil.rmtree(self.home, ignore_errors=True)
        conf = os.path.join(self.home, "config", "glib-2.0", "settings")
        os.makedirs(conf)
        self.rec_dir = os.path.join(run_dir, "recordings")
        self.export_dir = os.path.join(run_dir, "exports")
        os.makedirs(self.rec_dir, exist_ok=True)
        os.makedirs(self.export_dir, exist_ok=True)
        values = {"after-capture": "'editor'", "record-folder": "'%s'" % self.rec_dir,
                  "record-export-folder": "'%s'" % self.export_dir}
        values.update(settings or {})
        with open(os.path.join(conf, "keyfile"), "w") as f:
            f.write("[%s]\n" % lwc.SETTINGS_GROUP)
            for key, value in values.items():
                f.write("%s=%s\n" % (key, value))
        env = dict(os.environ, GSETTINGS_BACKEND="keyfile", LANGUAGE="en", LC_ALL="C.UTF-8",
                   XDG_CONFIG_HOME=os.path.join(self.home, "config"),
                   XDG_DATA_HOME=os.path.join(self.home, "data"),
                   XDG_STATE_HOME=os.path.join(self.home, "state"),
                   XDG_CACHE_HOME=os.path.join(self.home, "cache"),
                   GST_DEBUG=os.environ.get("GST_DEBUG", "pipewiresrc:4,basesrc:4"),
                   GST_DEBUG_NO_COLOR="1")
        if ctx.dev:
            env["GSETTINGS_SCHEMA_DIR"] = ctx.dev["schemas"]
            env["G_RESOURCE_OVERLAYS"] = "%s=%s" % (RESOURCE_PREFIX, ctx.dev["res"])
            icons = os.path.join(self.home, "data", "icons", "hicolor")
            shutil.copytree(os.path.join(ctx.dev["res"], "icons"), icons)
        self.env = env
        self.log_path = os.path.join(run_dir, "annota.log")
        self.log = open(self.log_path, "w")
        self.proc = subprocess.Popen([ctx.binary, *args], env=env, stdout=self.log,
                                     stderr=subprocess.STDOUT)

    def cli(self, *args):
        done = subprocess.run([self.ctx.binary, *args], env=self.env, capture_output=True,
                              text=True, timeout=15)
        return done.returncode

    def app(self, timeout=15):
        return wait(lambda: lwc.atspi_app(self.proc.pid), timeout, "Annota on AT-SPI")

    def stop(self):
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(5)
        self.log.close()
        shutil.rmtree(self.home, ignore_errors=True)


# ---------------------------------------------------------------------------------------------
# Helpers.

class Facts(dict):
    def __init__(self):
        super().__init__(events=[])
        self.t0 = time.monotonic()

    def event(self, what, **extra):
        t = round(time.monotonic() - self.t0, 2)
        self["events"].append(dict(t=t, what=what, **extra))
        print("  %6.2f  %s %s" % (t, what, extra if extra else ""), flush=True)
        return t


class Tone:
    """A quiet sine to the default output, so the monitor source carries a signal."""

    def __init__(self, freq=880):
        self.proc = subprocess.Popen(
            ["gst-launch-1.0", "-q", "audiotestsrc", "is-live=true", "freq=%d" % freq,
             "volume=0.05", "!", "audioconvert", "!", "pulsesink"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def stop(self):
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGINT)
            try:
                self.proc.wait(3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait(3)


class PortalMonitor:
    """dbus-monitor on the portal's Session.Closed and Request.Response signals."""

    def __init__(self, facts):
        self.facts = facts
        self.lines = []
        self.proc = subprocess.Popen(
            ["dbus-monitor", "--session",
             "type='signal',interface='org.freedesktop.portal.Session',member='Closed'",
             "type='signal',interface='org.freedesktop.portal.Request',member='Response'",
             "type='method_call',interface='org.freedesktop.portal.GlobalShortcuts'"],
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.proc.stdout:
            self.lines.append(line.rstrip())
            if "member=Closed" in line and "portal.Session" in line:
                self.facts.event("dbus Session.Closed", line=line.strip()[:160])

    def stop(self):
        if self.proc.poll() is None:
            self.proc.terminate()
            self.proc.wait(3)


PROVIDER = "gnome-control-center-global-shortcuts-provider"


def shortcut_dialogs():
    for app in lwc.apps():
        if name(app) == PROVIDER:
            return [f for f in (app.get_child_at_index(i) for i in range(app.get_child_count()))
                    if f is not None and name(f) == "Add Keyboard Shortcuts"]
    return []


def dismiss_shortcut_dialogs(facts, timeout=4.0):
    """GNOME's "Add Keyboard Shortcuts" dialog, which GNOME shows for Annota's BindShortcuts in
    every fresh settings session: its proposed triggers are recorded, then it is cancelled
    (never "Add": that would store bindings in the user's GNOME settings)."""
    end = time.monotonic() + timeout
    while time.monotonic() < end and not shortcut_dialogs():
        time.sleep(0.2)
    for dialog in shortcut_dialogs():
        texts = [name(n) for n in walk(dialog) if role(n) == "label"]
        facts.setdefault("shortcut_dialog", texts)
        cancel = find(dialog, lambda n: role(n) == "button" and name(n) == "Cancel")
        if cancel is not None and press(cancel):
            facts.event("GNOME 'Add Keyboard Shortcuts' dialog cancelled")
        time.sleep(0.5)
        check_overview(facts, "after cancelling the shortcut dialog")


def overview_open():
    """Whether GNOME's Activities overview is shown (its panel toggle is checked)."""
    for app in lwc.apps():
        if name(app) == "gnome-shell":
            node = find(app, lambda n: role(n) == "toggle button" and name(n) == "Activities")
            return node is not None and lwc.has_state(node, Atspi.StateType.CHECKED)
    return False


def check_overview(facts, where, inp=None):
    """Records the overview's state; with `inp`, closes it with Escape (the recordings must show
    the desktop, not the overview)."""
    if overview_open():
        facts.event("overview open " + where)
        if inp is not None:
            inp.keys("Escape")
            time.sleep(0.8)
            facts.event("overview closed with Escape: %s" % (not overview_open()))


def frames(app):
    return [app.get_child_at_index(i) for i in range(app.get_child_count())]


def frame_named(app, title):
    for f in frames(app):
        if name(f) == title:
            return f
    return None


def control(app):
    return frame_named(app, "Recording") or frame_named(app, "Paused")


def button(root, prefix):
    return find(root, lambda n: role(n) in ("button", "toggle button")
                and name(n).startswith(prefix))


def press_button(root, prefix):
    node = wait(lambda: button(root, prefix), 5, "button %r" % prefix)
    if not press(node):
        raise Fail("cannot press %r" % prefix)


def label_text(root, predicate):
    node = find(root, lambda n: role(n) == "label" and predicate(name(n)))
    return name(node) if node else None


def elapsed(ctrl):
    return label_text(ctrl, lambda t: re.fullmatch(r"\d+:\d\d", t) is not None)


def choose_first(inp):
    """GNOME's chooser, monitor list: the first monitor's toggle button, then Share."""
    dialog = wait(lwc.chooser, 15, "GNOME's chooser")
    time.sleep(0.4)
    toggles = wait(lambda: [n for n in walk(dialog) if role(n) == "toggle button"], 5,
                   "monitor buttons in the chooser")
    first = toggles[0]
    # A single monitor comes preselected ("pressed") and focused. Keys only: Tab would move to
    # Cancel, and an AT-SPI action on the toggle unselects it.
    if not lwc.has_state(first, Atspi.StateType.PRESSED):
        if not lwc.has_state(first, Atspi.StateType.FOCUSED):
            raise Fail("monitor %r neither selected nor focused" % name(first))
        inp.keys("space")
        time.sleep(0.3)
    if not lwc.has_state(first, Atspi.StateType.PRESSED):
        raise Fail("could not select the monitor %r" % name(first))
    share = find(dialog, lambda n: role(n) == "label" and name(n) == "Share")
    if not share or not press(share):
        raise Fail("no Share button")
    return name(first)


def watch_start(app, facts, timeout=30):
    """From the chooser answer to the control: waiting indicator, countdown, control."""
    seen = {}
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if "waiting" not in seen and label_text(app, lambda t: t.startswith("Waiting for a")):
            seen["waiting"] = facts.event("waiting indicator shown")
        if "waiting" in seen and "waiting_gone" not in seen and not label_text(
                app, lambda t: t.startswith("Waiting for a")):
            seen["waiting_gone"] = facts.event("waiting indicator gone")
        text = label_text(app, lambda t: t.startswith("Recording starts in"))
        if text and "countdown" not in seen:
            seen["countdown"] = facts.event("countdown shown", text=text)
        if "countdown" in seen and not text and "countdown_gone" not in seen:
            seen["countdown_gone"] = facts.event("countdown gone")
        if control(app):
            seen["control"] = facts.event("control shown")
            check_overview(facts, "when the control appeared")
            return seen
        time.sleep(0.05)
    raise Fail("no recording control within %d s" % timeout)


def export(app, facts, run_dir, rec_dir, export_dir, keep_working=True):
    """Waits for the Export Recording window, copies the working file, exports, returns paths."""
    win = wait(lambda: frame_named(app, "Export Recording"), 15, "the Export Recording window")
    facts.event("export window shown")
    working = glob.glob(os.path.join(rec_dir, "*.mkv"))
    facts["working_files"] = working
    copy = None
    if working and keep_working:
        copy = os.path.join(run_dir, "working-copy.mkv")
        shutil.copy(working[0], copy)
    time.sleep(1.0)
    press_button(win, "Export")
    facts.event("Export pressed")
    wait(lambda: label_text(win, lambda t: t == "Saved"), 120, "'Saved'")
    facts.event("Saved shown")
    out = sorted(glob.glob(os.path.join(export_dir, "*")))
    facts["exported"] = out
    facts["working_after_export"] = glob.glob(os.path.join(rec_dir, "*.mkv"))
    show = button(win, "Show in Files")
    facts["show_in_files_present"] = show is not None  # never pressed: it opens Nautilus
    press_button(win, "Close")
    wait(lambda: frame_named(app, "Export Recording") is None, 5, "the window to close")
    facts.event("export window closed")
    return out, copy


def pointer_visit(inp, width, points):
    pointer = lrp.Pointer(inp, width)
    for x, y in points:
        pointer.to(x, y, steps=8)
        time.sleep(0.3)
    return pointer


def run_scenario(ctx, title, func):
    run_dir = os.path.join(ctx.work, title)
    shutil.rmtree(run_dir, ignore_errors=True)
    os.makedirs(run_dir)
    facts = Facts()
    facts["scenario"] = title
    print("== %s" % title, flush=True)
    monitor = PortalMonitor(facts)
    try:
        check_overview(facts, "before the scenario", ctx.inp)
        func(ctx, run_dir, facts)
        facts["outcome"] = "done"
    except Exception as err:  # noqa: BLE001 - report and go on
        facts["outcome"] = "FAIL %s: %s" % (type(err).__name__, err)
    finally:
        monitor.stop()
        with open(os.path.join(ctx.out, title + ".dbus.txt"), "w") as f:
            f.write("\n".join(monitor.lines))
        log = os.path.join(run_dir, "annota.log")
        if os.path.exists(log):
            with open(log, errors="replace") as f:
                text = f.read()
            facts["log_eos_lines"] = [ln for ln in text.splitlines()
                                      if re.search(r"\bEOS\b|eos|end of stream|unlinked|"
                                                   r"state.*(unconnected|error)", ln)][:40]
            facts["log_annota"] = [ln for ln in text.splitlines() if ln.startswith("annota")]
            shutil.copy(log, os.path.join(ctx.out, title + ".log"))
        with open(os.path.join(ctx.out, title + ".json"), "w") as f:
            json.dump(facts, f, indent=2)
        print("-> %s: %s" % (title, facts["outcome"]), flush=True)
        if lwc.chooser() is not None:
            lwc.close_chooser()
        dismiss_shortcut_dialogs(facts, 0)
    return facts


# ---------------------------------------------------------------------------------------------
# Scenarios.

def s_screen(ctx, run_dir, facts):
    """Screen, countdown 3, microphone + system audio, pointer on: record ~8 s, mute system
    audio 3-5.5 s, pause 3 s, record ~4 s (mute microphone 1-2.5 s into it), stop by
    `--record toggle`, export MP4."""
    tone = Tone()
    backdrop = Target("max")  # hides the desktop's windows; non-magenta pixels are UI or pointer
    annota = Annota(ctx, run_dir, ["--record", "screen"],
                    {"record-countdown": "3", "record-microphone": "true",
                     "record-system-audio": "true", "include-pointer": "true"})
    try:
        app = annota.app()
        dismiss_shortcut_dialogs(facts)
        wait(lwc.chooser, 15, "GNOME's chooser")
        facts["waiting_with_chooser"] = bool(label_text(app, lambda t: t.startswith("Waiting")))
        facts["row"] = choose_first(ctx.inp)
        facts.event("Share pressed")
        seen = watch_start(app, facts)
        t_start = seen["control"]
        ctrl = control(app)
        time.sleep(1.0)
        pointer_visit(ctx.inp, 3072, [(600, 400), (700, 450)])
        facts.event("pointer at 700,450")
        time.sleep(max(0, t_start + 3.0 - (time.monotonic() - facts.t0)))
        press_button(ctrl, "Mute system audio")
        facts.event("system audio muted")
        time.sleep(2.5)
        press_button(ctrl, "System audio muted")
        facts.event("system audio unmuted")
        time.sleep(max(0, t_start + 8.0 - (time.monotonic() - facts.t0)))
        press_button(ctrl, "Pause")
        t_pause = facts.event("paused", elapsed=elapsed(ctrl))
        time.sleep(1.5)
        facts["paused_label"] = label_text(ctrl, lambda t: t == "Paused")
        facts["elapsed_during_pause"] = elapsed(ctrl)
        time.sleep(1.5)
        facts["elapsed_end_of_pause"] = elapsed(ctrl)
        press_button(ctrl, "Resume")
        t_resume = facts.event("resumed")
        time.sleep(1.0)
        press_button(ctrl, "Mute microphone")
        facts.event("microphone muted")
        time.sleep(1.5)
        press_button(ctrl, "Microphone muted")
        facts.event("microphone unmuted")
        time.sleep(max(0, t_resume + 4.0 - (time.monotonic() - facts.t0)))
        facts["elapsed_before_stop"] = elapsed(ctrl)
        t_stop = facts.event("--record toggle")
        facts["toggle_exit"] = annota.cli("--record", "toggle")
        facts["expected_duration"] = round((t_pause - t_start) + (t_stop - t_resume), 2)
        facts["exports"], facts["working_copy"] = export(
            app, facts, run_dir, annota.rec_dir, annota.export_dir)
    finally:
        tone.stop()
        annota.stop()
        backdrop.close()


def s_window(ctx, run_dir, facts):
    """Window of a magenta target, countdown 0, system audio, pointer off: record, maximize,
    unmaximize, close the target (Recorded window closed), export WebM."""
    tone = Tone(660)
    target = Target("window")
    annota = None
    try:
        facts["target_logical"] = [target.width, target.height, target.scale]
        annota = Annota(ctx, run_dir, ["--record", "window"],
                        {"record-countdown": "0", "record-microphone": "false",
                         "record-system-audio": "true", "include-pointer": "false",
                         "record-export-format": "'webm'"})
        app = annota.app()
        dismiss_shortcut_dialogs(facts)
        wait(lwc.chooser, 15, "GNOME's chooser")
        facts["waiting_with_chooser"] = bool(label_text(app, lambda t: t.startswith("Waiting")))
        lwc.choose(ctx.inp, title=TITLE)
        facts.event("Share pressed")
        watch_start(app, facts)
        time.sleep(1.0)
        # Pointer over the (unmaximized) target, which GNOME centres: hidden mode, so absent.
        pointer_visit(ctx.inp, 3072, [(1536, 864), (1600, 900)])
        time.sleep(1.5)
        target.send("max")
        facts.event("target maximize")
        time.sleep(3.0)
        target.send("unmax")
        facts.event("target unmaximize")
        time.sleep(3.0)
        target.send("quit")
        facts.event("target closed")
        facts["target_sizes"] = target.sizes
        facts["exports"], facts["working_copy"] = export(
            app, facts, run_dir, annota.rec_dir, annota.export_dir)
    finally:
        tone.stop()
        if annota:
            annota.stop()
        target.close()


def s_region(ctx, run_dir, facts):
    """Region 800x600 logical at (1050, 450) over a full-screen pattern, countdown 1, no audio,
    pointer on (moved into the area), Stop button, export MP4."""
    target = Target("pattern")
    annota = None
    try:
        facts["target_logical"] = [target.width, target.height, target.scale]
        facts["selection_logical"] = [REGION_FROM, REGION_TO]
        annota = Annota(ctx, run_dir, ["--record", "region"],
                        {"record-countdown": "1", "record-microphone": "false",
                         "record-system-audio": "false", "include-pointer": "true"})
        app = annota.app()
        dismiss_shortcut_dialogs(facts)
        wait(lambda: button(app, "Start Recording"), 20, "the recording setup")
        facts.event("setup shown")
        time.sleep(0.8)
        pointer = lrp.Pointer(ctx.inp, target.width)
        pointer.to(*REGION_FROM)
        pointer.button(True)
        try:
            pointer.to(*REGION_TO, steps=12)
        finally:
            pointer.button(False)
        time.sleep(0.5)
        pointer.to(2400, 1500)  # away from the area until the recording runs
        facts["size_labels"] = [name(n) for n in walk(app) if role(n) == "label"
                                and re.search(r"\d+\s*[×x]\s*\d+", name(n))]
        press_button(app, "Start Recording")
        facts.event("Start Recording pressed")
        wait(lwc.chooser, 15, "GNOME's chooser")
        facts["row"] = choose_first(ctx.inp)
        facts.event("Share pressed")
        watch_start(app, facts)
        ctrl = control(app)
        time.sleep(1.5)
        pointer.to(1450, 750, steps=8)  # centre of the area
        facts.event("pointer in the area")
        time.sleep(2.0)
        pointer.to(2400, 1500, steps=8)
        facts.event("pointer out of the area")
        time.sleep(1.5)
        press_button(ctrl, "Stop")
        facts.event("Stop pressed")
        facts["exports"], facts["working_copy"] = export(
            app, facts, run_dir, annota.rec_dir, annota.export_dir)
    finally:
        if annota:
            annota.stop()
        target.close()


def shell_stop_button():
    """GNOME's ■ next to the screen-sharing indicator, from gnome-shell's AT-SPI tree."""
    for app in lwc.apps():
        if name(app) != "gnome-shell":
            continue
        for node in walk(app):
            label = name(node).lower()
            if role(node) in ("push button", "button") and lwc.has_state(
                    node, Atspi.StateType.SHOWING) and (
                    "stop" in label or "screen shar" in label or "screencast" in label):
                return node
    return None


def shell_buttons():
    """gnome-shell's shown buttons, with their AT-SPI actions."""
    out = []
    for app in lwc.apps():
        if name(app) == "gnome-shell":
            for node in walk(app):
                if role(node) in ("push button", "button", "toggle button", "menu", "panel") \
                        and lwc.has_state(node, Atspi.StateType.SHOWING):
                    action = node.get_action_iface()
                    actions = [action.get_action_name(i) for i in range(action.get_n_actions())] \
                        if action else []
                    out.append("%s:%s:%s" % (role(node), name(node), actions))
    return out


def s_desktop_stop(ctx, run_dir, facts):
    """Screen, countdown 0, no audio: after 3 s GNOME's ■ in the panel (AT-SPI), then the
    Export Recording window is closed with Discard."""
    backdrop = Target("max")
    annota = Annota(ctx, run_dir, ["--record", "screen"],
                    {"record-countdown": "0", "record-microphone": "false",
                     "record-system-audio": "false", "include-pointer": "false"})
    try:
        app = annota.app()
        dismiss_shortcut_dialogs(facts)
        facts["shell_buttons_before"] = shell_buttons()
        wait(lwc.chooser, 15, "GNOME's chooser")
        choose_first(ctx.inp)
        facts.event("Share pressed")
        watch_start(app, facts)
        time.sleep(3.0)
        facts["shell_buttons_during"] = shell_buttons()
        stop = shell_stop_button()
        if stop is None:
            raise Fail("no stop button in gnome-shell's AT-SPI tree")
        # The button has no AT-SPI action: click its centre with the pointer (gnome-shell is the
        # compositor, so its extents are screen coordinates in logical px).
        box = stop.get_component_iface().get_extents(Atspi.CoordType.SCREEN)
        facts["shell_stop"] = "%s:%s at %d,%d %dx%d" % (role(stop), name(stop), box.x, box.y,
                                                       box.width, box.height)
        if box.width <= 0 or box.height <= 0:
            raise Fail("GNOME's stop button has no extents")
        pointer = lrp.Pointer(ctx.inp, 3072)
        pointer.to(box.x + box.width / 2, box.y + box.height / 2, steps=10)
        time.sleep(0.3)
        ctx.inp.click()
        facts.event("GNOME stop clicked")
        win = wait(lambda: frame_named(app, "Export Recording"), 15,
                   "the Export Recording window")
        facts.event("export window shown")
        working = glob.glob(os.path.join(annota.rec_dir, "*.mkv"))
        facts["working_files"] = working
        if working:
            shutil.copy(working[0], os.path.join(run_dir, "working-copy.mkv"))
        time.sleep(1.0)
        press_button(win, "Discard")
        dialog = wait(lambda: label_text(app, lambda t: t == "Discard Recording?") and app, 5,
                      "'Discard Recording?'")
        facts.event("discard asked")
        nodes = [n for n in walk(dialog) if role(n) == "button" and name(n) == "Discard"]
        if not nodes:
            raise Fail("no Discard in the dialog")
        press(nodes[-1])
        wait(lambda: frame_named(app, "Export Recording") is None, 5, "the window to close")
        facts.event("discarded")
        facts["working_after_discard"] = glob.glob(os.path.join(annota.rec_dir, "*.mkv"))
    finally:
        annota.stop()
        backdrop.close()


def record_trigger(app):
    card = wait(lambda: find(app, lambda n: role(n) == "button"
                             and (name(n) == "Record" or n.get_description() == "Record video"
                                  or "Record" in name(n))), 10, "the Record card")
    keys = [name(n) for n in walk(card) if role(n) == "label" and name(n) != "Record"]
    return card, keys


KEYSYM = {"Super": "Super_L", "Alt": "Alt_L", "Ctrl": "Control_L", "Shift": "Shift_L"}


def press_trigger(inp, keys):
    sym = lwc.portal_input.keysym
    mods = [KEYSYM[k] for k in keys[:-1]]
    for m in mods:
        inp._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym(m), 1)
    try:
        inp.keys(keys[-1].lower())
    finally:
        for m in reversed(mods):
            inp._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym(m), 0)


def s_shortcut(ctx, run_dir, facts):
    """The Record shortcut: the trigger the portal reports (Record card), pressed once with no
    recording (setup opens; Esc cancels it), and once during a screen recording (stops it)."""
    annota = Annota(ctx, run_dir, [],
                    {"record-countdown": "0", "record-microphone": "false",
                     "record-system-audio": "false", "include-pointer": "false"})
    try:
        app = annota.app()
        dismiss_shortcut_dialogs(facts)
        time.sleep(3.0)
        card, keys = record_trigger(app)
        facts["record_card_keys"] = keys
        facts.event("record card", keys=keys)
        if not keys or any(k not in KEYSYM for k in keys[:-1]) or len(keys[-1]) != 1:
            raise Fail("no usable trigger on the Record card: %s" % keys)
        press_trigger(ctx.inp, keys)
        facts.event("trigger pressed (idle)")
        wait(lambda: button(app, "Start Recording"), 10, "the recording setup")
        facts.event("setup shown by the shortcut")
        time.sleep(0.8)
        ctx.inp.keys("Escape")
        wait(lambda: button(app, "Start Recording") is None, 5, "the setup to close")
        facts.event("setup cancelled with Esc")
        facts["recordings_after_cancel"] = os.listdir(annota.rec_dir)
        annota.cli("--record", "screen")
        wait(lwc.chooser, 15, "GNOME's chooser")
        choose_first(ctx.inp)
        watch_start(app, facts)
        time.sleep(3.0)
        press_trigger(ctx.inp, keys)
        facts.event("trigger pressed (recording)")
        win = wait(lambda: frame_named(app, "Export Recording"), 10, "the Export window")
        facts.event("export window shown by the shortcut")
        press_button(win, "Discard")
        time.sleep(0.5)
        nodes = [n for n in walk(app) if role(n) == "button" and name(n) == "Discard"]
        press(nodes[-1])
        wait(lambda: frame_named(app, "Export Recording") is None, 5, "the window to close")
        facts.event("discarded")
    finally:
        annota.stop()


SCENARIOS = {"screen": s_screen, "window": s_window, "region": s_region,
             "desktop-stop": s_desktop_stop, "shortcut": s_shortcut}


# ---------------------------------------------------------------------------------------------
# Analysis of a recording.

def analyze(path, frames_dir=None, width=1280):
    gi_ = __import__("gi")
    gi_.require_version("Gst", "1.0")
    gi_.require_version("GstPbutils", "1.0")
    from gi.repository import Gst, GstPbutils
    Gst.init(None)
    info = GstPbutils.Discoverer.new(30 * Gst.SECOND).discover_uri(
        GLib.filename_to_uri(os.path.abspath(path)))
    result = {"file": path, "duration": info.get_duration() / Gst.SECOND,
              "container": info.get_stream_info().get_caps().to_string().split(",")[0],
              "video": [], "audio": []}
    for v in info.get_video_streams():
        caps = v.get_caps().to_string().split(",")[0]
        fr = v.get_framerate_num(), v.get_framerate_denom()
        result["video"].append({"caps": caps, "size": [v.get_width(), v.get_height()],
                                "framerate": "%d/%d" % fr})
    for a in info.get_audio_streams():
        result["audio"].append({"caps": a.get_caps().to_string().split(",")[0],
                                "channels": a.get_channels(), "rate": a.get_sample_rate()})
    # Audio levels per stream, 0.5 s windows (peak dB of the louder channel).
    for index in range(len(result["audio"])):
        pipe = Gst.Pipeline.new()
        decoder = Gst.ElementFactory.make("uridecodebin", "d")
        decoder.set_property("uri", GLib.filename_to_uri(os.path.abspath(path)))
        pipe.add(decoder)
        levels = []
        state = {"n": 0}

        def on_pad(_d, pad, pipe=pipe, state=state):
            caps = pad.get_current_caps() or pad.query_caps(None)
            if not caps.to_string().startswith("audio/"):
                sink = Gst.ElementFactory.make("fakesink")
                pipe.add(sink)
                sink.sync_state_with_parent()
                pad.link(sink.get_static_pad("sink"))
                return
            mine = state["n"] == index
            state["n"] += 1
            bin_ = Gst.parse_bin_from_description(
                "audioconvert ! level interval=500000000 post-messages=%s ! fakesink sync=false"
                % ("true" if mine else "false"), True)
            pipe.add(bin_)
            bin_.sync_state_with_parent()
            pad.link(bin_.get_static_pad("sink"))

        pipe.get_by_name("d").connect("pad-added", on_pad)
        pipe.set_state(Gst.State.PLAYING)
        bus = pipe.get_bus()
        while True:
            msg = bus.timed_pop_filtered(20 * Gst.SECOND, Gst.MessageType.ELEMENT
                                         | Gst.MessageType.EOS | Gst.MessageType.ERROR)
            if msg is None or msg.type != Gst.MessageType.ELEMENT:
                break
            s = msg.get_structure()
            if s.get_name() == "level":
                peak = s.get_value("peak")
                t = s.get_value("stream-time") / Gst.SECOND
                levels.append([round(t, 1), round(max(peak), 1)])
        pipe.set_state(Gst.State.NULL)
        result["audio"][index]["peak_db_per_0.5s"] = levels
    if frames_dir:
        os.makedirs(frames_dir, exist_ok=True)
        uri = GLib.filename_to_uri(os.path.abspath(path))
        subprocess.run(
            ["gst-launch-1.0", "-q", "uridecodebin", "uri=" + uri, "!", "videoconvert", "!",
             "videorate", "!", "video/x-raw,framerate=1/1", "!", "videoscale", "!",
             "video/x-raw,width=%d,pixel-aspect-ratio=1/1" % width, "!", "pngenc", "!",
             "multifilesink", "location=" + os.path.join(frames_dir, "frame-%03d.png")],
            check=False, timeout=600)
        result["frames"] = sorted(glob.glob(os.path.join(frames_dir, "frame-*.png")))
    return result


# ---------------------------------------------------------------------------------------------

class Context:
    pass


def main():
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0], epilog=__doc__.split("\n\n", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    run = sub.add_parser("run", help="drive recordings")
    run.add_argument("scenarios", nargs="+", choices=sorted(SCENARIOS))
    run.add_argument("--annota", default=shutil.which("annota") or "annota")
    run.add_argument("--dev", action="store_true",
                     help="run _build/src/annota with the build's schema and resources")
    run.add_argument("--work", default=os.path.join(REPO, "_build", "scratch", "live-recording"))
    run.add_argument("--out", default="", help="folder for the JSON facts and logs "
                     "(default: the work folder)")
    an = sub.add_parser("analyze", help="streams, levels and frames of a recording")
    an.add_argument("file")
    an.add_argument("--frames", default="", help="write 1 fps PNG frames here")
    an.add_argument("--width", type=int, default=1280)
    args = parser.parse_args()
    if args.command == "analyze":
        print(json.dumps(analyze(args.file, args.frames or None, args.width), indent=1))
        return 0
    if subprocess.run(["pgrep", "-x", "annota"], capture_output=True).returncode == 0:
        print("Annota is running: quit it first (the D-Bus single instance would forward "
              "into it)", file=sys.stderr)
        return 2
    ctx = Context()
    ctx.work = os.path.abspath(args.work)
    ctx.out = os.path.abspath(args.out or args.work)
    os.makedirs(ctx.work, exist_ok=True)
    os.makedirs(ctx.out, exist_ok=True)
    ctx.dev = prepare_dev(ctx.work) if args.dev else None
    ctx.binary = ctx.dev["binary"] if ctx.dev else args.annota
    print("Annota:", subprocess.run([ctx.binary, "--version"], capture_output=True,
                                    text=True).stdout.strip(), flush=True)
    failed = False
    for title in args.scenarios:
        # One input session per scenario: GNOME's "Stop Screen Sharing" (desktop-stop) also ends
        # the RemoteDesktop session, which then cannot be closed any more.
        inp = lwc.portal_input.Input().__enter__()
        ctx.inp = inp
        try:
            facts = run_scenario(ctx, title, SCENARIOS[title])
        finally:
            try:
                inp.__exit__(None, None, None)
            except GLib.Error:
                pass
        failed |= facts["outcome"] != "done"
        time.sleep(1.5)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
