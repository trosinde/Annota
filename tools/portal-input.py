#!/usr/bin/env python3
"""Synthetic keyboard and pointer input for Annota's live tests, through the RemoteDesktop portal.

GNOME's own dialogs (the ScreenCast window chooser, the screenshot UI) cannot be driven over
AT-SPI on Wayland: their buttons can be pressed, but focus, row selection and synthetic keys are
refused. The RemoteDesktop portal is the sanctioned way to send input. GNOME asks once; with
`persist_mode` 2 it hands out a restore token, which this tool keeps in
`~/.cache/annota-tests/remote-desktop.token` (mode 0600), so later runs need no human.

This is test tooling only. Annota itself never uses the RemoteDesktop portal.

    tools/portal-input.py keys Tab Down space          # keysym names, pressed and released
    tools/portal-input.py move -4000 -4000             # relative pointer motion (logical px)
    tools/portal-input.py click                        # left button
    tools/portal-input.py forget                       # delete the stored token

As a module: `with Input() as i: i.keys("Tab", "space"); i.move(10, 0)`.
The first run shows GNOME's "Remote Desktop" question, parented to a small window of this tool.
"""

import os
import sys
import time

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

DESKTOP = "org.freedesktop.portal.Desktop"
PATH = "/org/freedesktop/portal/desktop"
IFACE = "org.freedesktop.portal.RemoteDesktop"
KEYBOARD, POINTER = 1, 2
BTN_LEFT = 0x110
TOKEN_FILE = os.path.join(os.path.expanduser("~/.cache/annota-tests"), "remote-desktop.token")


def keysym(name):
    gi.require_version("Gdk", "4.0")
    from gi.repository import Gdk  # noqa: E402

    value = Gdk.keyval_from_name(name)
    if not value or value == 0xFFFFFF:
        raise ValueError("unknown key name %r" % name)
    return value


class Input:
    def __init__(self, timeout=120):
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.timeout = timeout
        self.loop = GLib.MainLoop()
        self.unique = self.bus.get_unique_name().lstrip(":").replace(".", "_")
        self.counter = 0
        self.session = None

    def _request(self, method, params_for):
        self.counter += 1
        token = "annotainput%d" % self.counter
        handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (self.unique, token)
        answer = []

        def on_response(_c, _s, _p, _i, _sig, params):
            answer.append(params.unpack())
            self.loop.quit()

        sub = self.bus.signal_subscribe(None, "org.freedesktop.portal.Request", "Response",
                                        handle, None, Gio.DBusSignalFlags.NONE, on_response)
        self.bus.call_sync(DESKTOP, PATH, IFACE, method, params_for(token), None,
                           Gio.DBusCallFlags.NONE, 5000, None)
        source = GLib.timeout_add_seconds(self.timeout, self.loop.quit)
        self.loop.run()
        if answer:
            GLib.source_remove(source)
        self.bus.signal_unsubscribe(sub)
        if not answer:
            raise RuntimeError("%s: no answer within %d s" % (method, self.timeout))
        code, results = answer[0]
        if code != 0:
            raise RuntimeError("%s: response code %d" % (method, code))
        return results

    def __enter__(self):
        created = self._request("CreateSession", lambda t: GLib.Variant("(a{sv})", ({
            "handle_token": GLib.Variant("s", t),
            "session_handle_token": GLib.Variant("s", t + "s"),
        },)))
        self.session = created["session_handle"]
        options = {"types": GLib.Variant("u", KEYBOARD | POINTER),
                   "persist_mode": GLib.Variant("u", 2)}
        stored = _read_token()
        if stored:
            options["restore_token"] = GLib.Variant("s", stored)
        self._request("SelectDevices", lambda t: GLib.Variant("(oa{sv})", (
            self.session, dict(options, handle_token=GLib.Variant("s", t)))))
        parent = "" if stored else _parent_window()
        started = self._request("Start", lambda t: GLib.Variant("(osa{sv})", (
            self.session, parent, {"handle_token": GLib.Variant("s", t)})))
        if started.get("restore_token"):
            _write_token(started["restore_token"])
        return self

    def __exit__(self, *_):
        if self.session:
            self.bus.call_sync(DESKTOP, self.session, "org.freedesktop.portal.Session", "Close",
                               None, None, Gio.DBusCallFlags.NONE, 5000, None)
            self.session = None

    def _notify(self, method, signature, *args):
        self.bus.call_sync(DESKTOP, PATH, IFACE, method,
                           GLib.Variant(signature, (self.session, {}) + args), None,
                           Gio.DBusCallFlags.NONE, 5000, None)

    def keys(self, *names, pause=0.08):
        for name in names:
            sym = keysym(name)
            self._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym, 1)
            self._notify("NotifyKeyboardKeysym", "(oa{sv}iu)", sym, 0)
            time.sleep(pause)

    def move(self, dx, dy):
        self._notify("NotifyPointerMotion", "(oa{sv}dd)", float(dx), float(dy))
        time.sleep(0.05)

    def click(self):
        self._notify("NotifyPointerButton", "(oa{sv}iu)", BTN_LEFT, 1)
        self._notify("NotifyPointerButton", "(oa{sv}iu)", BTN_LEFT, 0)
        time.sleep(0.05)


def _read_token():
    try:
        with open(TOKEN_FILE) as f:
            return f.read().strip() or None
    except OSError:
        return None


def _write_token(token):
    os.makedirs(os.path.dirname(TOKEN_FILE), mode=0o700, exist_ok=True)
    fd = os.open(TOKEN_FILE, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(token)


def _parent_window():
    """A small visible window as the question's parent: without one GNOME may not show it."""
    gi.require_version("Gtk", "4.0")
    gi.require_version("GdkWayland", "4.0")
    # Importing GdkWayland is what gives the surface its export_handle() method.
    from gi.repository import GdkWayland, Gtk  # noqa: E402, F401

    Gtk.init()
    window = Gtk.Window(title="Annota test input")
    window.set_default_size(360, 80)
    window.set_child(Gtk.Label(label="Annota live tests ask for input access once."))
    window.present()
    handle = []
    context = GLib.MainContext.default()
    end = time.monotonic() + 5
    while not window.get_surface() and time.monotonic() < end:
        context.iteration(False)
    window.get_surface().export_handle(lambda _t, h: handle.append(h))
    while not handle and time.monotonic() < end:
        context.iteration(False)
    _parent_window.keep = window
    return "wayland:" + handle[0] if handle else ""


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    if argv[0] == "forget":
        try:
            os.remove(TOKEN_FILE)
        except FileNotFoundError:
            pass
        print("token removed")
        return 0
    with Input() as device:
        if argv[0] == "keys":
            device.keys(*argv[1:])
        elif argv[0] == "move":
            device.move(float(argv[1]), float(argv[2]))
        elif argv[0] == "click":
            device.click()
        elif argv[0] == "check":
            print("input session ready, token stored" if _read_token() else "no token")
        else:
            print("unknown command %r" % argv[0], file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
