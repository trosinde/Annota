#!/usr/bin/env python3
"""Runs recording probes (tools/recording-probe.py) with no human in the loop.

GNOME's screen-sharing chooser is driven the way `tools/live-window-capture.py` drives it: rows
by synthetic keys through the RemoteDesktop portal with its stored restore token
(`tools/portal-input.py`; run `tools/portal-input.py check` once, which asks for consent the
first time), the "Share" button through AT-SPI. The target window is a plain GTK window this
driver starts itself and commands over stdin, so the probe's prompts ("RESIZE ...", "CLOSE THE
WINDOW NOW") are answered by the driver, not by a person.

    tools/recording-probe-drive.py window-end              # P6: maximize, unmaximize, close
    tools/recording-probe-drive.py window-end --duration 30

Only `window-end` is driven so far; other probes can be added as further subcommands.

Needs a logged-in GNOME Wayland session, AT-SPI, and Annota NOT running. Do not touch keyboard or
mouse while it runs. The probe writes its usual log under
`openspec/changes/add-screen-recording/probe-results/` and its frames under /tmp (they show the
target window only; do not commit them).
"""

import argparse
import os
import subprocess
import sys
import threading
import time

TOOLS = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(TOOLS)
TITLE = "Annota live target"

# A 640x400 magenta GTK window: easy to recognise in a frame. Commands on stdin: max, unmax, quit.
TARGET = r"""
import sys, threading
import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, GLib, Gtk
GLib.set_prgname("livetarget")
Gtk.init()
css = Gtk.CssProvider()
css.load_from_string("window, window * { background: #ff00ff; }")
Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 800)
w = Gtk.Window(title=sys.argv[1])
w.set_default_size(640, 400)
w.present()
loop = GLib.MainLoop()
def ready():
    if w.get_surface() is None or w.get_width() == 0:
        return True
    print("ready %d %d" % (w.get_width(), w.get_height()), flush=True)
    return False
GLib.timeout_add(300, ready)
def report(line):
    print("size %s %d %d" % (line, w.get_width(), w.get_height()), flush=True)
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
    return False
def reader():
    for line in sys.stdin:
        GLib.idle_add(cmd, line)
threading.Thread(target=reader, daemon=True).start()
loop.run()
"""


def stamp():
    return time.strftime("%T")


def drive_window_end(args):
    sys.path.insert(0, TOOLS)
    lwc = __import__("live-window-capture")
    portal_input = __import__("portal-input")

    env = dict(os.environ, WAYLAND_DISPLAY=os.environ.get("WAYLAND_DISPLAY") or "wayland-0",
               XDG_SESSION_TYPE="wayland")
    target = subprocess.Popen([sys.executable, "-I", "-c", TARGET, TITLE], env=env,
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    print("target:", target.stdout.readline().strip(), flush=True)

    def target_log():
        for line in target.stdout:
            print("target:", line.strip(), "at", stamp(), flush=True)

    threading.Thread(target=target_log, daemon=True).start()

    def send(command):
        if target.poll() is not None:
            return
        print("-> target", command, stamp(), flush=True)
        target.stdin.write(command + "\n")
        target.stdin.flush()

    probe = subprocess.Popen(
        [sys.executable, "-u", os.path.join(TOOLS, "recording-probe.py"), "window-end",
         "--duration", str(args.duration)], cwd=REPO, env=env, stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    probe.stdin.write("\n")  # answers the probe's "Enter = start"
    probe.stdin.flush()
    state = {"resized": False, "closed": False}

    def probe_log():
        for line in probe.stdout:
            print("probe:", line.rstrip(), flush=True)
            if "RESIZE the recorded window" in line and not state["resized"]:
                state["resized"] = True
                threading.Timer(1.0, send, ["max"]).start()
                threading.Timer(3.0, send, ["unmax"]).start()
            if "CLOSE THE WINDOW NOW" in line and not state["closed"]:
                state["closed"] = True
                send("quit")

    reader = threading.Thread(target=probe_log, daemon=True)
    reader.start()
    try:
        with portal_input.Input() as inp:
            lwc.choose(inp, title=TITLE)
            print("chooser: target chosen and Share pressed", stamp(), flush=True)
            probe.wait(args.duration + 150)
    finally:
        reader.join(5)
        send("quit")
        try:
            target.wait(5)
        except subprocess.TimeoutExpired:
            target.kill()
        if probe.poll() is None:
            probe.kill()
    print("probe exit", probe.returncode, flush=True)
    return probe.returncode or 0


DRIVERS = {"window-end": drive_window_end}


def main():
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0], epilog=__doc__.split("\n\n", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("probe", choices=sorted(DRIVERS), help="probe to drive")
    parser.add_argument("--duration", type=int, default=30,
                        help="probe duration in seconds (default 30; the close comes at half)")
    args = parser.parse_args()
    return DRIVERS[args.probe](args)


if __name__ == "__main__":
    sys.exit(main())
