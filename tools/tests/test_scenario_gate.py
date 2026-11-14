"""Tests for scenario_gate.py. Run: python3 -m unittest test_scenario_gate.py (from this folder)."""

from __future__ import annotations

import contextlib
import io
import os
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import scenario_gate as gate  # noqa: E402

MAIN_SPEC = """\
# capture

## Requirements

### Requirement: Region capture
The app SHALL capture a region.

#### Scenario: Select and confirm
- WHEN the user drags
- THEN a region is captured

#### Scenario: Cancel
- WHEN the user presses Esc
- THEN nothing is captured
"""

CHANGE_SPEC = """\
## MODIFIED Requirements

### Requirement: Region capture
The app SHALL capture a region.

#### Scenario: Select and confirm
- WHEN the user drags
- THEN a region is captured

#### Scenario: Cancel
- WHEN the user presses Esc
- THEN nothing is captured

#### Scenario: Pointer inside
- WHEN the pointer is inside
- THEN it is drawn

## ADDED Requirements

### Requirement: Pointer preview
#### Scenario: Preview shown
- WHEN include pointer is on
- THEN a preview is shown
"""

RUST_CONFIG = """\
test_globs = ["src/**/*.rs"]
test_marker = '^\\s*#\\[test\\]'
name_regex = 'fn\\s+(\\w+)'
comment = "//"

[[run]]
name = "unit"
cmd = "python3 fake_cargo.py"

[[run]]
name = "live"
select = '#\\[ignore'
per_test = true
cmd = "python3 fake_cargo.py --ignored {name}"
"""

FAKE_CARGO = """\
import sys, json
results = json.load(open("results.json"))
ignored = "--ignored" in sys.argv
for name, (status, live) in results.items():
    if ignored and sys.argv[-1] not in name:
        continue
    if live and not ignored:
        print(f"test {name} ... ignored")
    elif live == ignored:
        print(f"test {name} ... {status}")
"""


class Project:
    def __init__(self, tmp: Path):
        self.root = tmp
        self.write("openspec/specs/capture/spec.md", MAIN_SPEC)
        self.write("openspec/scenario-gate.toml", RUST_CONFIG)
        self.write("fake_cargo.py", FAKE_CARGO)
        self.results({})

    def write(self, rel: str, text: str) -> None:
        p = self.root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(textwrap.dedent(text), encoding="utf-8")

    def results(self, data: dict) -> None:
        import json
        self.write("results.json", json.dumps(data))

    def gate(self, *argv: str) -> tuple[int, str]:
        old = os.getcwd()
        os.chdir(self.root)
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf):
                code = gate.main(list(argv))
        finally:
            os.chdir(old)
        return code, buf.getvalue()


TAGGED_BOTH = """\
// spec: capture / Region capture / Select and confirm
#[test]
fn drag_selects() {}

/// spec: capture / Region capture / Cancel
#[test]
fn esc_cancels() {}
"""


class GateTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.p = Project(Path(self._tmp.name))

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def test_all_scenarios_tagged_passes(self):
        self.p.write("src/capture.rs", TAGGED_BOTH)
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)

    def test_untagged_test_fails(self):
        self.p.write("src/capture.rs", TAGGED_BOTH + "\n#[test]\nfn helper_math() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)
        self.assertIn("untagged-test: src/capture.rs::helper_math", out)

    def test_uncovered_scenario_fails(self):
        self.p.write("src/capture.rs", "// spec: capture / Region capture / Cancel\n#[test]\nfn esc() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)
        self.assertIn("uncovered-scenario: capture / Region capture / Select and confirm", out)

    def test_orphan_tag_fails(self):
        self.p.write("src/capture.rs", TAGGED_BOTH + "\n// spec: capture / Region capture / Typo\n#[test]\nfn t() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)
        self.assertIn("orphan-tag: src/capture.rs::t -> capture / Region capture / Typo", out)

    def test_tag_on_active_change_scenario_is_not_orphan(self):
        self.p.write("openspec/changes/pointer/specs/capture/spec.md", CHANGE_SPEC)
        self.p.write("src/capture.rs", TAGGED_BOTH + "\n// spec: capture / Pointer preview / Preview shown\n#[test]\nfn p() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)

    def test_attributes_between_tag_and_marker(self):
        self.p.write("src/capture.rs", TAGGED_BOTH.replace(
            "/// spec: capture / Region capture / Cancel\n",
            "// spec: capture / Region capture / Cancel\n#[cfg(feature = \"x\")]\n"))
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)

    def test_valid_manual_check_covers(self):
        self.p.write("src/capture.rs", "// spec: capture / Region capture / Cancel\n#[test]\nfn esc() {}\n")
        self.p.write("openspec/changes/archive/x/manual-checks.md", """\
            | Spec | Check | Why not automated | Result | Date |
            |---|---|---|---|---|
            | capture / Region capture / Select and confirm | drag on a real desktop | real portal | pass | 2026-11-14 |
            """)
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)

    def test_manual_check_not_run_does_not_cover(self):
        self.p.write("src/capture.rs", "// spec: capture / Region capture / Cancel\n#[test]\nfn esc() {}\n")
        self.p.write("openspec/changes/archive/x/manual-checks.md", """\
            | Spec | Check | Why not automated | Result | Date |
            |---|---|---|---|---|
            | capture / Region capture / Select and confirm | drag | real portal | not run | |
            """)
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)

    def test_manual_check_without_reason_does_not_cover(self):
        self.p.write("src/capture.rs", "// spec: capture / Region capture / Cancel\n#[test]\nfn esc() {}\n")
        self.p.write("openspec/changes/archive/x/manual-checks.md", """\
            | Spec | Check | Why not automated | Result | Date |
            |---|---|---|---|---|
            | capture / Region capture / Select and confirm | drag |  | pass | 2026-11-14 |
            """)
        code, _ = self.p.gate("check")
        self.assertEqual(code, 1)

    def test_baseline_allows_known_and_rejects_new_and_stale(self):
        self.p.write("src/capture.rs", "#[test]\nfn old() {}\n")
        code, _ = self.p.gate("check", "--write-baseline")
        self.assertEqual(code, 0)
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)
        # a new untagged test is never allowed
        self.p.write("src/more.rs", "#[test]\nfn new_one() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)
        self.assertIn("untagged-test: src/more.rs::new_one", out)
        # fixing a baselined violation forces deleting the entry
        (self.p.root / "src/more.rs").unlink()
        self.p.write("src/capture.rs", "// spec: capture / Region capture / Cancel\n#[test]\nfn old() {}\n")
        code, out = self.p.gate("check")
        self.assertEqual(code, 1)
        self.assertIn("stale-baseline-entry", out)

    def _change_with_tests(self):
        self.p.write("openspec/changes/pointer/specs/capture/spec.md", CHANGE_SPEC)
        self.p.write("src/capture.rs", TAGGED_BOTH + textwrap.dedent("""\

            // spec: capture / Region capture / Pointer inside
            #[test]
            fn pointer_inside() {}

            // spec: capture / Pointer preview / Preview shown
            #[test]
            #[ignore]
            fn preview_live() {}
            """))

    def test_verify_green(self):
        self._change_with_tests()
        self.p.results({
            "capture::tests::drag_selects": ["ok", False],
            "capture::tests::esc_cancels": ["ok", False],
            "capture::tests::pointer_inside": ["ok", False],
            "capture::tests::preview_live": ["ok", True],
        })
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 0, out)
        report = (self.p.root / "openspec/changes/pointer/scenario-report.md").read_text()
        self.assertIn("capture / Pointer preview / Preview shown", report)
        self.assertIn("**PASS**", report)

    def test_verify_red_test_fails(self):
        self._change_with_tests()
        self.p.results({
            "capture::tests::drag_selects": ["ok", False],
            "capture::tests::esc_cancels": ["ok", False],
            "capture::tests::pointer_inside": ["FAILED", False],
            "capture::tests::preview_live": ["ok", True],
        })
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 1)
        self.assertIn("test failed: src/capture.rs::pointer_inside", out)

    def test_verify_live_test_not_run_fails(self):
        self._change_with_tests()
        self.p.results({
            "capture::tests::drag_selects": ["ok", False],
            "capture::tests::esc_cancels": ["ok", False],
            "capture::tests::pointer_inside": ["ok", False],
        })
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 1)
        self.assertIn("test not run: src/capture.rs::preview_live", out)

    def test_verify_change_scenario_cannot_hide_in_baseline(self):
        self.p.write("openspec/changes/pointer/specs/capture/spec.md", CHANGE_SPEC)
        self.p.write("src/capture.rs", TAGGED_BOTH)
        self.p.write("openspec/scenario-baseline.txt",
                     "uncovered-scenario: capture / Pointer preview / Preview shown\n")
        self.p.results({"capture::tests::drag_selects": ["ok", False],
                        "capture::tests::esc_cancels": ["ok", False]})
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 1)
        self.assertIn("uncovered-scenario: capture / Pointer preview / Preview shown", out)

    def test_verify_change_manual_not_run_fails(self):
        self._change_with_tests()
        self.p.write("openspec/changes/pointer/manual-checks.md", """\
            | Spec | Check | Why not automated | Result | Date |
            |---|---|---|---|---|
            | capture / Pointer preview / Preview shown | look at it | needs eyes | not run | |
            """)
        self.p.results({
            "capture::tests::drag_selects": ["ok", False],
            "capture::tests::esc_cancels": ["ok", False],
            "capture::tests::pointer_inside": ["ok", False],
            "capture::tests::preview_live": ["ok", True],
        })
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 1)
        self.assertIn("manual check not passed", out)

    def test_removed_requirement_is_not_implemented_in_verify(self):
        self.p.write("openspec/changes/drop/specs/capture/spec.md",
                     "## REMOVED Requirements\n\n### Requirement: Region capture\n")
        code, out = self.p.gate("verify", "drop")
        self.assertEqual(code, 0, out)

    def test_scenario_dropped_by_change_orphans_its_tag(self):
        self._change_with_tests()
        self.p.write("openspec/changes/pointer/specs/capture/spec.md",
                     CHANGE_SPEC.replace("#### Scenario: Cancel\n", "#### Scenario: Abort\n"))
        self.p.results({})
        code, out = self.p.gate("verify", "pointer")
        self.assertEqual(code, 1)
        self.assertIn("orphan-tag: src/capture.rs::esc_cancels -> capture / Region capture / Cancel", out)

    def test_pytest_style(self):
        self.p.write("openspec/scenario-gate.toml", """\
            test_globs = ["tests/**/test_*.py"]
            test_marker = '^\\s*def test_'
            name_regex = 'def (test_\\w+)'
            comment = "#"

            [[run]]
            cmd = "python3 fake_pytest.py"
            junit = "junit.xml"
            """)
        self.p.write("tests/test_capture.py", """\
            # spec: capture / Region capture / Select and confirm
            @pytest.mark.slow
            def test_drag():
                pass

            # spec: capture / Region capture / Cancel
            def test_esc():
                pass
            """)
        self.p.write("fake_pytest.py", """\
            open("junit.xml", "w").write(
                '<testsuite><testcase classname="tests.test_capture" name="test_drag"/>'
                '<testcase classname="tests.test_capture" name="test_esc"><failure/></testcase></testsuite>')
            """)
        self.p.write("openspec/changes/noop/proposal.md", "x")
        code, out = self.p.gate("check")
        self.assertEqual(code, 0, out)
        code, out = self.p.gate("verify", "noop")
        self.assertEqual(code, 1)
        self.assertIn("test failed: tests/test_capture.py::test_esc", out)


if __name__ == "__main__":
    unittest.main()
