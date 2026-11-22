#!/usr/bin/env python3
"""Measures what screen recording assumes about the desktop (add-screen-recording, probes P1-P12).

The `add-screen-recording` spec describes only what Annota sends and how it handles each answer.
Everything the real GNOME session does in reply is an assumption until it is measured here
(CLAUDE.md §5, "Assumptions about the portal are measured"). Annota's own tests answer every
portal call themselves and can never catch a wrong assumption about the real portal. This tool
needs no Annota build. It follows `tools/portal-probe.py`, which measured window stills.

Run it on the desktop under test, with Annota NOT running (GNOME delivers shortcuts to every
bound session, and only one screen-sharing session should be open). Each probe says what it is
about to do and which dialogs will appear, then waits for Enter (or `s` to skip).

    tools/recording-probe.py env                  # versions, portal properties, GStreamer elements
    tools/recording-probe.py audio                # microphone + system audio (no portal)
    tools/recording-probe.py monitor              # P1 P2 P3 P10: screen chooser, stream geometry
    tools/recording-probe.py monitor --expect cancel   # P1: what "Cancel" in the chooser answers
    tools/recording-probe.py monitor --cursor embedded --save /tmp/frame.png   # P4 P12
    tools/recording-probe.py cursor-modes         # P4: Start with every offered cursor mode
    tools/recording-probe.py session-close        # P5: stop sharing from GNOME's own indicator
    tools/recording-probe.py window-end           # P6: resize, then close the recorded window
    tools/recording-probe.py frame-rate           # P7: buffers per second on a still screen
    tools/recording-probe.py load --minutes 30    # P8: 4K-capable encode load, nothing written
    tools/recording-probe.py shortcuts            # P11: a 4th GlobalShortcuts ID `record`
    tools/recording-probe.py all                  # everything except `load`, one after another

P9 (X11) is the same `env`, `monitor` and `window-end` run in a "GNOME on Xorg" session, where
the desktop still offers one (Ubuntu 24.04; GNOME 50 has no X11 session any more).

Dialogs and indicators you will see:
- GNOME's screen-sharing chooser ("Screen Share"/"Share Screen") for `monitor`, `cursor-modes`,
  `session-close`, `frame-rate`, `load` (monitor tab) and `window-end` (window tab). Nothing is
  shared with anyone: the stream goes only into this process.
- GNOME's screen-sharing indicator in the top bar while a stream is open.
- With `--parent` (default on), a small window "Annota recording probe" that is the chooser's
  parent, as Annota's waiting indicator will be.
- `audio`: no dialog; GNOME's microphone indicator appears for about 3 s.
- `shortcuts`: GNOME may show its "Allow Annota to use global shortcuts?" dialog. It binds the
  IDs region, window, screen AND record under Annota's app ID, exactly as Annota will at start,
  and GNOME keeps what it stores. This is the one probe with a lasting effect.

Privacy: no frame and no audio is written anywhere, except the single frame `--save` writes for
a visual check (P4 pointer, P12 indicator). Do not commit that file. The log contains only
numbers, sizes, codes and timings.

Every run appends its result table to a markdown log, by default
`openspec/changes/add-screen-recording/probe-results/<date>-<os>-<session>.md`, which can be
committed as the evidence for task 2.1.
"""

import argparse
import datetime
import importlib.util
import os
import pathlib
import resource
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
APP_ID = "io.github.trosinde.Annota"
TOKEN = "annotarecprobe"
DESKTOP = "org.freedesktop.portal.Desktop"
PORTAL_PATH = "/org/freedesktop/portal/desktop"
SCREENCAST = "org.freedesktop.portal.ScreenCast"
SOURCE_MONITOR, SOURCE_WINDOW = 1, 2
CURSOR_MODES = {"hidden": 1, "embedded": 2, "metadata": 4}
CODES = {0: "success", 1: "cancelled by the user", 2: "other"}
# The elements of "Recording runtime dependencies" (debian-packaging delta).
ELEMENTS = ("pipewiresrc", "pulsesrc", "level", "audiomixer", "videocrop", "videorate",
            "videoscale", "videoconvert", "x264enc", "opusenc", "matroskamux", "mp4mux",
            "vp9enc", "webmmux", "voaacenc", "h264parse", "matroskademux")
RECORD_TRIGGER = "SHIFT+CTRL+Print"

GLib = Gio = Gst = None  # imported lazily, so `--help` touches nothing


def load_gi(gst=False):
    global GLib, Gio, Gst
    import gi

    gi.require_version("Gio", "2.0")
    from gi.repository import Gio as _Gio, GLib as _GLib

    GLib, Gio = _GLib, _Gio
    if gst:
        gi.require_version("Gst", "1.0")
        from gi.repository import Gst as _Gst

        _Gst.init(None)
        Gst = _Gst


