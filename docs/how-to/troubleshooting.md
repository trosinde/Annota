# Troubleshooting

Known problems and how to get past them. Each entry starts with what you see.

## Annota does not start after installing the package

**What you see:** you installed or upgraded the `.deb`, and Annota does not open. The journal
(`journalctl --user -b | grep -i annota`) shows:

```
GLib-GIO-ERROR **: Settings schema 'io.github.trosinde.Annota' does not contain a key named 'shortcut-record'
```

The key named in the message may differ.

**Cause:** an older Annota was once built and installed from source with `meson install`, whose
default prefix is `/usr/local`. Its settings schema is still in
`/usr/local/share/glib-2.0/schemas/`. The desktop reads `/usr/local/share` before `/usr/share`,
so that old schema hides the newer one the package installed, and Annota asks for a setting the
old schema does not know.

**Fix:**

1. Quit Annota if it still runs: `pkill -x annota`.
2. List Annota's files under `/usr/local`:

   ```bash
   find /usr/local -iname '*annota*'
   ```

3. Remove them. If the build directory of that old build still exists, this does it for you:

   ```bash
   sudo ninja -C _build uninstall
   ```

   Otherwise delete the files the `find` command listed with `sudo rm` (check the list first;
   it should only contain Annota's own files, such as `/usr/local/bin/annota` and
   `/usr/local/share/glib-2.0/schemas/io.github.trosinde.Annota.gschema.xml`).
4. Rebuild the schema cache in `/usr/local`:

   ```bash
   sudo glib-compile-schemas /usr/local/share/glib-2.0/schemas
   ```

5. Start Annota again.

## A new version still behaves like the old one

**What you see:** after reinstalling or upgrading, Annota looks and acts as before. **About
Annota** shows the old version.

**Cause:** Annota runs as a single instance. If an older Annota process is still running, every
new launch – from the app grid, a shortcut or the command line – is handed to that old process
instead of starting the new program.

**Fix:** quit Annota before you install a new version, or afterwards with:

```bash
pkill -x annota
```

Then start it again. If it still shows the old version, check that no stale copy from a source
build shadows the package (see the entry above): `which -a annota` should list only
`/usr/bin/annota`.

## Something else

Recording problems: see the known limits in [How to record the screen](record-the-screen.md#known-limits-in-020).
Otherwise, please [open an issue](https://github.com/trosinde/Annota/issues) with your Ubuntu or
Debian version, `annota --version` and what you did.
