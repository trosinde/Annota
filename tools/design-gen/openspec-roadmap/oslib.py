"""Tiny helper to write OpenSpec changes from compact Python data."""
import os, textwrap, datetime

ROOT = "/home/claude/annota/openspec/changes"


def R(name, text, *scenarios):
    return (name, text, scenarios)


def S(name, when, then, *ands):
    return (name, when, then, ands)


def _spec(purpose, reqs):
    out = ["# Spec Delta", ""]
    if purpose:
        out += ["## Purpose", purpose, ""]
    out += ["## ADDED Requirements", ""]
    for name, text, scns in reqs:
        out += [f"### Requirement: {name}", text, ""]
        for sname, when, then, ands in scns:
            out += [f"#### Scenario: {sname}", f"- **WHEN** {when}", f"- **THEN** {then}"]
            out += [f"- **AND** {a}" for a in ands]
            out.append("")
    return "\n".join(out)


def change(cid, phase, title, why, what, covers, out_of_scope, caps, decisions, tasks, impact):
    """caps: list of (cap_name, purpose_or_None, [R(...)])"""
    d = os.path.join(ROOT, cid)
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, ".openspec.yaml"), "w") as f:
        f.write(f"schema: spec-driven\ncreated: {datetime.date.today()}\n")

    new_caps = [c for c, p, _ in caps if p]
    mod_caps = [c for c, p, _ in caps if not p]
    proposal = [
        f"# {title}", "",
        f"**Roadmap phase:** {phase}", "",
        "## Why", why.strip(), "",
        "## What Changes", *[f"- {w}" for w in what], "",
        "## Benchmark features covered", *[f"- {c}" for c in covers], "",
        "## Out of scope", *[f"- {o}" for o in out_of_scope], "",
        "## Capabilities", "",
        "### New Capabilities",
        *([f"- `{c}`" for c in new_caps] or ["- none"]), "",
        "### Modified Capabilities",
        *([f"- `{c}`" for c in mod_caps] or ["- none"]), "",
        "## Impact", *[f"- {i}" for i in impact], "",
    ]
    with open(os.path.join(d, "proposal.md"), "w") as f:
        f.write("\n".join(proposal))

    design = [
        f"# Design: {title}", "",
        "## Mockups (design gate)", "",
        f"Mockups live in `docs/design/{cid}/` and are produced by the `designer` agent.",
        f"Scenario-to-mockup mapping: `docs/design/{cid}/index.md`.", "",
        "Design approved: _pending_", "",
        "## Technical decisions", *[f"- {x}" for x in decisions], "",
        "## Wayland / X11", "- Behaviour is identical unless stated in a decision above.", "",
        "## Risks / open questions", "- To be completed during `/opsx:propose` refinement.", "",
    ]
    with open(os.path.join(d, "design.md"), "w") as f:
        f.write("\n".join(design))

    for cap, purpose, reqs in caps:
        sd = os.path.join(d, "specs", cap)
        os.makedirs(sd, exist_ok=True)
        with open(os.path.join(sd, "spec.md"), "w") as f:
            f.write(_spec(purpose, reqs))

    t = ["# Tasks", "", "## 1. Design gate",
         f"- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/{cid}/` (light + dark, 1x + 2x)",
         "- [ ] 1.2 Maintainer approval recorded in design.md", ""]
    n = 2
    for group, items in tasks:
        t.append(f"## {n}. {group}")
        for i, it in enumerate(items, 1):
            t.append(f"- [ ] {n}.{i} {it}")
        t.append("")
        n += 1
    t += [f"## {n}. Verify",
          f"- [ ] {n}.1 Tests for every scenario (unit, golden-image or documented manual check)",
          f"- [ ] {n}.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green",
          f"- [ ] {n}.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean",
          f"- [ ] {n}.4 German translation updated", ""]
    with open(os.path.join(d, "tasks.md"), "w") as f:
        f.write("\n".join(t))
