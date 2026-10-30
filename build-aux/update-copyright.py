#!/usr/bin/env python3
"""Regenerate the vendored-crates stanza of debian/copyright from Cargo metadata."""
import collections, json, pathlib, subprocess

root = pathlib.Path(__file__).resolve().parent.parent
meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--locked"], cwd=root))
licenses = collections.Counter()
crates = []
for p in meta["packages"]:
    if p["name"] == "annota":
        continue
    lic = (p.get("license") or "UNKNOWN").replace("/", " OR ")
    licenses[lic] += 1
    crates.append(f"{p['name']} {p['version']}: {lic}")
start, end = "# BEGIN VENDORED CRATES", "# END VENDORED CRATES"
block = [start, "Files: vendor/*", "Copyright: the respective crate authors",
         "License: " + " AND ".join(f"({l})" for l in sorted(licenses)),
         " Rust crates statically linked into the annota binary:"]
block += [f" {c}" for c in sorted(crates)]
if any("Apache-2.0" in l for l in licenses):
    # lintian's copyright-not-using-common-license-for-apache2 wants this spelled out
    # whenever Apache-2.0 appears in a License field (Debian Policy §12.5).
    block += [" .",
              " The Apache-2.0 license referenced above can be found in full at",
              " /usr/share/common-licenses/Apache-2.0."]
block += [end]
path = root / "debian" / "copyright"
text = path.read_text()
pre, rest = text.split(start, 1)
post = rest.split(end, 1)[1]
path.write_text(pre + "\n".join(block) + post)
print(f"{len(crates)} crates, {len(licenses)} license expressions")