def portal_probe_module():
    """`tools/portal-probe.py` as a module, for its frame helpers."""
    spec = importlib.util.spec_from_file_location("portal_probe", ROOT / "tools/portal-probe.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# ---------------------------------------------------------------------------------------------
# Log


class Log:
    def __init__(self, path):
        self.path = path
        self.rows = []
        self.notes = []

    def row(self, probe, item, value):
        value = str(value)
        self.rows.append((probe, item, value))
        print("  [%s] %-38s %s" % (probe, item, value))

    def note(self, text):
        self.notes.append(text)
        print("  note: %s" % text)

    def write(self, header):
        if not self.rows and not self.notes:
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        new = not self.path.exists()
        with open(self.path, "a", encoding="utf-8") as out:
            if new:
                out.write("# Recording probe results\n\nWritten by `tools/recording-probe.py`. "
                          "Probes P1-P12: `openspec/changes/add-screen-recording/design.md`.\n")
            out.write("\n## Run %s\n\n" % header)
            out.write("| Probe | Item | Result |\n|---|---|---|\n")
            for probe, item, value in self.rows:
                out.write("| %s | %s | %s |\n" % (probe, item, value.replace("|", "\\|")))
            if self.notes:
                out.write("\nNotes:\n\n")
                out.writelines("- %s\n" % n for n in self.notes)
        print("\nresults appended to %s" % self.path)


def ask(text, dialogs):
    """Says what happens next; returns False if the user skips."""
    print("\n" + "=" * 78)
    print(text)
    print("Dialogs: %s" % dialogs)
    try:
        answer = input("Enter = start, s = skip: ").strip().lower()
    except EOFError:
        return False
    return answer != "s"


def run_cmd(*argv):
    try:
        return subprocess.run(argv, capture_output=True, text=True, timeout=10).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""


def os_tag():
    fields = {}
    try:
        for line in open("/etc/os-release", encoding="utf-8"):
            key, _, value = line.strip().partition("=")
            fields[key] = value.strip('"')
    except OSError:
        pass
    return "%s%s" % (fields.get("ID", "linux"), fields.get("VERSION_ID", ""))


# ---------------------------------------------------------------------------------------------
# Portal plumbing (same request pattern as tools/portal-probe.py)


class Portal:
    def __init__(self, timeout):
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.timeout = timeout
        self.counter = 0
        self.unique = self.bus.get_unique_name().lstrip(":").replace(".", "_")
        try:
            self.bus.call_sync(DESKTOP, PORTAL_PATH, "org.freedesktop.host.portal.Registry",
                               "Register", GLib.Variant("(sa{sv})", (APP_ID, {})), None,
                               Gio.DBusCallFlags.NONE, 5000, None)
        except GLib.Error as err:
            print("host Registry not available (%s); the portal sees no app ID" % err.message)

    def properties(self, iface):
        try:
            return self.bus.call_sync(
                DESKTOP, PORTAL_PATH, "org.freedesktop.DBus.Properties", "GetAll",
                GLib.Variant("(s)", (iface,)), None, Gio.DBusCallFlags.NONE, 5000, None,
            ).unpack()[0]
        except GLib.Error as err:
            return {"error": err.message}

    def request(self, iface, method, params_for):
        """(code, results, elapsed ms) or None when no answer came in time."""
        self.counter += 1
        token = "%s%d" % (TOKEN, self.counter)
        handle = "%s/request/%s/%s" % (PORTAL_PATH, self.unique, token)
        loop = GLib.MainLoop()
        answer = []

        def on_response(_c, _s, _p, _i, _sig, params):
            answer.append(params.unpack())
            loop.quit()

        sub = self.bus.signal_subscribe(None, "org.freedesktop.portal.Request", "Response",
                                        handle, None, Gio.DBusSignalFlags.NONE, on_response)
        start = time.monotonic()
        try:
            self.bus.call_sync(DESKTOP, PORTAL_PATH, iface, method, params_for(token), None,
                               Gio.DBusCallFlags.NONE, 5000, None)
        except GLib.Error as err:
            self.bus.signal_unsubscribe(sub)
            print("%s refused: %s" % (method, err.message))
            return None
        source = GLib.timeout_add_seconds(self.timeout, loop.quit)
        loop.run()
        if answer:
            GLib.source_remove(source)
        self.bus.signal_unsubscribe(sub)
        elapsed = (time.monotonic() - start) * 1000
        if not answer:
            print("%s: no answer within %d s" % (method, self.timeout))
            return None
        code, results = answer[0]
        print("%s: code %d (%s) after %.0f ms" % (method, code, CODES.get(code, "?"), elapsed))
        return code, results, elapsed


class Parent:
    """A small GTK window whose exported handle is the chooser's parent (P10)."""

    def __init__(self, enabled):
        self.handle = ""
        self.window = None
        if not enabled:
            return
        try:
            import gi

            gi.require_version("Gtk", "4.0")
            from gi.repository import Gtk

            for backend in ("GdkWayland", "GdkX11"):  # load the backend's surface methods
                try:
                    gi.require_version(backend, "4.0")
                    __import__("gi.repository." + backend)
                except (ValueError, ImportError):
                    pass
            Gtk.init()
            self.window = Gtk.Window(title="Annota recording probe")
            self.window.set_default_size(320, 80)
            self.window.set_child(Gtk.Label(label="Waiting for the desktop's chooser…"))
            self.window.present()
            context = GLib.MainContext.default()
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline and not self._export():
                context.iteration(False)
                time.sleep(0.02)
        except Exception as err:  # noqa: BLE001 - a probe reports, it does not fail here
            print("no parent window (%s)" % err)

    def _export(self):
        surface = self.window.get_surface()
        if surface is None:
            return False
        name = type(surface).__name__
        if "Wayland" in name:
            if not getattr(self, "_asked", False):
                self._asked = True

                def got(_toplevel, handle, *_):
                    self.handle = "wayland:" + handle

                surface.export_handle(got)
            return bool(self.handle)
        if "X11" in name:
            self.handle = "x11:%x" % surface.get_xid()
            return True
        return False

    def close(self):
        if self.window is not None:
            self.window.destroy()


class ScreenCast:
    def __init__(self, portal, log, probe):
        self.portal, self.log, self.probe = portal, log, probe
        self.session = None
        self.closed_at = None
        self._closed_sub = None

    def open(self, source_type, cursor, parent_handle, expect):
        created = self.portal.request(SCREENCAST, "CreateSession", lambda t: GLib.Variant(
            "(a{sv})", ({"handle_token": GLib.Variant("s", t),
                         "session_handle_token": GLib.Variant("s", t + "s")},)))
        if not created or created[0] != 0:
            self.log.row(self.probe, "CreateSession", created and created[0])
            return None
        self.session = created[1]["session_handle"]

        def on_closed(*_):
            self.closed_at = time.monotonic()
            print("Session.Closed received")

        self._closed_sub = self.portal.bus.signal_subscribe(
            None, "org.freedesktop.portal.Session", "Closed", self.session, None,
            Gio.DBusSignalFlags.NONE, on_closed)
        selected = self.portal.request(SCREENCAST, "SelectSources", lambda t: GLib.Variant(
            "(oa{sv})", (self.session, {
                "handle_token": GLib.Variant("s", t),
                "types": GLib.Variant("u", source_type),
                "multiple": GLib.Variant("b", False),
                "cursor_mode": GLib.Variant("u", CURSOR_MODES[cursor]),
                "persist_mode": GLib.Variant("u", 0),
            })))
        self.log.row(self.probe, "SelectSources code", selected and selected[0])
        if not selected or selected[0] != 0:
            self.close()
            return None
        what = "a screen" if source_type == SOURCE_MONITOR else "a window"
        if expect == "cancel":
            print(">>> In GNOME's chooser, press Cancel (or Esc) WITHOUT choosing.")
        else:
            print(">>> In GNOME's chooser, choose %s and confirm with Share." % what)
        started = self.portal.request(SCREENCAST, "Start", lambda t: GLib.Variant(
            "(osa{sv})", (self.session, parent_handle, {"handle_token": GLib.Variant("s", t)})))
        if started is None:
            self.log.row(self.probe, "Start", "no answer within %d s" % self.portal.timeout)
            self.close()
            return None
        code, results, elapsed = started
        self.log.row(self.probe, "Start code (%s)" % expect,
                     "%d %s after %.0f ms" % (code, CODES.get(code, "?"), elapsed))
        if code != 0:
            self.close()
            return None
        streams = results.get("streams", [])
        self.log.row(self.probe, "streams", len(streams))
        node, props = streams[0]
        for key in ("source_type", "position", "size", "id", "mapping_id"):
            self.log.row(self.probe, "stream %s" % key, props.get(key, "(absent)"))
        return node, props

    def remote_fd(self):
        reply, fds = self.portal.bus.call_with_unix_fd_list_sync(
            DESKTOP, PORTAL_PATH, SCREENCAST, "OpenPipeWireRemote",
            GLib.Variant("(oa{sv})", (self.session, {})), GLib.VariantType("(h)"),
            Gio.DBusCallFlags.NONE, 5000, None, None)
        return fds.get(reply.unpack()[0])

    def close(self):
        if self.session and self.closed_at is None:
            try:
                self.portal.bus.call_sync(DESKTOP, self.session, "org.freedesktop.portal.Session",
                                          "Close", None, None, Gio.DBusCallFlags.NONE, 5000, None)
                print("session closed")
            except GLib.Error as err:
                print("Session.Close failed: %s" % err.message)
        if self._closed_sub is not None:
            self.portal.bus.signal_unsubscribe(self._closed_sub)
            self._closed_sub = None
        self.session = None


def gdk_monitors():
    """[(connector, logical x, y, w, h, scale)] of the current display, or an error string."""
    try:
        import gi

        gi.require_version("Gtk", "4.0")
        from gi.repository import Gtk

        Gtk.init()
        from gi.repository import Gdk

        model = Gdk.Display.get_default().get_monitors()
        result = []
        for i in range(model.get_n_items()):
            mon = model.get_item(i)
            g = mon.get_geometry()
            scale = mon.get_scale() if hasattr(mon, "get_scale") else mon.get_scale_factor()
            result.append((mon.get_connector(), g.x, g.y, g.width, g.height, scale))
        return result
    except Exception as err:  # noqa: BLE001
        return "unavailable (%s)" % err


class Stream:
    """pipewiresrc into an appsink that only counts and measures; nothing is kept."""

    def __init__(self, fd, node, keepalive=0, extra=""):
        src = "pipewiresrc fd=%d path=%d always-copy=true" % (fd, node)
        if keepalive:
            src += " keepalive-time=%d" % keepalive
        self.pipeline = Gst.parse_launch(
            "%s ! videoconvert ! %s video/x-raw,format=BGRA ! appsink name=sink "
            "emit-signals=true sync=false max-buffers=2 drop=true" % (src, extra))
        self.sink = self.pipeline.get_by_name("sink")
        self.times, self.caps_seen, self.first = [], [], None
        self.end = None  # ("eos"|"error", message, monotonic)
        self.sink.connect("new-sample", self._on_sample)
        bus = self.pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self._on_message)

    def _on_sample(self, sink):
        sample = sink.emit("pull-sample")
        now = time.monotonic()
        self.times.append(now)
        s = sample.get_caps().get_structure(0)
        size = (s.get_value("width"), s.get_value("height"))
        if not self.caps_seen or self.caps_seen[-1][0] != size:
            self.caps_seen.append((size, now))
        if self.first is None:
            self.first = sample
        return Gst.FlowReturn.OK

    def _on_message(self, _bus, msg):
        if msg.type == Gst.MessageType.EOS and self.end is None:
            self.end = ("eos", "", time.monotonic())
        elif msg.type == Gst.MessageType.ERROR and self.end is None:
            err, _dbg = msg.parse_error()
            self.end = ("error", err.message, time.monotonic())

    def run(self, seconds, stop_when=lambda: False):
        self.pipeline.set_state(Gst.State.PLAYING)
        context = GLib.MainContext.default()
        start = time.monotonic()
        deadline = start + seconds
        while time.monotonic() < deadline and self.end is None and not stop_when():
            context.iteration(False)
            time.sleep(0.02)
        return start

    def stop(self):
        self.pipeline.set_state(Gst.State.NULL)


