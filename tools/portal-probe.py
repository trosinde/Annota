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
    tools/portal-probe.py --global-shortcuts [--watch 120]
                                          # GlobalShortcuts: the triggers GNOME has bound
    tools/portal-probe.py --screencast-window [--cursor embedded] [--save out.png]
                                          # ScreenCast window chooser: what window capture uses

The elapsed time is the useful part of an interactive run. A UI the user closes answers after
seconds; a UI whose modal grab failed, and which therefore never appeared, answers in
milliseconds. Response code 2 alone does not tell them apart, and neither can Annota.

`--global-shortcuts` registers this process as `io.github.trosinde.Annota` (host Registry,
xdg-desktop-portal >= 1.19) and binds Annota's three IDs with Annota's configured accelerators as
preferred triggers -- exactly what Annota does at every start. It prints the bind response and
`ListShortcuts`, then closes the session. GNOME keeps the triggers it stored for an ID it has seen
before; this mode shows which ones those are. With `--watch N` it keeps the session open for N
seconds, prints every `ShortcutsChanged` signal and lists the shortcuts again every 5 s, so a
rebind in GNOME Settings can be watched. Annota itself should not be running meanwhile: GNOME
delivers a pressed shortcut to every session bound to it.

`--screencast-window` does what window capture does (ADR 0002): a ScreenCast session with
source type WINDOW and no restore token, one frame through GStreamer, session closed. It prints
the response code and time of `Start` (close the chooser to see what "closed" answers), the
stream's reported size, the buffer size, whether the buffer carries a `GstVideoCropMeta`, and the
bounding box of its non-transparent pixels. The frame is dropped unless `--save PATH` is given,
which writes the cut-out window as PNG for a visual check (e.g. of the pointer with
`--cursor embedded`).

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


APP_ID = "io.github.trosinde.Annota"
SHORTCUT_IDS = (
    ("region", "Region capture", "shortcut-region", "Print"),
    ("window", "Window capture", "shortcut-window", "<Alt>Print"),
    ("screen", "Screen capture", "shortcut-screen", "<Shift>Print"),
)
XDG_MODIFIERS = {
    "shift": "SHIFT",
    "control": "CTRL",
    "ctrl": "CTRL",
    "primary": "CTRL",
    "alt": "ALT",
    "super": "LOGO",
}


def xdg_trigger(accel):
    """GTK accelerator to the XDG shortcuts-spec trigger, as `Accel::to_xdg_trigger` does."""
    mods = []
    rest = accel.strip()
    while rest.startswith("<") and ">" in rest:
        name, rest = rest[1:].split(">", 1)
        mod = XDG_MODIFIERS.get(name.lower())
        if mod and mod not in mods:
            mods.append(mod)
    return "+".join(mods + [rest]) if rest else ""


def configured_accel(key, default):
    source = Gio.SettingsSchemaSource.get_default()
    if source is None or source.lookup(APP_ID, True) is None:
        return default
    return Gio.Settings.new(APP_ID).get_string(key)


