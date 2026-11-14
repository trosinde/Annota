#!/usr/bin/env python3
"""Scenario <-> test gate for OpenSpec projects.

Two rules, both hard:
  1. Every test references at least one OpenSpec scenario
     (`<comment> spec: <capability> / <Requirement> / <Scenario>` directly above the test).
  2. No implemented scenario without a test. Implemented = every scenario in openspec/specs/,
     plus the ADDED/MODIFIED scenarios of the change being verified. The only alternative to a
     test is a manual-check row with Result `pass`, a date and a reason why it cannot be automated.

Modes:
  check               static: untagged tests, orphan tags, uncovered scenarios
  verify <change-id>  check + run the tests (config [[run]]); tagged tests must be green,
                      the change's manual checks must not be `not run`; writes
                      openspec/changes/<id>/scenario-report.md

Configuration: openspec/scenario-gate.toml. Known violations of an existing project may be listed
in openspec/scenario-baseline.txt (`--write-baseline`); the baseline may only shrink.

Standard library only, Python >= 3.11.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tomllib
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from pathlib import Path

CONFIG = Path("openspec/scenario-gate.toml")
BASELINE = Path("openspec/scenario-baseline.txt")

DEFAULTS = {
    "test_globs": [],
    "exclude_globs": ["**/target/**", "**/node_modules/**", "**/.venv/**", "**/vendor/**"],
    "test_marker": r"^\s*#\[test\]",
    "name_regex": r"fn\s+(\w+)",
    "name_window": 12,
    "comment": "//",
    "attribute_regex": r"^\s*(#\[|@|\[)",
    "manual_checks": ["openspec/changes/**/manual-checks.md"],
    "run": [],
}


def norm(text: str) -> str:
    return " ".join(text.split()).casefold()


# ---------------------------------------------------------------- specs


@dataclass(frozen=True)
class Scenario:
    capability: str
    requirement: str
    scenario: str

    @property
    def ref(self) -> str:
        return f"{self.capability} / {self.requirement} / {self.scenario}"

    @property
    def key(self) -> tuple[str, str, str]:
        return (norm(self.capability), norm(self.requirement), norm(self.scenario))


REQ_RE = re.compile(r"^###\s+Requirement:\s*(.+?)\s*$")
SCEN_RE = re.compile(r"^####\s+Scenario:\s*(.+?)\s*$")
SECTION_RE = re.compile(r"^##\s+(ADDED|MODIFIED|REMOVED|RENAMED)\s+Requirements\b", re.I)
RENAME_RE = re.compile(r"^\s*-\s*(FROM|TO):\s*`?(?:###\s+)?Requirement:\s*(.+?)`?\s*$", re.I)


def parse_spec(text: str) -> dict[str, dict[str, list[str]]]:
    """Return {section: {requirement: [scenarios]}}; section is '' for main specs."""
    out: dict[str, dict[str, list[str]]] = {}
    section, req = "", None
    for line in text.splitlines():
        m = SECTION_RE.match(line)
        if m:
            section, req = m.group(1).upper(), None
            continue
        m = REQ_RE.match(line)
        if m:
            req = m.group(1)
            out.setdefault(section, {}).setdefault(req, [])
            continue
        m = SCEN_RE.match(line)
        if m and req is not None:
            out[section][req].append(m.group(1))
    return out


def parse_renames(text: str) -> list[tuple[str, str]]:
    pairs, src = [], None
    in_renamed = False
    for line in text.splitlines():
        m = SECTION_RE.match(line)
        if m:
            in_renamed = m.group(1).upper() == "RENAMED"
            continue
        if not in_renamed:
            continue
        m = RENAME_RE.match(line)
        if m and m.group(1).upper() == "FROM":
            src = m.group(2)
        elif m and src is not None:
            pairs.append((src, m.group(2)))
            src = None
    return pairs


def main_scenarios(root: Path) -> dict[tuple[str, str], list[str]]:
    reqs: dict[tuple[str, str], list[str]] = {}
    for spec in sorted((root / "openspec/specs").glob("*/spec.md")):
        cap = spec.parent.name
        for req, scens in parse_spec(spec.read_text(encoding="utf-8")).get("", {}).items():
            reqs[(cap, req)] = scens
    return reqs


def apply_change(reqs: dict[tuple[str, str], list[str]], change_dir: Path) -> dict:
    """Overlay a change's deltas on the main requirements; returns the change's own scenarios."""
    own: dict[tuple[str, str], list[str]] = {}
    for spec in sorted((change_dir / "specs").glob("*/spec.md")):
        cap = spec.parent.name
        text = spec.read_text(encoding="utf-8")
        for src, dst in parse_renames(text):
            for key in list(reqs):
                if key[0] == cap and norm(key[1]) == norm(src):
                    reqs[(cap, dst)] = reqs.pop(key)
        parsed = parse_spec(text)
        for req in parsed.get("REMOVED", {}):
            for key in list(reqs):
                if key[0] == cap and norm(key[1]) == norm(req):
                    del reqs[key]
        for section in ("ADDED", "MODIFIED"):
            for req, scens in parsed.get(section, {}).items():
                for key in list(reqs):
                    if key[0] == cap and norm(key[1]) == norm(req):
                        del reqs[key]
                reqs[(cap, req)] = scens
                own[(cap, req)] = scens
    return own


def flatten(reqs: dict[tuple[str, str], list[str]]) -> list[Scenario]:
    return [Scenario(c, r, s) for (c, r), scens in reqs.items() for s in scens]


def active_changes(root: Path) -> list[Path]:
    base = root / "openspec/changes"
    if not base.is_dir():
        return []
    return sorted(p for p in base.iterdir() if p.is_dir() and p.name != "archive")


# ---------------------------------------------------------------- tests and tags


@dataclass
class Test:
    file: str
    line: int
    name: str
    header: str  # tag/attribute block above the marker plus the marker line
    tags: list[str] = field(default_factory=list)

    @property
    def ident(self) -> str:
        return f"{self.file}::{self.name}"


def find_tests(root: Path, cfg: dict) -> list[Test]:
    marker = re.compile(cfg["test_marker"])
    name_re = re.compile(cfg["name_regex"])
    attr = re.compile(cfg["attribute_regex"])
    comment = cfg["comment"]
    tag_re = re.compile(r"^\s*" + re.escape(comment) + r"[/!]*\s*spec:\s*(.+?)\s*$")
    tests: list[Test] = []
    seen: set[Path] = set()
    for pattern in cfg["test_globs"]:
        for path in sorted(root.glob(pattern)):
            rel = path.relative_to(root).as_posix()
            if path in seen or not path.is_file():
                continue
            if any(fnmatch.fnmatch(rel, ex) for ex in cfg["exclude_globs"]):
                continue
            seen.add(path)
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
            i = 0
            while i < len(lines):
                if not marker.search(lines[i]):
                    i += 1
                    continue
                # A run of stacked markers (e.g. #[test] #[ignore]) belongs to one test.
                start = i
                name = None
                for j in range(i, min(len(lines), i + cfg["name_window"])):
                    m = name_re.search(lines[j])
                    if m:
                        name, end = m.group(1), j
                        break
                if name is None:
                    i += 1
                    continue
                tags, k = [], start - 1
                while k >= 0:
                    s = lines[k]
                    t = tag_re.match(s)
                    if t:
                        tags.append(t.group(1))
                    elif not (s.strip().startswith(comment) or attr.match(s)):
                        break
                    k -= 1
                header = "\n".join(lines[k + 1 : end + 1])
                tests.append(Test(rel, start + 1, name, header, list(reversed(tags))))
                i = end + 1
    return tests


# ---------------------------------------------------------------- manual checks


@dataclass
class ManualRow:
    file: str
    ref: str
    result: str
    date: str
    why: str

    @property
    def passed(self) -> bool:
        return self.result.strip().casefold().startswith("pass") and bool(self.date.strip())

    @property
    def valid(self) -> bool:
        return self.passed and bool(self.why.strip())


def split_row(line: str) -> list[str]:
    return [c.strip() for c in line.strip().strip("|").split("|")]


def find_manual(root: Path, cfg: dict) -> list[ManualRow]:
    rows: list[ManualRow] = []
    files: set[Path] = set()
    for pattern in cfg["manual_checks"]:
        files.update(p for p in root.glob(pattern) if p.is_file())
    for path in sorted(files):
        rel = path.relative_to(root).as_posix()
        cols: dict[str, int] | None = None
        for line in path.read_text(encoding="utf-8").splitlines():
            if not line.lstrip().startswith("|"):
                cols = None
                continue
            cells = split_row(line)
            if cols is None:
                names = [c.casefold() for c in cells]
                if {"spec", "result", "date"} <= set(names):
                    cols = {n: names.index(n) for n in names}
                continue
            if all(set(c) <= set("-: ") for c in cells):
                continue
            why_idx = next((v for k, v in cols.items() if k.startswith("why")), None)

            def cell(idx: int | None) -> str:
                return cells[idx] if idx is not None and idx < len(cells) else ""

            rows.append(
                ManualRow(rel, cell(cols["spec"]).strip("`"), cell(cols["result"]),
                          cell(cols["date"]), cell(why_idx))
            )
    return rows


# ---------------------------------------------------------------- running tests


def run_tests(root: Path, cfg: dict, tests: list[Test]) -> tuple[dict[str, str], list[str]]:
    """Return ({test ident: passed|failed|not run}, errors)."""
    results: dict[str, list[str]] = {t.ident: [] for t in tests}
    errors: list[str] = []
    for run in cfg["run"]:
        label = run.get("name", run["cmd"])
        select = re.compile(run["select"]) if run.get("select") else None
        exclude = re.compile(run["exclude"]) if run.get("exclude") else None
        chosen = [
            t for t in tests
            if (not select or select.search(t.header)) and not (exclude and exclude.search(t.header))
        ]
        if not chosen:
            continue
        cmds = (
            [(run["cmd"].format(name=t.name), [t]) for t in chosen]
            if run.get("per_test") else [(run["cmd"], chosen)]
        )
        for cmd, targets in cmds:
            proc = subprocess.run(cmd, shell=True, cwd=root, capture_output=True, text=True)
            raw = parse_results(root, run, proc.stdout + "\n" + proc.stderr)
            if not raw:
                errors.append(f"run '{label}' produced no test results (exit {proc.returncode}): {cmd}")
                continue
            for t in targets:
                status = match_status(t, raw, run.get("pass", ["ok", "passed"]))
                if status:
                    results[t.ident].append(status)
    final = {}
    for ident, statuses in results.items():
        if "failed" in statuses:
            final[ident] = "failed"
        elif "passed" in statuses:
            final[ident] = "passed"
        else:
            final[ident] = "not run"
    return final, errors


def parse_results(root: Path, run: dict, output: str) -> list[tuple[str, str]]:
    if run.get("junit"):
        out = []
        for path in root.glob(run["junit"]):
            for case in ET.parse(path).iter("testcase"):
                full = f"{case.get('classname', '')}::{case.get('name', '')}"
                if case.find("failure") is not None or case.find("error") is not None:
                    out.append((full, "failed"))
                elif case.find("skipped") is not None:
                    out.append((full, "skipped"))
                else:
                    out.append((full, "passed"))
        return out
    rx = re.compile(run.get("result_regex", r"^test (?P<name>\S+) \.\.\. (?P<status>\w+)"), re.M)
    return [(m.group("name"), m.group("status")) for m in rx.finditer(output)]


SEG = re.compile(r"::|\.|/|#|\s+")


def match_status(test: Test, raw: list[tuple[str, str]], passes: list[str]) -> str | None:
    hits = [(n, s) for n, s in raw if [p for p in SEG.split(n) if p][-1:] == [test.name]]
    if len(hits) > 1:
        stem = Path(test.file).stem
        narrowed = [(n, s) for n, s in hits if stem in SEG.split(n)]
        hits = narrowed or hits
    if not hits:
        return None
    statuses = {s for _, s in hits}
    if any(s.casefold() in ("failed", "fail", "error") for s in statuses):
        return "failed"
    if all(s in passes or s == "passed" for s in statuses):
        return "passed"
    return None  # ignored / skipped: not run in this run


# ---------------------------------------------------------------- gate


def load_config(root: Path) -> dict:
    path = root / CONFIG
    if not path.is_file():
        sys.exit(f"missing {CONFIG}; set the gate up with the openspec-scenario-gate skill")
    cfg = dict(DEFAULTS)
    cfg.update(tomllib.loads(path.read_text(encoding="utf-8")))
    if not cfg["test_globs"]:
        sys.exit(f"{CONFIG}: test_globs is empty")
    return cfg


def find_root() -> Path:
    here = Path.cwd().resolve()
    for p in (here, *here.parents):
        if (p / "openspec").is_dir():
            return p
    sys.exit("no openspec/ directory found above the current directory")


def evaluate(root: Path, cfg: dict, change: str | None, run: bool) -> dict:
    reqs = main_scenarios(root)
    own: dict = {}
    if change:
        cdir = root / "openspec/changes" / change
        if not cdir.is_dir():
            sys.exit(f"no active change '{change}' in openspec/changes/")
        own = apply_change(reqs, cdir)
    implemented = flatten(reqs)
    own_scen = flatten(own)

    # Tags may also point at scenarios of other active (not yet archived) changes.
    known = {s.key for s in implemented}
    for cdir in active_changes(root):
        tmp = main_scenarios(root)
        known |= {s.key for s in flatten(apply_change(tmp, cdir))}

    tests = find_tests(root, cfg)
    manual = find_manual(root, cfg)
    violations: list[str] = []
    errors: list[str] = []

    def tag_key(tag: str) -> tuple[str, ...]:
        return tuple(norm(p) for p in tag.split(" / ", 2))

    by_scen: dict[tuple, list[Test]] = {}
    for t in tests:
        if not t.tags:
            violations.append(f"untagged-test: {t.ident}")
        for tag in t.tags:
            k = tag_key(tag)
            if len(k) != 3 or k not in known:
                violations.append(f"orphan-tag: {t.ident} -> {tag}")
            else:
                by_scen.setdefault(k, []).append(t)

    manual_by: dict[tuple, list[ManualRow]] = {}
    for row in manual:
        k = tag_key(row.ref)
        if len(k) != 3 or k not in known:
            violations.append(f"orphan-manual-check: {row.file} -> {row.ref}")
        else:
            manual_by.setdefault(k, []).append(row)

    for s in implemented:
        if not by_scen.get(s.key) and not any(r.valid for r in manual_by.get(s.key, [])):
            violations.append(f"uncovered-scenario: {s.ref}")

    statuses: dict[str, str] = {}
    if run:
        statuses, errors = run_tests(root, cfg, [t for t in tests if t.tags])
        for t in tests:
            if t.tags and statuses.get(t.ident) != "passed":
                errors.append(f"test {statuses.get(t.ident, 'not run')}: {t.ident}")
        cdir = f"openspec/changes/{change}/"
        for row in manual:
            if row.file.startswith(cdir) and not row.passed:
                errors.append(f"manual check not passed: {row.file} -> {row.ref} ({row.result or 'empty'})")
            elif row.file.startswith(cdir) and not row.why.strip():
                errors.append(f"manual check without reason: {row.file} -> {row.ref}")

    baseline_path = root / BASELINE
    baseline = set()
    if baseline_path.is_file():
        baseline = {
            ln.strip() for ln in baseline_path.read_text(encoding="utf-8").splitlines()
            if ln.strip() and not ln.startswith("#")
        }
    own_keys = {s.key for s in own_scen}
    current = set(violations)
    blocking = []
    for v in violations:
        # The change being verified may never lean on the baseline.
        is_own = v.startswith("uncovered-scenario: ") and tag_key(v.split(": ", 1)[1]) in own_keys
        if v not in baseline or is_own:
            blocking.append(v)
    stale = sorted(b for b in baseline if b not in current)
    blocking += [f"stale-baseline-entry (now fixed, delete it): {b}" for b in stale]

    rows = []
    for s in own_scen or []:
        ts = by_scen.get(s.key, [])
        ms = [r for r in manual_by.get(s.key, []) if r.valid]
        rows.append({
            "scenario": s.ref,
            "tests": [{"test": t.ident, "status": statuses.get(t.ident, "-")} for t in ts],
            "manual": [f"{r.file} ({r.result}, {r.date})" for r in ms],
        })

    return {
        "change": change,
        "implemented": len(implemented),
        "tests": len(tests),
        "tagged_tests": sum(1 for t in tests if t.tags),
        "covered": sum(1 for s in implemented if by_scen.get(s.key)
                       or any(r.valid for r in manual_by.get(s.key, []))),
        "baselined": len(current & baseline),
        "blocking": sorted(set(blocking)) + errors,
        "violations": sorted(current),
        "change_rows": rows,
    }


def render(result: dict, fmt: str) -> str:
    if fmt == "json":
        return json.dumps(result, indent=2, ensure_ascii=False)
    ok = not result["blocking"]
    head = (
        f"Scenarios implemented: {result['implemented']}, covered: {result['covered']}; "
        f"tests: {result['tests']}, tagged: {result['tagged_tests']}; "
        f"baselined violations: {result['baselined']}"
    )
    if fmt == "text":
        out = [("PASS" if ok else "FAIL") + " - " + head]
        out += [f"  {b}" for b in result["blocking"]]
        for r in result["change_rows"]:
            ev = [f"{t['test']} [{t['status']}]" for t in r["tests"]] + r["manual"]
            out.append(f"  {r['scenario']}: {', '.join(ev) or 'NO EVIDENCE'}")
        return "\n".join(out)
    title = f"Scenario gate - {result['change']}" if result["change"] else "Scenario gate"
    out = [f"# {title}", "", f"**{'PASS' if ok else 'FAIL'}** - {head}", ""]
    if result["change_rows"]:
        out += ["| Scenario | Evidence | Result |", "|---|---|---|"]
        for r in result["change_rows"]:
            ev = [f"`{t['test']}`" for t in r["tests"]] + r["manual"]
            st = sorted({t["status"] for t in r["tests"]}) + (["manual pass"] if r["manual"] else [])
            out.append(f"| {r['scenario']} | {'<br>'.join(ev) or '**none**'} | {', '.join(st) or '**uncovered**'} |")
        out.append("")
    if result["blocking"]:
        out += ["## Blocking", ""] + [f"- {b}" for b in result["blocking"]] + [""]
    return "\n".join(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="mode", required=True)
    for name in ("check", "verify"):
        p = sub.add_parser(name)
        if name == "verify":
            p.add_argument("change")
        p.add_argument("--format", choices=["text", "markdown", "json"], default="text")
        p.add_argument("--write-baseline", action="store_true",
                       help="record today's static violations as the baseline (existing projects)")
    args = ap.parse_args(argv)
    root = find_root()
    cfg = load_config(root)
    change = getattr(args, "change", None)
    result = evaluate(root, cfg, change, run=args.mode == "verify")
    if args.write_baseline:
        (root / BASELINE).write_text(
            "# Known violations of the scenario gate. This list may only shrink.\n"
            + "".join(v + "\n" for v in result["violations"]),
            encoding="utf-8",
        )
        print(f"wrote {BASELINE} with {len(result['violations'])} entries")
        return 0
    print(render(result, args.format))
    if args.mode == "verify":
        report = root / "openspec/changes" / change / "scenario-report.md"
        report.write_text(render(result, "markdown"), encoding="utf-8")
    return 0 if not result["blocking"] else 1


if __name__ == "__main__":
    sys.exit(main())