# ---------------------------------------------------------------------------------------------
# Probes


def probe_env(args, log):
    load_gi(gst=True)
    log.row("env", "XDG_SESSION_TYPE", os.environ.get("XDG_SESSION_TYPE", ""))
    log.row("env", "XDG_CURRENT_DESKTOP", os.environ.get("XDG_CURRENT_DESKTOP", ""))
    log.row("env", "os", os_tag())
    log.row("env", "gnome-shell", run_cmd("gnome-shell", "--version") or "?")
    for pkg in ("xdg-desktop-portal", "xdg-desktop-portal-gnome", "pipewire",
                "gstreamer1.0-pipewire"):
        log.row("env", pkg, run_cmd("dpkg-query", "-W", "-f=${Version}", pkg) or "not installed")
    log.row("env", "GStreamer", Gst.version_string())
    portal = Portal(args.timeout)
    props = portal.properties(SCREENCAST)
    for key in ("version", "AvailableSourceTypes", "AvailableCursorModes", "error"):
        if key in props:
            log.row("env", "ScreenCast %s" % key, props[key])
    shortcuts = portal.properties("org.freedesktop.portal.GlobalShortcuts")
    log.row("env", "GlobalShortcuts version", shortcuts.get("version", shortcuts.get("error")))
    log.row("env", "monitors (connector, x, y, w, h, scale)", gdk_monitors())
    for name in ELEMENTS:
        factory = Gst.ElementFactory.find(name)
        if factory is None:
            log.row("env", "element %s" % name, "MISSING")
            continue
        plugin = factory.get_plugin()
        filename = plugin.get_filename() if plugin else ""
        owner = run_cmd("dpkg", "-S", filename).split(":")[0] if filename else ""
        log.row("env", "element %s" % name, "%s (%s)" % (owner or "?", plugin.get_license()))
    return 0