def global_shortcuts(bus, watch, timeout):
    desktop, path = "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop"
    iface = "org.freedesktop.portal.GlobalShortcuts"
    try:
        bus.call_sync(
            desktop, path, "org.freedesktop.host.portal.Registry", "Register",
            GLib.Variant("(sa{sv})", (APP_ID, {})), None, Gio.DBusCallFlags.NONE, 5000, None,
        )
    except GLib.Error as err:
        print("host Registry not available (%s); the portal sees no app ID" % err.message)
    try:
        version = bus.call_sync(
            desktop, path, "org.freedesktop.DBus.Properties", "Get",
            GLib.Variant("(ss)", (iface, "version")), None, Gio.DBusCallFlags.NONE, 5000, None,
        ).unpack()[0]
    except GLib.Error as err:
        print("no GlobalShortcuts portal: %s" % err.message, file=sys.stderr)
        return 1
    print("GlobalShortcuts version %d, LANG=%s" % (version, os.environ.get("LANG", "")))
    unique = bus.get_unique_name().lstrip(":").replace(".", "_")
    loop = GLib.MainLoop()
    counter = [0]

    def request(method, params_for):
        counter[0] += 1
        token = "%s%d" % (TOKEN, counter[0])
        handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (unique, token)
        answer = []

        def on_response(_c, _s, _p, _i, _sig, params):
            answer.append(params.unpack())
            loop.quit()

        sub = bus.signal_subscribe(
            None, "org.freedesktop.portal.Request", "Response", handle, None,
            Gio.DBusSignalFlags.NONE, on_response,
        )
        start = time.monotonic()
        bus.call_sync(
            desktop, path, iface, method, params_for(token), None,
            Gio.DBusCallFlags.NONE, 5000, None,
        )
        source = GLib.timeout_add_seconds(timeout, loop.quit)
        loop.run()
        if answer:
            GLib.source_remove(source)
        bus.signal_unsubscribe(sub)
        if not answer:
            print("%s: no answer within %d s" % (method, timeout), file=sys.stderr)
            return None
        print("%s: code %d after %.0f ms" % (method, answer[0][0], (time.monotonic() - start) * 1000))
        return answer[0]

    def show(shortcuts):
        if not shortcuts:
            print("  (no shortcuts)")
        for sid, props in shortcuts:
            print("  %-7s trigger_description=%r" % (sid, props.get("trigger_description", "")))

    created = request("CreateSession", lambda t: GLib.Variant("(a{sv})", ({
        "handle_token": GLib.Variant("s", t),
        "session_handle_token": GLib.Variant("s", t + "s"),
    },)))
    if not created or created[0] != 0:
        return 1
    session = created[1]["session_handle"]
    shortcuts = []
    for sid, description, key, default in SHORTCUT_IDS:
        trigger = xdg_trigger(configured_accel(key, default))
        print("  bind %-7s preferred_trigger=%r" % (sid, trigger))
        props = {"description": GLib.Variant("s", description)}
        if trigger:
            props["preferred_trigger"] = GLib.Variant("s", trigger)
        shortcuts.append((sid, props))
    bound = request("BindShortcuts", lambda t: GLib.Variant(
        "(oa(sa{sv})sa{sv})", (session, shortcuts, "", {"handle_token": GLib.Variant("s", t)})
    ))
    if bound:
        show(bound[1].get("shortcuts", []))

    def list_now():
        listed = request("ListShortcuts", lambda t: GLib.Variant(
            "(oa{sv})", (session, {"handle_token": GLib.Variant("s", t)})
        ))
        if listed:
            show(listed[1].get("shortcuts", []))

    list_now()
    if watch > 0:
        def on_changed(_c, _s, _p, _i, _sig, params):
            print("ShortcutsChanged at +%.0f s:" % (time.monotonic() - watch_start))
            show(params.unpack()[1])

        def on_activated(_c, _s, _p, _i, _sig, params):
            print("Activated %s at +%.0f s" % (params.unpack()[1], time.monotonic() - watch_start))

        bus.signal_subscribe(None, iface, "ShortcutsChanged", path, None,
                             Gio.DBusSignalFlags.NONE, on_changed)
        bus.signal_subscribe(None, iface, "Activated", path, None,
                             Gio.DBusSignalFlags.NONE, on_activated)
        watch_start = time.monotonic()
        end = watch_start + watch
        print("watching for %d s; rebind in GNOME Settings or press a shortcut" % watch)
        while time.monotonic() < end:
            context = GLib.MainContext.default()
            pause = time.monotonic() + 5
            while time.monotonic() < min(pause, end):
                context.iteration(False)
                time.sleep(0.05)
            if time.monotonic() < end:
                list_now()
    bus.call_sync(desktop, session, "org.freedesktop.portal.Session", "Close", None, None,
                  Gio.DBusCallFlags.NONE, 5000, None)
    print("session closed")
    return 0


SOURCE_WINDOW = 2
CURSOR_MODES = {"hidden": 1, "embedded": 2, "metadata": 4}


def alpha_bbox(data, width, height, stride):
    """Bounding box (x, y, w, h) of pixels with alpha > 0 in BGRA data, or None."""
    rows = [y for y in range(height) if any(data[y * stride + 3:y * stride + width * 4:4])]
    if not rows:
        return None
    top, bottom = rows[0], rows[-1]
    left, right = width, -1
    for y in range(top, bottom + 1):
        alphas = data[y * stride + 3:y * stride + width * 4:4]
        if alphas.strip(b"\0"):
            left = min(left, len(alphas) - len(alphas.lstrip(b"\0")))
            right = max(right, len(alphas.rstrip(b"\0")) - 1)
    return (left, top, right - left + 1, bottom - top + 1)


