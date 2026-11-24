# Proposal

**Roadmap phase:** defect fix (released 0.2.0). No new feature.
**Benchmark feature (docs/feature-catalog.md):** none; app shell (startup).

## Why

Measured 2026-11-24 on the maintainer's machine: the installed `.deb` 0.2.0 does not start.
It aborts with

```
GLib-GIO-ERROR **: Settings schema 'io.github.trosinde.Annota' does not contain a key named 'shortcut-record'
```

A `meson install` from 2026-10-30 left `/usr/local/share/glib-2.0/schemas/io.github.trosinde.Annota.gschema.xml`
(compiled into that directory's `gschemas.compiled`) without the recording keys. `XDG_DATA_DIRS`
lists `/usr/local/share/` before `/usr/share/`, so GLib resolves the stale schema and shadows the
one the package installed. The first `gio::Settings::get` on a missing key aborts the process.
The user sees a GLib assertion that names a key, not the stale file nor how to fix it.

## What Changes

- Before any window and before `application::run()`, Annota checks the schema GLib resolves
  against every key of the schema this build ships.
- On a missing schema or missing keys, Annota prints a clear English message to stderr (missing
  keys, every schema file on the search path in lookup order, the fix) and exits with status 1.
- New requirement in `app-shell`: "Settings schema check at startup".

**Out of scope**
- Removing or repairing the stale file. Annota never writes outside the user's locations, and a
  file in `/usr/local` needs root.
- A dialog. Without a working schema the app cannot start its UI safely; stderr only, as for the
  existing missing-gresource error.
- Translation of the message (see design.md D3).
- Detecting a schema whose keys exist but whose types or defaults changed.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `app-shell`: ADDED requirement "Settings schema check at startup".

## Impact

- Code: new `src/schema_check.rs`, called from `src/main.rs`.
- Dependencies: none new.
- Packaging: none. `glib-compile-schemas` (libglib2.0-bin) is already a build dependency and is
  used by the tests.
- Platform: no Wayland/X11 difference; the check runs before any display connection.
