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
    tools/recording-probe.py selftest             # offline check of the frame analysis, no portal

P9 (X11) is the same `env`, `monitor` and `window-end` run in a "GNOME on Xorg" session, where
the desktop still offers one (Ubuntu 24.04; GNOME 50 has no X11 session any more).

Dialogs and indicators you will see:
- GNOME's screen-sharing chooser ("Screen Share"/"Share Screen") for `monitor`, `cursor-modes`,
  `session-close`, `frame-rate`, `load` (monitor tab) and `window-end` (window tab). Nothing is
  shared with anyone: the stream goes only into this process.
- GNOME's screen-sharing indicator while a stream is open, in GNOME's panel / quick settings
  (wherever the system status area is, at the top or the bottom of the screen). On some
  desktops it was not found at all (2026-11-23); `session-close --stop-hint` says what to look
  for.
- With `--parent` (default on), a small window "Annota recording probe" that is the chooser's
  parent, as Annota's waiting indicator will be.
- `audio`: no dialog; GNOME's microphone indicator appears for about 3 s.
- `shortcuts`: GNOME may show its "Allow Annota to use global shortcuts?" dialog. It binds the
  IDs region, window, screen AND record under Annota's app ID, exactly as Annota will at start,
  and GNOME keeps what it stores. This is the one probe with a lasting effect.

Privacy: no frame and no audio is written anywhere, except the single frame `--save` (P12) and
`cursor-modes` (P4) write for a visual check. That PNG shows your screen: do not commit or share
it. The log contains only numbers, sizes, codes and timings; `window-end` and `session-close`
hash a few rows of each frame in memory to tell repeated frames from new ones and log only how
often the hash changed, never a hash or a pixel.