def probe_audio(args, log):
    load_gi(gst=True)
    log.row("audio", "pactl default source", run_cmd("pactl", "get-default-source") or "?")
    log.row("audio", "pactl default sink", run_cmd("pactl", "get-default-sink") or "?")
    monitor = Gst.DeviceMonitor()
    monitor.add_filter("Audio/Source", None)
    monitor.start()
    devices = monitor.get_devices()
    monitor.stop()
    log.row("audio", "Audio/Source devices", len(devices))
    for dev in devices:
        log.row("audio", "device", "%s [%s]" % (dev.get_display_name(), dev.get_device_class()))
    if not ask("Records 3 s from the default microphone, then 3 s from @DEFAULT_MONITOR@ "
               "(system audio). Only levels are measured; nothing is written. Play some sound "
               "for the second part.", "none; GNOME's microphone indicator appears briefly"):
        return 0
    for label, device in (("microphone", ""), ("system audio", "@DEFAULT_MONITOR@")):
        src = "pulsesrc" + (" device=%s" % device if device else "")
        pipeline = Gst.parse_launch(
            "%s ! audioconvert ! level name=lvl interval=200000000 post-messages=true ! "
            "fakesink sync=false" % src)
        peaks, errors = [], []
        bus = pipeline.get_bus()
        pipeline.set_state(Gst.State.PLAYING)
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            msg = bus.timed_pop(100 * Gst.MSECOND)
            if msg is None:
                continue
            if msg.type == Gst.MessageType.ERROR:
                errors.append(msg.parse_error()[0].message)
                break
            s = msg.get_structure()
            if s and s.get_name() == "level":
                peaks.append(max(s.get_value("peak")))
        pipeline.set_state(Gst.State.NULL)
        if errors:
            log.row("audio", "%s (%s)" % (label, src), "ERROR " + errors[0])
        else:
            log.row("audio", "%s (%s)" % (label, src), "%d level messages, max peak %.1f dB" % (
                len(peaks), max(peaks) if peaks else float("-inf")))
    return 0