def crop_meta_rect(buf):
    """(x, y, width, height) of the buffer's GstVideoCropMeta through libgstvideo, or a note.

    Avoids the GstVideo typelib, which a desktop need not have installed.
    """
    import ctypes
    import ctypes.util

    name = ctypes.util.find_library("gstvideo-1.0")
    if not name:
        return "present (libgstvideo-1.0 not found to read it)"
    video = ctypes.CDLL(name)
    core = ctypes.CDLL(ctypes.util.find_library("gstreamer-1.0"))
    # gst_buffer_get_video_crop_meta() is a macro; this is what it expands to.
    video.gst_video_crop_meta_api_get_type.restype = ctypes.c_size_t
    core.gst_buffer_get_meta.restype = ctypes.c_void_p
    core.gst_buffer_get_meta.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    # PyGObject's PyGBoxed: PyObject_HEAD, then the boxed pointer (CPython, default build).
    pointer = ctypes.c_void_p.from_address(id(buf) + 2 * ctypes.sizeof(ctypes.c_void_p)).value
    meta = core.gst_buffer_get_meta(pointer, video.gst_video_crop_meta_api_get_type())
    if not meta:
        return "none"

    class CropMeta(ctypes.Structure):
        _fields_ = [("flags", ctypes.c_uint), ("info", ctypes.c_void_p),
                    ("x", ctypes.c_uint), ("y", ctypes.c_uint),
                    ("width", ctypes.c_uint), ("height", ctypes.c_uint)]

    rect = CropMeta.from_address(meta)
    return "(%d, %d, %d, %d)" % (rect.x, rect.y, rect.width, rect.height)