Every run appends its result table to a markdown log, by default
`openspec/changes/add-screen-recording/probe-results/<date>-<os>-<session>.md`, which can be
committed as the evidence for task 2.1.
"""

import argparse
import ctypes
import ctypes.util
import datetime
import functools
import hashlib
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
    """Markdown log of one run, rewritten after every row and note.

    A probe that brings the interpreter down (a native crash in GStreamer or PyGObject) cannot
    reach a ``finally`` block, so the file is kept current instead of written at the end.
    """

    def __init__(self, path, header):
        self.path = path
        self.header = header
        self.rows = []
        self.notes = []
        self.before = None  # file contents from earlier runs, read on the first flush

    def row(self, probe, item, value):
        value = str(value)
        self.rows.append((probe, item, value))
        print("  [%s] %-38s %s" % (probe, item, value))
        self.flush()

    def note(self, text):
        self.notes.append(text)
        print("  note: %s" % text)
        self.flush()

    def flush(self):
        if not self.rows and not self.notes:
            return
        if self.before is None:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            self.before = (self.path.read_text(encoding="utf-8") if self.path.exists() else
                           "# Recording probe results\n\nWritten by `tools/recording-probe.py`. "
                           "Probes P1-P12: `openspec/changes/add-screen-recording/design.md`.\n")
        parts = [self.before, "\n## Run %s\n\n" % self.header,
                 "| Probe | Item | Result |\n|---|---|---|\n"]
        parts += ["| %s | %s | %s |\n" % (probe, item, value.replace("|", "\\|"))
                  for probe, item, value in self.rows]
        if self.notes:
            parts.append("\nNotes:\n\n")
            parts += ["- %s\n" % n for n in self.notes]
        tmp = self.path.with_suffix(".tmp")
        tmp.write_text("".join(parts), encoding="utf-8")
        tmp.replace(self.path)

    def done(self):
        if self.rows or self.notes:
            print("\nresults written to %s" % self.path)


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
    # Gio.bus_get_sync hands every Portal the same connection, and the host Registry binds the
    # app ID to the connection once: register once per process. (2026-11-23: every later
    # Register failed with "Connection already associated with an application ID", which the
    # old message misreported as "the portal sees no app ID".)
    registered = None

    def __init__(self, timeout):
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.timeout = timeout
        self.counter = 0
        self.unique = self.bus.get_unique_name().lstrip(":").replace(".", "_")
        if Portal.registered is not None:
            return
        try:
            self.bus.call_sync(DESKTOP, PORTAL_PATH, "org.freedesktop.host.portal.Registry",
                               "Register", GLib.Variant("(sa{sv})", (APP_ID, {})), None,
                               Gio.DBusCallFlags.NONE, 5000, None)
            Portal.registered = True
        except GLib.Error as err:
            if "already associated" in err.message:
                Portal.registered = True
            else:
                Portal.registered = False
                print("host Registry not available (%s); the portal sees no app ID"
                      % err.message)

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


# ---------------------------------------------------------------------------------------------
# Frame analysis (read-only, in memory; tested offline on videotestsrc, see `selftest`)


class MapInfo(ctypes.Structure):
    """struct GstMapInfo (gst/gstmemory.h)."""
    _fields_ = [("memory", ctypes.c_void_p), ("flags", ctypes.c_int),
                ("data", ctypes.c_void_p), ("size", ctypes.c_size_t),
                ("maxsize", ctypes.c_size_t), ("user_data", ctypes.c_void_p * 4),
                ("_gst_reserved", ctypes.c_void_p * 4)]


class CropMeta(ctypes.Structure):
    """struct GstVideoCropMeta (gst/video/gstvideometa.h)."""
    _fields_ = [("flags", ctypes.c_uint), ("info", ctypes.c_void_p),
                ("x", ctypes.c_uint), ("y", ctypes.c_uint),
                ("width", ctypes.c_uint), ("height", ctypes.c_uint)]


@functools.lru_cache(maxsize=None)
def gst_libs():
    """(libgstreamer, crop meta API GType or 0), loaded once.

    `ctypes.util.find_library` runs ldconfig on every call, far too slow for every buffer.
    """
    core = ctypes.CDLL(ctypes.util.find_library("gstreamer-1.0"))
    core.gst_buffer_get_meta.restype = ctypes.c_void_p
    core.gst_buffer_get_meta.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    core.gst_buffer_map.restype = ctypes.c_int
    core.gst_buffer_map.argtypes = [ctypes.c_void_p, ctypes.POINTER(MapInfo), ctypes.c_int]
    core.gst_buffer_unmap.restype = None
    core.gst_buffer_unmap.argtypes = [ctypes.c_void_p, ctypes.POINTER(MapInfo)]
    crop_api = 0
    name = ctypes.util.find_library("gstvideo-1.0")
    if name:
        video = ctypes.CDLL(name)
        video.gst_video_crop_meta_api_get_type.restype = ctypes.c_size_t
        crop_api = video.gst_video_crop_meta_api_get_type()
    return core, crop_api


def boxed_pointer(buf):
    # PyGObject's PyGBoxed: PyObject_HEAD, then the boxed pointer (as in portal-probe.py).
    return ctypes.c_void_p.from_address(id(buf) + 2 * ctypes.sizeof(ctypes.c_void_p)).value


def crop_rect(buf):
    """(x, y, width, height) of the buffer's GstVideoCropMeta, or None. Read-only."""
    core, crop_api = gst_libs()
    if not crop_api:
        return None
    meta = core.gst_buffer_get_meta(boxed_pointer(buf), crop_api)
    if not meta:
        return None
    rect = CropMeta.from_address(meta)
    return (rect.x, rect.y, rect.width, rect.height)


GST_MAP_READ = 1


def frame_digest(buf, height, row_step=16):
    """Short hash of every `row_step`-th row of a mapped frame: equal digest = repeated frame.

    Only in memory, never logged: the probes log how often it changed. Rows are hashed whole, so
    a change anywhere in a sampled row (pointer, text cursor, video) changes the digest.
    """
    core, _ = gst_libs()
    pointer = boxed_pointer(buf)
    info = MapInfo()
    if not core.gst_buffer_map(pointer, ctypes.byref(info), GST_MAP_READ):
        return None
    try:
        stride = info.size // max(1, height)
        digest = hashlib.blake2b(digest_size=8)
        for row in range(0, height, row_step):
            digest.update(ctypes.string_at(info.data + row * stride, stride))
        return digest.hexdigest()
    finally:
        core.gst_buffer_unmap(pointer, ctypes.byref(info))