def frame_facts(sample, log, probe, props, monitors):
    pp = portal_probe_module()
    buf = sample.get_buffer()
    s = sample.get_caps().get_structure(0)
    width, height = s.get_value("width"), s.get_value("height")
    log.row(probe, "buffer size", "%d x %d" % (width, height))
    ok, rate = s.get_fraction("framerate")[1:] if s.has_field("framerate") else (0, 0)
    log.row(probe, "caps framerate", "%s/%s" % (ok, rate) if s.has_field("framerate") else "none")
    crop = buf.get_meta("GstVideoCropMetaAPI")
    log.row(probe, "crop meta", pp.crop_meta_rect(buf) if crop else "none")
    size = props.get("size")
    if size:
        log.row(probe, "buffer / stream size", "%.3f x %.3f" % (width / size[0], height / size[1]))
    if isinstance(monitors, list):
        position = props.get("position")
        for conn, x, y, w, h, scale in monitors:
            if position and tuple(position) == (x, y):
                log.row(probe, "matches GDK monitor", "%s logical %dx%d scale %s" % (
                    conn, w, h, scale))
    return width, height


def save_frame(sample, path, log, probe):
    import gi

    gi.require_version("GdkPixbuf", "2.0")
    from gi.repository import GdkPixbuf

    buf = sample.get_buffer()
    s = sample.get_caps().get_structure(0)
    width, height = s.get_value("width"), s.get_value("height")
    ok, mapped = buf.map(Gst.MapFlags.READ)
    data = bytes(mapped.data)
    buf.unmap(mapped)
    stride = len(data) // height
    rgba = bytearray()
    for row in range(height):
        line = data[row * stride:row * stride + width * 4]
        px = bytearray(line)
        px[0::4], px[2::4] = line[2::4], line[0::4]
        rgba += px
    GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(bytes(rgba)), GdkPixbuf.Colorspace.RGB,
                                    True, 8, width, height, width * 4).savev(path, "png", [], [])
    log.note("%s: one frame saved to %s for a visual check (pointer? top-bar indicator?). "
             "Do not commit it." % (probe, path))


def open_stream(args, log, probe, source_type, cursor="hidden", expect="choose"):
    portal = Portal(args.timeout)
    parent = Parent(args.parent)
    log.row(probe, "parent window", parent.handle or "none")
    cast = ScreenCast(portal, log, probe)
    opened = cast.open(source_type, cursor, parent.handle, expect)
    parent.close()
    return portal, cast, opened


def probe_monitor(args, log):
    load_gi(gst=True)
    probe = "P1/P2/P3/P10" if args.cursor == "hidden" else "P4/P12"
    monitors = gdk_monitors()
    log.row(probe, "GDK monitors", monitors)
    if not ask("Opens a ScreenCast session for ONE SCREEN (types=MONITOR, multiple=false, "
               "cursor_mode=%s, no restore token) and takes frames for 3 s. %s" % (
                   args.cursor, "Press Cancel in the chooser." if args.expect == "cancel"
                   else "Choose a screen in the chooser."),
               "GNOME's screen-sharing chooser (monitor list), then GNOME's sharing indicator; "
               "with --parent a small 'Annota recording probe' window"):
        return 0
    _portal, cast, opened = open_stream(args, log, probe, SOURCE_MONITOR, args.cursor, args.expect)
    if not opened:
        return 0
    node, props = opened
    stream = Stream(cast.remote_fd(), node)
    start = stream.run(3)
    stream.stop()
    if stream.first is None:
        log.row(probe, "first frame", "NONE within 3 s (%s)" % (stream.end,))
    else:
        log.row(probe, "first frame after", "%.0f ms" % ((stream.times[0] - start) * 1000))
        frame_facts(stream.first, log, probe, props, monitors)
        if args.save:
            save_frame(stream.first, args.save, log, probe)
    log.row(probe, "buffers in 3 s", len(stream.times))
    cast.close()
    return 0


def probe_cursor_modes(args, log):
    load_gi(gst=True)
    portal = Portal(args.timeout)
    offered = portal.properties(SCREENCAST).get("AvailableCursorModes", 0)
    log.row("P4", "AvailableCursorModes", "%d (%s)" % (offered, ", ".join(
        n for n, bit in CURSOR_MODES.items() if offered & bit)))
    for name, bit in sorted(CURSOR_MODES.items(), key=lambda kv: kv[1]):
        if not offered & bit:
            log.row("P4", "cursor_mode %s" % name, "not offered, skipped")
            continue
        if not ask("ScreenCast for one screen with cursor_mode=%s. Choose a screen, keep the "
                   "pointer over it. A frame is saved to /tmp/annota-probe-cursor-%s.png for "
                   "you to look at (pointer drawn in or not)." % (name, name),
                   "GNOME's screen-sharing chooser, then the sharing indicator"):
            continue
        _p, cast, opened = open_stream(args, log, "P4", SOURCE_MONITOR, name)
        if not opened:
            continue
        node, _props = opened
        stream = Stream(cast.remote_fd(), node)
        stream.run(2)
        stream.stop()
        if stream.first is not None:
            save_frame(stream.first, "/tmp/annota-probe-cursor-%s.png" % name, log, "P4")
            meta = [meta_name(m) for m in iter_metas(stream.first.get_buffer())]
            log.row("P4", "cursor_mode %s buffer metas" % name, meta or "none")
        cast.close()
        log.note("P4: open /tmp/annota-probe-cursor-%s.png and record whether the pointer "
                 "is visible." % name)
    return 0