def screencast_window(bus, cursor, save, timeout):
    gi.require_version("Gst", "1.0")
    from gi.repository import Gst  # noqa: E402

    Gst.init(None)
    desktop, path = "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop"
    iface = "org.freedesktop.portal.ScreenCast"
    try:
        bus.call_sync(
            desktop, path, "org.freedesktop.host.portal.Registry", "Register",
            GLib.Variant("(sa{sv})", (APP_ID, {})), None, Gio.DBusCallFlags.NONE, 5000, None,
        )
    except GLib.Error as err:
        print("host Registry not available (%s)" % err.message)
    props = bus.call_sync(
        desktop, path, "org.freedesktop.DBus.Properties", "GetAll", GLib.Variant("(s)", (iface,)),
        None, Gio.DBusCallFlags.NONE, 5000, None,
    ).unpack()[0]
    print("ScreenCast %s" % props)
    if not props.get("AvailableSourceTypes", 0) & SOURCE_WINDOW:
        print("no window source: Annota falls back to the Screenshot UI")
        return 1
    unique = bus.get_unique_name().lstrip(":").replace(".", "_")
    loop = GLib.MainLoop()
    counter = [0]

    def request(method, params_for):
        counter[0] += 1
        token = "%s%d" % (TOKEN, counter[0])
        handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (unique, token)
        answer = []

        def on_response(_c, _s, _p, _i, _sig, params):
            answer.append(params.unpack())
            loop.quit()

        sub = bus.signal_subscribe(
            None, "org.freedesktop.portal.Request", "Response", handle, None,
            Gio.DBusSignalFlags.NONE, on_response,
        )
        start = time.monotonic()
        bus.call_sync(desktop, path, iface, method, params_for(token), None,
                      Gio.DBusCallFlags.NONE, 5000, None)
        source = GLib.timeout_add_seconds(timeout, loop.quit)
        loop.run()
        if answer:
            GLib.source_remove(source)
        bus.signal_unsubscribe(sub)
        elapsed = (time.monotonic() - start) * 1000
        if not answer:
            print("%s: no answer within %d s" % (method, timeout))
            return None
        code = answer[0][0]
        meaning = {0: "success", 1: "cancelled by the user", 2: "other"}.get(code, "unknown")
        print("%s: code %d (%s) after %.0f ms" % (method, code, meaning, elapsed))
        return answer[0]

    created = request("CreateSession", lambda t: GLib.Variant("(a{sv})", ({
        "handle_token": GLib.Variant("s", t),
        "session_handle_token": GLib.Variant("s", t + "s"),
    },)))
    if not created or created[0] != 0:
        return 1
    session = created[1]["session_handle"]

    def close():
        bus.call_sync(desktop, session, "org.freedesktop.portal.Session", "Close", None, None,
                      Gio.DBusCallFlags.NONE, 5000, None)
        print("session closed")

    selected = request("SelectSources", lambda t: GLib.Variant("(oa{sv})", (session, {
        "handle_token": GLib.Variant("s", t),
        "types": GLib.Variant("u", SOURCE_WINDOW),
        "multiple": GLib.Variant("b", False),
        "cursor_mode": GLib.Variant("u", CURSOR_MODES[cursor]),
        "persist_mode": GLib.Variant("u", 0),
    })))
    if not selected or selected[0] != 0:
        close()
        return 1
    print("Start: choose a window in the desktop's chooser (or close it)")
    started = request("Start", lambda t: GLib.Variant(
        "(osa{sv})", (session, "", {"handle_token": GLib.Variant("s", t)})
    ))
    if not started or started[0] != 0:
        close()
        return 1
    node, stream = started[1]["streams"][0]
    print("stream node %d %s" % (node, stream))
    reply, fds = bus.call_with_unix_fd_list_sync(
        desktop, path, iface, "OpenPipeWireRemote", GLib.Variant("(oa{sv})", (session, {})),
        GLib.VariantType("(h)"), Gio.DBusCallFlags.NONE, 5000, None, None,
    )
    fd = fds.get(reply.unpack()[0])
    pipeline = Gst.parse_launch(
        "pipewiresrc fd=%d path=%d num-buffers=1 always-copy=true ! videoconvert ! "
        "video/x-raw,format=BGRA ! appsink name=sink sync=false" % (fd, node)
    )
    sink = pipeline.get_by_name("sink")
    start = time.monotonic()
    pipeline.set_state(Gst.State.PLAYING)
    sample = sink.emit("try-pull-sample", 5 * Gst.SECOND)
    print("frame: %s after %.0f ms" % ("yes" if sample else "NONE", (time.monotonic() - start) * 1000))
    if sample:
        buf = sample.get_buffer()
        caps = sample.get_caps().get_structure(0)
        width, height = caps.get_value("width"), caps.get_value("height")
        # Presence only: reading its fields needs the GstVideo typelib
        # (gir1.2-gst-plugins-base-1.0), which a desktop need not have.
        crop = buf.get_meta("GstVideoCropMetaAPI")
        ok, mapped = buf.map(Gst.MapFlags.READ)
        data = bytes(mapped.data)
        buf.unmap(mapped)
        stride = len(data) // height
        print("buffer %d x %d, stride %d" % (width, height, stride))
        print("crop meta: %s" % (crop_meta_rect(buf) if crop else "none"))
        box = alpha_bbox(data, width, height, stride)
        print("alpha bounding box: %s" % (box,))
        if save and box:
            gi.require_version("GdkPixbuf", "2.0")
            from gi.repository import GdkPixbuf  # noqa: E402
            x, y, w, h = box
            rgba = bytearray()
            for row in range(y, y + h):
                line = data[row * stride + x * 4:row * stride + (x + w) * 4]
                px = bytearray(line)
                px[0::4], px[2::4] = line[2::4], line[0::4]
                rgba += px
            GdkPixbuf.Pixbuf.new_from_bytes(
                GLib.Bytes.new(bytes(rgba)), GdkPixbuf.Colorspace.RGB, True, 8, w, h, w * 4
            ).savev(save, "png", [], [])
            print("saved the window to %s" % save)
    pipeline.set_state(Gst.State.NULL)
    os.close(fd)
    close()
    return 0 if sample else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--interactive",
        action="store_true",
        help="show the desktop's own screenshot UI (what window capture does)",
    )
    parser.add_argument(
        "--global-shortcuts",
        action="store_true",
        help="bind Annota's shortcut IDs and print the triggers the desktop reports",
    )
    parser.add_argument(
        "--screencast-window",
        action="store_true",
        help="pick a window through the ScreenCast portal and take one frame",
    )
    parser.add_argument(
        "--cursor", choices=sorted(CURSOR_MODES), default="hidden",
        help="with --screencast-window: cursor_mode to ask for (default hidden)",
    )
    parser.add_argument(
        "--save", help="with --screencast-window: write the cut-out window to this PNG"
    )
    parser.add_argument(
        "--watch", type=int, default=0, help="with --global-shortcuts: keep listening N seconds"
    )
    parser.add_argument(
        "--timeout", type=int, default=60, help="seconds to wait for an answer (default 60)"
    )
    args = parser.parse_args()

    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    if args.screencast_window:
        return screencast_window(bus, args.cursor, args.save, args.timeout)
    if args.global_shortcuts:
        return global_shortcuts(bus, args.watch, args.timeout)
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