class FrameTrack:
    """Buffer size, crop meta and content digest of every buffer, with monotonic times."""

    def __init__(self):
        self.samples = []  # (monotonic, (w, h), crop or None, digest or None)

    def observe(self, sample, now):
        buf = sample.get_buffer()
        s = sample.get_caps().get_structure(0)
        size = (s.get_value("width"), s.get_value("height"))
        self.samples.append((now, size, crop_rect(buf), frame_digest(buf, size[1])))

    def geometry_changes(self, start):
        """[(seconds after start, size, crop)] for the first buffer and every change."""
        changes, last = [], None
        for at, size, crop, _digest in self.samples:
            if (size, crop) != last:
                changes.append((at - start, size, crop))
                last = (size, crop)
        return changes

    def per_second(self, start, total):
        """(buffers, content changes) per whole second from `start`, `total` seconds."""
        buffers, changed = [0] * max(1, int(total)), [0] * max(1, int(total))
        previous = None
        for at, _size, _crop, digest in self.samples:
            index = int(at - start)
            if 0 <= index < len(buffers):
                buffers[index] += 1
                if previous is not None and digest != previous:
                    changed[index] += 1
            previous = digest
        return buffers, changed

    def summary_after(self, t0):
        """What happened from monotonic `t0` on: buffer count, geometry changes, content."""
        later = [s for s in self.samples if s[0] >= t0]
        before = [s for s in self.samples if s[0] < t0]
        last = before[-1] if before else None
        geometry = content = 0
        last_content = None
        for sample in later:
            if last is not None and (sample[1], sample[2]) != (last[1], last[2]):
                geometry += 1
            if last is not None and sample[3] != last[3]:
                content += 1
                last_content = sample[0]
            last = sample
        return {"buffers": len(later), "geometry_changes": geometry,
                "content_changes": content, "distinct": len({s[3] for s in later}),
                "last_content_change": last_content}

    def gaps(self, start, longer_than=1.0):
        """[(from s, to s)] relative to start where no buffer arrived for `longer_than` s."""
        found = []
        for (a, *_), (b, *_) in zip(self.samples, self.samples[1:]):
            if b - a > longer_than:
                found.append((a - start, b - start))
        return found


def describe_geometry(size, crop):
    return "buffer %d x %d, crop %s" % (size[0], size[1], crop if crop else "none")


class Cues:
    """Prints timed instructions while a stream runs: [(seconds, text)] in order."""

    def __init__(self, cues):
        self.cues = sorted(cues, key=lambda c: c[0])
        self.next = 0

    def __call__(self, elapsed):
        while self.next < len(self.cues) and elapsed >= self.cues[self.next][0]:
            print(self.cues[self.next][1], flush=True)
            self.next += 1