def meta_name(meta):
    try:
        return meta.info.api.name
    except Exception:  # noqa: BLE001
        return type(meta).__name__


def iter_metas(buf):
    metas = []
    try:
        buf.foreach_meta(lambda _b, meta: metas.append(meta) or True)
    except Exception:  # noqa: BLE001 - older PyGObject lacks foreach_meta
        pass
    return metas


def probe_session_close(args, log):
    load_gi(gst=True)
    if not ask("Opens a screen stream and keeps it running for up to %d s. While it runs, STOP "
               "the sharing from GNOME's own UI: the screen-sharing indicator in the top bar / "
               "quick settings ('Stop' or the sharing toggle). The probe records whether "
               "Session.Closed arrives and what the stream does." % args.duration,
               "GNOME's chooser, then the sharing indicator, which you use to stop"):
        return 0
    _p, cast, opened = open_stream(args, log, "P5", SOURCE_MONITOR)
    if not opened:
        return 0
    node, _props = opened
    stream = Stream(cast.remote_fd(), node)
    print(">>> Now stop the sharing from GNOME's top bar / quick settings.")
    start = stream.run(args.duration, stop_when=lambda: cast.closed_at is not None
                       and time.monotonic() - cast.closed_at > 2)
    stream.stop()
    log.row("P5", "Session.Closed", "after %.1f s" % (cast.closed_at - start)
            if cast.closed_at else "not received within %d s" % args.duration)
    log.row("P5", "stream end", "%s %s after %.1f s" % (stream.end[0], stream.end[1],
                                                        stream.end[2] - start)
            if stream.end else "none (buffers just stopped?)")
    if stream.times:
        log.row("P5", "last buffer at", "%.1f s" % (stream.times[-1] - start))
    cast.close()
    return 0


def probe_window_end(args, log):
    load_gi(gst=True)
    if not ask("Opens a WINDOW stream. Choose a window you can close (e.g. a new Text Editor "
               "window). Then, within %d s: RESIZE it a few times, wait 5 s, then CLOSE it. "
               "The probe logs size changes, the end of the stream and Session.Closed." % (
                   args.duration),
               "GNOME's screen-sharing chooser (window list), then the sharing indicator"):
        return 0
    _p, cast, opened = open_stream(args, log, "P6", SOURCE_WINDOW)
    if not opened:
        return 0
    node, props = opened
    stream = Stream(cast.remote_fd(), node)
    print(">>> Resize the window, wait, then close it.")
    start = stream.run(args.duration, stop_when=lambda: cast.closed_at is not None
                       and time.monotonic() - cast.closed_at > 2)
    stream.stop()
    for size, at in stream.caps_seen:
        log.row("P6", "buffer size at %.1f s" % (at - start), "%d x %d" % size)
    if stream.first is not None:
        frame_facts(stream.first, log, "P6", props, None)
    log.row("P6", "stream end", "%s %s after %.1f s" % (stream.end[0], stream.end[1],
                                                        stream.end[2] - start)
            if stream.end else "none")
    log.row("P6", "Session.Closed", "after %.1f s" % (cast.closed_at - start)
            if cast.closed_at else "not received")
    if stream.times:
        log.row("P6", "last buffer at", "%.1f s" % (stream.times[-1] - start))
    cast.close()
    return 0


