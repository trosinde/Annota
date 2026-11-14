#!/usr/bin/env python3
"""Asks the real xdg-desktop-portal for one screenshot and reports what came back.

Annota's own tests answer every portal call themselves (`src/capture/live_tests.rs` owns
`org.freedesktop.portal.Desktop` on a private bus). That proves what Annota sends and how it
sorts the answers — it cannot prove what a real GNOME session does with them. This tool closes
that gap: it is the one place where an assumption about the portal can be checked instead of
believed, and it needs no Annota build.

It was written after an assumption went unchecked for four days. The `capture` spec says a
window capture is "chosen through the portal's interactive picker"; in truth the Screenshot
portal has no mode option at all. `interactive: true` opens GNOME's general screenshot UI
(xdg-desktop-portal-gnome calls `org.gnome.Shell.Screenshot.InteractiveScreenshot`), and that
UI answers response code 2 — the same code as a real failure — both when the user closes it and
when it never opens. Run this before trusting any sentence about how the portal behaves.

    tools/portal-probe.py                 # frozen grab: what region and screen capture use
    tools/portal-probe.py --interactive   # GNOME's screenshot UI: what window capture uses

The elapsed time is the useful part of an interactive run. A UI the user closes answers after
seconds; a UI whose modal grab failed, and which therefore never appeared, answers in
milliseconds. Response code 2 alone does not tell them apart, and neither can Annota.

Nothing is kept: the image the portal writes is measured (width and height from the PNG header)
and deleted again, exactly as `portal::discard_portal_file` does. Its content is never read,
copied or printed.
"""

import argparse
import os
import struct
import sys
import time
from urllib.parse import unquote, urlparse

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

TOKEN = "annotaprobe"


def png_size(path):
    """Width and height from the PNG header; no pixel is read."""
    try:
        with open(path, "rb") as image:
            header = image.read(24)
        if header[:8] != b"\x89PNG\r\n\x1a\n":
            return "not a PNG"
        return "%d x %d" % struct.unpack(">II", header[16:24])
    except OSError as err:
        return "unreadable (%s)" % err


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--interactive",
        action="store_true",
        help="show the desktop's own screenshot UI (what window capture does)",
    )
    parser.add_argument(
        "--timeout", type=int, default=60, help="seconds to wait for an answer (default 60)"
    )
    args = parser.parse_args()

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    loop = GLib.MainLoop()
    unique = bus.get_unique_name().lstrip(":").replace(".", "_")
    handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (unique, TOKEN)
    answered = []
    start = time.monotonic()

    def on_response(_conn, _sender, _path, _iface, _signal, params):
        code, results = params.unpack()
        elapsed = (time.monotonic() - start) * 1000
        answered.append(code)
        meaning = {0: "success", 1: "cancelled by the user", 2: "other"}.get(code, "unknown")
        print("response code %d (%s) after %.0f ms" % (code, meaning, elapsed))
        uri = results.get("uri")
        if uri:
            path = unquote(urlparse(uri).path)
            print("image: %s" % png_size(path))
            try:
                os.remove(path)
                print("deleted the portal's file again")
            except OSError as err:
                print("could not delete %s: %s" % (path, err), file=sys.stderr)
        elif code == 2 and args.interactive:
            print(
                "no file. GNOME's screenshot UI reports `success: false` this way, both when\n"
                "it is closed and when its modal grab failed and it never appeared. The time\n"
                "above is the only hint: milliseconds mean it never opened."
            )
        loop.quit()

    bus.signal_subscribe(
        None,
        "org.freedesktop.portal.Request",
        "Response",
        handle,
        None,
        Gio.DBusSignalFlags.NONE,
        on_response,
    )

    options = {
        "handle_token": GLib.Variant("s", TOKEN),
        "interactive": GLib.Variant("b", args.interactive),
        "modal": GLib.Variant("b", True),
    }
    print("Screenshot(interactive=%s, modal=True), no parent window" % args.interactive)
    try:
        bus.call_sync(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Screenshot",
            "Screenshot",
            GLib.Variant("(sa{sv})", ("", options)),
            GLib.VariantType("(o)"),
            Gio.DBusCallFlags.NONE,
            5000,
            None,
        )
    except GLib.Error as err:
        print("the portal refused the call: %s" % err.message, file=sys.stderr)
        return 1

    GLib.timeout_add_seconds(args.timeout, loop.quit)
    loop.run()
    if not answered:
        print("no answer within %d s" % args.timeout, file=sys.stderr)
        return 1
    return 0 if answered[0] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