class Stream:
    """pipewiresrc into an appsink that only counts and measures; nothing is kept.

    With `track=True` every buffer's size, crop meta and content digest go into `self.track`.
    `launch` replaces the pipewiresrc part (offline self-test with videotestsrc).
    """

    def __init__(self, fd, node, keepalive=0, extra="", track=False, launch=None):
        src = "pipewiresrc fd=%d path=%d always-copy=true" % (fd, node)
        if keepalive:
            src += " keepalive-time=%d" % keepalive
        self.pipeline = Gst.parse_launch(
            "%s ! videoconvert ! %s video/x-raw,format=BGRA ! appsink name=sink "
            "emit-signals=true sync=false max-buffers=2 drop=true" % (launch or src, extra))
        self.sink = self.pipeline.get_by_name("sink")
        self.times, self.caps_seen, self.first = [], [], None
        self.track = FrameTrack() if track else None
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
        if self.track is not None:
            self.track.observe(sample, now)
        if self.first is None:
            self.first = sample
        return Gst.FlowReturn.OK

    def _on_message(self, _bus, msg):
        if msg.type == Gst.MessageType.EOS and self.end is None:
            self.end = ("eos", "", time.monotonic())
        elif msg.type == Gst.MessageType.ERROR and self.end is None:
            err, _dbg = msg.parse_error()
            self.end = ("error", err.message, time.monotonic())

    def run(self, seconds, stop_when=lambda: False, on_tick=None):
        self.pipeline.set_state(Gst.State.PLAYING)
        context = GLib.MainContext.default()
        start = time.monotonic()
        deadline = start + seconds
        while time.monotonic() < deadline and self.end is None and not stop_when():
            context.iteration(False)
            if on_tick is not None:
                on_tick(time.monotonic() - start)
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
    log.note("%s: one frame saved to %s for a visual check (pointer? GNOME's sharing "
             "indicator in the panel?). It shows your screen: do not commit or share it."
             % (probe, path))


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
            meta = iter_metas(stream.first.get_buffer())
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
    """API type names of every GstMeta on ``buf``, read-only.

    Not ``Gst.Buffer.foreach_meta``: it requires a writable buffer (it may remove metas), and on
    a sample's read-only buffer it fails an assertion and corrupts the heap (2026-11-23, P4).
    ``gst_buffer_iterate_meta`` only reads.
    """
    import ctypes
    import ctypes.util

    core = ctypes.CDLL(ctypes.util.find_library("gstreamer-1.0"))
    gobject = ctypes.CDLL(ctypes.util.find_library("gobject-2.0"))
    core.gst_buffer_iterate_meta.restype = ctypes.c_void_p
    core.gst_buffer_iterate_meta.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
    gobject.g_type_name.restype = ctypes.c_char_p
    gobject.g_type_name.argtypes = [ctypes.c_size_t]
    # PyGObject's PyGBoxed: PyObject_HEAD, then the boxed pointer (as in portal-probe.py).
    pointer = ctypes.c_void_p.from_address(id(buf) + 2 * ctypes.sizeof(ctypes.c_void_p)).value
    state = ctypes.c_void_p(None)
    names = []
    while True:
        meta = core.gst_buffer_iterate_meta(pointer, ctypes.byref(state))
        if not meta:
            break
        # struct GstMeta { GstMetaFlags flags; const GstMetaInfo *info; } ; info->api is a GType.
        info = ctypes.c_void_p.from_address(meta + ctypes.sizeof(ctypes.c_void_p)).value
        api = ctypes.c_size_t.from_address(info).value if info else 0
        name = gobject.g_type_name(api) if api else None
        names.append(name.decode() if name else "unknown")
    return names


def log_track(log, probe, stream, start, total):
    """Buffer size / crop changes and per-second buffer and content counts of a tracked run."""
    changes = stream.track.geometry_changes(start)
    for at, size, crop in changes[:40]:
        log.row(probe, "geometry at %.2f s" % at, describe_geometry(size, crop))
    if len(changes) > 40:
        log.row(probe, "geometry changes not listed", len(changes) - 40)
    buffers, content = stream.track.per_second(start, total)
    log.row(probe, "buffers per second", " ".join(map(str, buffers)))
    log.row(probe, "content changes per second", " ".join(map(str, content)))
    gaps = stream.track.gaps(start)
    log.row(probe, "gaps > 1 s without buffers", ", ".join(
        "%.1f-%.1f s" % gap for gap in gaps) if gaps else "none")
    if stream.times:
        log.row(probe, "last buffer at", "%.2f s" % (stream.times[-1] - start))
    else:
        log.row(probe, "last buffer at", "no buffer at all")


def log_end(log, probe, stream, cast, start):
    log.row(probe, "stream end", "%s %s after %.2f s" % (stream.end[0], stream.end[1],
                                                         stream.end[2] - start)
            if stream.end else "none (no EOS, no error)")
    log.row(probe, "Session.Closed", "after %.2f s" % (cast.closed_at - start)
            if cast.closed_at else "not received")


DEFAULT_STOP_HINT = ("Look for an ORANGE screen-sharing icon in GNOME's panel / quick settings "
                     "(wherever the system status area is, at the top or the bottom). Click it, "
                     "or open quick settings, and choose Stop / turn screen sharing off.")