def per_bucket(times, start, bucket, total):
    counts = [0] * max(1, int(total // bucket))
    for t in times:
        index = int((t - start) // bucket)
        if 0 <= index < len(counts):
            counts[index] += 1
    return counts


def probe_frame_rate(args, log):
    load_gi(gst=True)
    for keepalive in (0, 1000):
        if not ask("Screen stream for %d s, pipewiresrc keepalive-time=%d. Keep the screen "
                   "COMPLETELY STILL for the first half (do not move the pointer), then move "
                   "the pointer or play a video for the second half." % (args.duration, keepalive),
                   "GNOME's screen-sharing chooser, then the sharing indicator"):
            continue
        _p, cast, opened = open_stream(args, log, "P7", SOURCE_MONITOR)
        if not opened:
            continue
        node, _props = opened
        stream = Stream(cast.remote_fd(), node, keepalive=keepalive)
        start = stream.run(args.duration)
        stream.stop()
        counts = per_bucket(stream.times, start, 5, args.duration)
        log.row("P7", "keepalive %d: buffers per 5 s" % keepalive, " ".join(map(str, counts)))
        if stream.first is not None:
            s = stream.first.get_caps().get_structure(0)
            log.row("P7", "caps", s.to_string()[:160])
        cast.close()
    return 0


def rss_mb():
    try:
        for line in open("/proc/self/status", encoding="utf-8"):
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024
    except OSError:
        pass
    return 0.0


def probe_load(args, log):
    load_gi(gst=True)
    if Gst.ElementFactory.find("x264enc") is None:
        log.row("P8", "x264enc", "MISSING - install gstreamer1.0-plugins-ugly first")
        return 1
    if not ask("Screen stream for %d min encoded like a recording (videorate 30 fps, x264enc "
               "speed-preset=superfast) into a fakesink: NOTHING is written. Use the desktop "
               "normally, ideally on the largest monitor, a video playing helps." % args.minutes,
               "GNOME's screen-sharing chooser, then the sharing indicator for the whole run"):
        return 0
    _p, cast, opened = open_stream(args, log, "P8", SOURCE_MONITOR)
    if not opened:
        return 0
    node, _props = opened
    fd = cast.remote_fd()
    pipeline = Gst.parse_launch(
        "pipewiresrc fd=%d path=%d ! videoconvert ! videorate name=rate ! "
        "video/x-raw,framerate=30/1 ! x264enc speed-preset=superfast pass=quant quantizer=20 ! "
        "fakesink name=out sync=false signal-handoffs=true" % (fd, node))
    encoded = [0]
    pipeline.get_by_name("out").connect("handoff", lambda *_: encoded.__setitem__(0, encoded[0] + 1))
    qos, errors = [0], []

    def on_message(_bus, msg):
        if msg.type == Gst.MessageType.QOS:
            qos[0] += 1
        elif msg.type == Gst.MessageType.ERROR:
            errors.append(msg.parse_error()[0].message)

    bus = pipeline.get_bus()
    bus.add_signal_watch()
    bus.connect("message", on_message)
    cpu0 = resource.getrusage(resource.RUSAGE_SELF)
    start = time.monotonic()
    pipeline.set_state(Gst.State.PLAYING)
    context = GLib.MainContext.default()
    next_report = start + 60
    end = start + args.minutes * 60
    while time.monotonic() < end and not errors and cast.closed_at is None:
        context.iteration(False)
        time.sleep(0.05)
        if time.monotonic() >= next_report:
            minute = int((time.monotonic() - start) // 60)
            log.row("P8", "minute %d" % minute, "encoded %d, RSS %.0f MB" % (encoded[0], rss_mb()))
            next_report += 60
    wall = time.monotonic() - start
    cpu1 = resource.getrusage(resource.RUSAGE_SELF)
    rate = pipeline.get_by_name("rate")
    pipeline.set_state(Gst.State.NULL)
    cpu = (cpu1.ru_utime - cpu0.ru_utime) + (cpu1.ru_stime - cpu0.ru_stime)
    log.row("P8", "wall time", "%.0f s" % wall)
    log.row("P8", "encoded frames", "%d (%.1f fps)" % (encoded[0], encoded[0] / max(wall, 1)))
    log.row("P8", "videorate in/out/drop/dup", "%d/%d/%d/%d" % tuple(
        rate.get_property(p) for p in ("in", "out", "drop", "duplicate")))
    log.row("P8", "CPU", "%.0f %% of one core" % (100 * cpu / max(wall, 1)))
    log.row("P8", "RSS at end", "%.0f MB" % rss_mb())
    log.row("P8", "QoS messages / errors", "%d / %s" % (qos[0], errors or "none"))
    cast.close()
    return 0


def normalise(accel):
    """'<Shift><Control>Print' / 'SHIFT+CTRL+Print' -> (frozenset(mods), key)."""
    mods, rest = set(), accel.strip()
    names = {"shift": "shift", "control": "ctrl", "ctrl": "ctrl", "primary": "ctrl",
             "alt": "alt", "super": "super", "logo": "super"}
    while rest.startswith("<") and ">" in rest:
        name, rest = rest[1:].split(">", 1)
        mods.add(names.get(name.lower(), name.lower()))
    parts = rest.split("+")
    for part in parts[:-1]:
        mods.add(names.get(part.lower(), part.lower()))
    return frozenset(mods), parts[-1].lower()


def gnome_holders(trigger):
    """GNOME keybinding settings (read only) whose value is `trigger`."""
    wanted = normalise(trigger)
    source = Gio.SettingsSchemaSource.get_default()
    found = []
    for schema_id in ("org.gnome.shell.keybindings", "org.gnome.settings-daemon.plugins.media-keys",
                      "org.gnome.desktop.wm.keybindings", "org.gnome.mutter.keybindings",
                      "org.gnome.mutter.wayland.keybindings"):
        schema = source.lookup(schema_id, True) if source else None
        if schema is None:
            continue
        settings = Gio.Settings.new(schema_id)
        for key in schema.list_keys():
            value = settings.get_value(key)
            values = value.unpack() if value.get_type_string() == "as" else (
                [value.unpack()] if value.get_type_string() == "s" else [])
            if any(isinstance(v, str) and v and normalise(v) == wanted for v in values):
                found.append("%s %s" % (schema_id, key))
    return found


def probe_shortcuts(args, log):
    load_gi()
    log.row("P11", "GNOME keybindings holding %s" % RECORD_TRIGGER,
            gnome_holders(RECORD_TRIGGER) or "none")
    if not ask("Binds Annota's GlobalShortcuts IDs region, window, screen AND the new `record` "
               "(preferred %s) under app ID %s, prints what GNOME reports, lists again, then "
               "watches %d s: press %s to see whether it is delivered (Activated). GNOME KEEPS "
               "the stored triggers (same as Annota at start). Quit Annota first." % (
                   RECORD_TRIGGER, APP_ID, args.watch, RECORD_TRIGGER),
               "possibly GNOME's 'Allow global shortcuts' dialog listing the 4 shortcuts"):
        return 0
    pp = portal_probe_module()
    portal = Portal(args.timeout)
    iface = "org.freedesktop.portal.GlobalShortcuts"
    created = portal.request(iface, "CreateSession", lambda t: GLib.Variant("(a{sv})", ({
        "handle_token": GLib.Variant("s", t),
        "session_handle_token": GLib.Variant("s", t + "s")},)))
    if not created or created[0] != 0:
        log.row("P11", "CreateSession", created and created[0])
        return 1
    session = created[1]["session_handle"]
    shortcuts = []
    for sid, description, key, default in pp.SHORTCUT_IDS:
        trigger = pp.xdg_trigger(pp.configured_accel(key, default))
        props = {"description": GLib.Variant("s", description)}
        if trigger:
            props["preferred_trigger"] = GLib.Variant("s", trigger)
        shortcuts.append((sid, props))
    shortcuts.append(("record", {"description": GLib.Variant("s", "Record video"),
                                 "preferred_trigger": GLib.Variant("s", RECORD_TRIGGER)}))
    bound = portal.request(iface, "BindShortcuts", lambda t: GLib.Variant(
        "(oa(sa{sv})sa{sv})", (session, shortcuts, "", {"handle_token": GLib.Variant("s", t)})))
    if bound:
        log.row("P11", "BindShortcuts", "code %d after %.0f ms (long = a dialog was shown)" % (
            bound[0], bound[2]))
        for sid, props in bound[1].get("shortcuts", []):
            log.row("P11", "bound %s" % sid, props.get("trigger_description", "(none)"))
    activated = []
    sub = portal.bus.signal_subscribe(
        None, iface, "Activated", PORTAL_PATH, None, Gio.DBusSignalFlags.NONE,
        lambda *a: activated.append(a[5].unpack()[1]))
    print(">>> Press %s now (watching %d s)." % (RECORD_TRIGGER, args.watch))
    context = GLib.MainContext.default()
    end = time.monotonic() + args.watch
    while time.monotonic() < end:
        context.iteration(False)
        time.sleep(0.05)
    portal.bus.signal_unsubscribe(sub)
    log.row("P11", "Activated during watch", activated or "none")
    listed = portal.request(iface, "ListShortcuts", lambda t: GLib.Variant(
        "(oa{sv})", (session, {"handle_token": GLib.Variant("s", t)})))
    if listed:
        for sid, props in listed[1].get("shortcuts", []):
            log.row("P11", "listed %s" % sid, props.get("trigger_description", "(none)"))
    portal.bus.call_sync(DESKTOP, session, "org.freedesktop.portal.Session", "Close", None, None,
                         Gio.DBusCallFlags.NONE, 5000, None)
    return 0


PROBES = {
    "env": probe_env,
    "audio": probe_audio,
    "monitor": probe_monitor,
    "cursor-modes": probe_cursor_modes,
    "session-close": probe_session_close,
    "window-end": probe_window_end,
    "frame-rate": probe_frame_rate,
    "load": probe_load,
    "shortcuts": probe_shortcuts,
}


def main():
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        epilog=__doc__.split("\n\n", 1)[1],
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("probe", choices=sorted(PROBES) + ["all"], help="what to measure")
    parser.add_argument("--expect", choices=("choose", "cancel"), default="choose",
                        help="monitor: choose a screen (default) or press Cancel in the chooser")
    parser.add_argument("--cursor", choices=sorted(CURSOR_MODES), default="hidden",
                        help="monitor: cursor_mode to ask for (default hidden)")
    parser.add_argument("--save", help="monitor: write the first frame to this PNG (do not "
                                       "commit it)")
    parser.add_argument("--no-parent", dest="parent", action="store_false",
                        help="pass no parent window to Start (P10 compares both)")
    parser.add_argument("--duration", type=int, default=60,
                        help="session-close, window-end, frame-rate: seconds (default 60)")
    parser.add_argument("--minutes", type=int, default=30, help="load: minutes (default 30)")
    parser.add_argument("--watch", type=int, default=15,
                        help="shortcuts: seconds to wait for a key press (default 15)")
    parser.add_argument("--timeout", type=int, default=120,
                        help="seconds to wait for a portal answer (default 120)")
    parser.add_argument("--log", type=pathlib.Path,
                        help="markdown log to append to (default under "
                             "openspec/changes/add-screen-recording/probe-results/)")
    args = parser.parse_args()

    session = os.environ.get("XDG_SESSION_TYPE", "unknown")
    stamp = datetime.date.today().isoformat()
    log = Log(args.log or ROOT / "openspec/changes/add-screen-recording/probe-results" /
              ("%s-%s-%s.md" % (stamp, os_tag(), session)))
    names = [n for n in PROBES if n != "load"] if args.probe == "all" else [args.probe]
    status = 0
    try:
        for name in names:
            if args.probe == "all" and name == "monitor":
                status |= probe_monitor(args, log)
                args.expect = "cancel"
                status |= probe_monitor(args, log)
                args.expect = "choose"
                continue
            status |= PROBES[name](args, log)
    except KeyboardInterrupt:
        log.note("interrupted by the user")
    finally:
        header = "%s · %s · %s session · `%s`" % (
            datetime.datetime.now().strftime("%Y-%m-%d %H:%M"), os_tag(), session,
            " ".join(["recording-probe.py"] + sys.argv[1:]))
        log.write(header)
    return status


if __name__ == "__main__":
    sys.exit(main())