def probe_session_close(args, log):
    load_gi(gst=True)
    hint = (" Hint: %s" % args.stop_hint) if args.stop_hint else ""
    if not ask("Opens a screen stream and keeps it running for up to %d s. While it runs, STOP "
               "the sharing from GNOME's own UI: the screen-sharing indicator in GNOME's panel / "
               "quick settings (wherever the system status area is). The probe logs every "
               "buffer until the end, whether Session.Closed arrives and what the stream does. "
               "If you find no stop control, let it run out and say so.%s" % (
                   args.duration, hint),
               "GNOME's chooser, then GNOME's sharing indicator (panel / quick settings), which "
               "you use to stop"):
        return 0
    _p, cast, opened = open_stream(args, log, "P5", SOURCE_MONITOR)
    if not opened:
        return 0
    node, _props = opened
    stream = Stream(cast.remote_fd(), node, track=True)
    cues = [(0, ">>> Now stop the sharing from GNOME's panel / quick settings "
                "(wherever the system status area is)." + hint)]
    cues += [(t, "    ... still streaming at %d s (limit %d s)" % (t, args.duration))
             for t in range(10, args.duration, 10)]
    start = stream.run(args.duration, on_tick=Cues(cues),
                       stop_when=lambda: cast.closed_at is not None
                       and time.monotonic() - cast.closed_at > 2)
    stream.stop()
    log.row("P5", "stop hint shown", "yes" if args.stop_hint else "no")
    log_end(log, "P5", stream, cast, start)
    log_track(log, "P5", stream, start, args.duration)
    ran_out = (cast.closed_at is None and stream.end is None and stream.times
               and stream.times[-1] - start > args.duration - 2)
    if ran_out:
        log.row("P5", "verdict", "NO STOP SEEN: no Session.Closed, no stream end, buffers kept "
                "arriving until the %d s limit. Note whether a stop control was found and used."
                % args.duration)
    elif cast.closed_at is None and stream.end is None:
        log.row("P5", "verdict", "buffers stopped without Session.Closed or stream end")
    else:
        log.row("P5", "verdict", "the session or the stream ended (see rows above)")
    cast.close()
    return 0


def probe_window_end(args, log):
    load_gi(gst=True)
    close_at = max(15, args.duration // 2)
    if not ask("Opens a WINDOW stream for %d s. Choose a window you can close that is NOT this "
               "terminal (e.g. a new Text Editor window). Follow the countdown printed here: "
               "RESIZE the window a few times until %d s, then hands off, then CLOSE it when told "
               "(at %d s) and wait for the end. Every buffer's size, crop meta and a content "
               "hash are recorded, so a resize, a close and repeated frames become visible." % (
                   args.duration, close_at - 10, close_at),
               "GNOME's screen-sharing chooser (window list), then GNOME's sharing indicator"):
        return 0
    _p, cast, opened = open_stream(args, log, "P6", SOURCE_WINDOW)
    if not opened:
        return 0
    node, props = opened
    stream = Stream(cast.remote_fd(), node, track=True)
    cues = [(0, ">>> RESIZE the recorded window a few times now (until %d s)." % (close_at - 10)),
            (close_at - 10, ">>> Stop resizing. Hands off the window. Close it in 10 s.")]
    cues += [(close_at - n, "    close in %d ..." % n) for n in (5, 4, 3, 2, 1)]
    cues += [(close_at, ">>> CLOSE THE WINDOW NOW (then do nothing until the end)."),
             (close_at + 5, "    observing what the stream does after the close ...")]
    cues += [(t, "    ... %d s left" % (args.duration - t))
             for t in range(close_at + 10, args.duration, 10)]
    start = stream.run(args.duration, on_tick=Cues(cues),
                       stop_when=lambda: cast.closed_at is not None
                       and time.monotonic() - cast.closed_at > 2)
    stream.stop()
    log.row("P6", "close prompt at", "%d s" % close_at)
    if stream.first is not None:
        frame_facts(stream.first, log, "P6", props, None)
    log_end(log, "P6", stream, cast, start)
    log_track(log, "P6", stream, start, args.duration)
    # From 3 s after the prompt: the user needs a moment to close the window.
    after = stream.track.summary_after(start + close_at + 3)
    log.row("P6", "after close prompt +3 s", "%d buffers, %d size/crop changes, %d content "
            "changes, %d distinct frames%s" % (
                after["buffers"], after["geometry_changes"], after["content_changes"],
                after["distinct"], "" if after["last_content_change"] is None else
                ", last content change at %.2f s" % (after["last_content_change"] - start)))
    if stream.end is None and cast.closed_at is None:
        if after["buffers"] == 0:
            verdict = ("buffers STOPPED after the close prompt, but no EOS, no error and no "
                       "Session.Closed")
        elif after["geometry_changes"] == 0 and after["distinct"] <= 1:
            verdict = ("the stream NEITHER ENDED NOR CHANGED after the close prompt: buffers kept "
                       "arriving with identical content (repeats of the last frame)")
        else:
            verdict = ("no EOS, no error, no Session.Closed, but buffers still changed after the "
                       "close prompt (was the window really closed?)")
    else:
        verdict = "the session or the stream ended (see rows above)"
    log.row("P6", "verdict", verdict)
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
    half = args.duration // 2
    for keepalive in (0, 1000):
        if not ask("Screen stream for %d s, pipewiresrc keepalive-time=%d. BEFORE you press "
                   "Enter: stop EVERY video, animation and audio visualiser, and close or "
                   "pause anything that redraws by itself. Phase 1 (%d s): keep the pointer "
                   "completely still, touch nothing. Phase 2 (%d s): scroll a page or drag a "
                   "window. A 3-2-1 is printed before each phase." % (
                       args.duration + 6, keepalive, half, half),
                   "GNOME's screen-sharing chooser, then GNOME's sharing indicator"):
            continue
        _p, cast, opened = open_stream(args, log, "P7", SOURCE_MONITOR)
        if not opened:
            continue
        node, _props = opened
        stream = Stream(cast.remote_fd(), node, keepalive=keepalive)
        cues = [(0, "    still phase in 3 ..."), (1, "    2 ..."), (2, "    1 ..."),
                (3, ">>> STILL: pointer still, touch nothing (%d s)." % half),
                (3 + half, "    movement phase in 3 ..."), (4 + half, "    2 ..."),
                (5 + half, "    1 ..."),
                (6 + half, ">>> MOVE: scroll a page or drag a window (%d s)." % half)]
        start = stream.run(args.duration + 6, on_tick=Cues(cues))
        stream.stop()
        # The still phase starts at 3 s; skip its first second (the printed cue redraws).
        still = per_bucket(stream.times, start + 4, 5, half - 1)
        moved = per_bucket(stream.times, start + 6 + half, 5, half)
        log.row("P7", "keepalive %d: still phase, buffers per 5 s" % keepalive,
                " ".join(map(str, still)))
        log.row("P7", "keepalive %d: movement phase, buffers per 5 s" % keepalive,
                " ".join(map(str, moved)))
        if stream.first is not None:
            s = stream.first.get_caps().get_structure(0)
            log.row("P7", "caps", s.to_string()[:160])
        cast.close()
    return 0


def probe_selftest(_args, log):
    """Offline check of the frame-analysis helpers on videotestsrc. No portal, no screen."""
    load_gi(gst=True)
    failures = []

    def check(what, ok, detail):
        print("  selftest %-44s %s (%s)" % (what, "ok" if ok else "FAILED", detail))
        if not ok:
            failures.append(what)

    def run(launch, seconds=3):
        stream = Stream(0, 0, track=True, launch=launch)
        start = stream.run(seconds)
        stream.stop()
        return stream, start

    moving, start = run("videotestsrc num-buffers=20 pattern=ball ! "
                        "video/x-raw,format=BGRA,width=320,height=240,framerate=30/1 ! "
                        "videocrop left=10 top=20 right=30 bottom=40")
    track = moving.track
    check("videocrop: buffers seen", len(track.samples) == 20, len(track.samples))
    check("videocrop: cropped size", {s[1] for s in track.samples} == {(280, 180)},
          {s[1] for s in track.samples})
    check("moving ball: digests differ", len({s[3] for s in track.samples}) > 10,
          "%d distinct" % len({s[3] for s in track.samples}))
    check("geometry: one entry for a constant stream", len(track.geometry_changes(start)) == 1,
          track.geometry_changes(start))
    still, _ = run("videotestsrc num-buffers=20 pattern=solid-color ! "
                   "video/x-raw,format=BGRA,width=320,height=240,framerate=30/1 ! "
                   "videocrop left=10 right=30")
    check("still frame: one digest", len({s[3] for s in still.track.samples}) == 1,
          "%d distinct" % len({s[3] for s in still.track.samples}))

    # Crop meta as pipewiresrc attaches it: added by hand to a writable buffer.
    core, crop_api = gst_libs()
    video = ctypes.CDLL(ctypes.util.find_library("gstvideo-1.0"))
    video.gst_video_crop_meta_get_info.restype = ctypes.c_void_p
    core.gst_buffer_add_meta.restype = ctypes.c_void_p
    core.gst_buffer_add_meta.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    buf = Gst.Buffer.new_wrapped(bytes(64 * 32 * 4))
    check("crop meta: none on a plain buffer", crop_rect(buf) is None, crop_rect(buf))
    meta = CropMeta.from_address(core.gst_buffer_add_meta(
        boxed_pointer(buf), video.gst_video_crop_meta_get_info(), None))
    meta.x, meta.y, meta.width, meta.height = 1, 2, 30, 20
    check("crop meta: read back", crop_rect(buf) == (1, 2, 30, 20), crop_rect(buf))
    check("digest of a hand-made buffer", frame_digest(buf, 32) is not None, "mapped")

    # Analysis on synthetic samples: resize at 1 s, crop change at 2 s, frozen after 3 s.
    synth = FrameTrack()
    for i in range(50):
        t = i * 0.1
        size = (640, 480) if t < 1 else (800, 600)
        crop = (0, 0, 600, 400) if t < 2 else (0, 0, 700, 500)
        synth.samples.append((t, size, crop, "d%d" % i if t < 3 else "frozen"))
    changes = synth.geometry_changes(0)
    check("analysis: resize and crop change found", [round(c[0], 1) for c in changes]
          == [0.0, 1.0, 2.0], changes)
    after = synth.summary_after(3.0)
    check("analysis: frozen after 3 s", after["distinct"] == 1 and after["content_changes"] == 1
          and after["geometry_changes"] == 0, after)
    buffers, content = synth.per_second(0, 5)
    check("analysis: per second", buffers == [10] * 5 and content[4] == 0, (buffers, content))
    synth.samples.append((9.0, (800, 600), (0, 0, 700, 500), "frozen"))
    check("analysis: gap found", synth.gaps(0) == [(4.9, 9.0)], synth.gaps(0))
    print("selftest: %s" % ("all checks passed" if not failures else
                            "FAILED: " + ", ".join(failures)))
    return 1 if failures else 0


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
    "selftest": probe_selftest,
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
                        help="session-close, window-end, frame-rate: seconds (default 60; "
                             "window-end prompts the close at half of it)")
    parser.add_argument("--stop-hint", nargs="?", const=DEFAULT_STOP_HINT, default=None,
                        help="session-close: also tell the user where to look for GNOME's stop "
                             "control (alone: look for an orange screen-sharing icon in GNOME's "
                             "panel / quick settings; or give your own text)")
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
    header = "%s · %s · %s session · `%s`" % (
        datetime.datetime.now().strftime("%Y-%m-%d %H:%M"), os_tag(), session,
        " ".join(["recording-probe.py"] + sys.argv[1:]))
    log = Log(args.log or ROOT / "openspec/changes/add-screen-recording/probe-results" /
              ("%s-%s-%s.md" % (stamp, os_tag(), session)), header)
    if args.probe == "selftest":  # offline, writes no probe log
        return probe_selftest(args, None)
    names = ([n for n in PROBES if n not in ("load", "selftest")] if args.probe == "all"
             else [args.probe])
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
        log.done()
    return status


if __name__ == "__main__":
    sys.exit(main())
