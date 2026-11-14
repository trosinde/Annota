# How Annota is built

Design → Spec → Code → Verify → Package. Every user-visible change gets high-fidelity mockups and
maintainer approval before any code is written. The rule book is [`CLAUDE.md`](../../CLAUDE.md).

```
Propose (/opsx:propose) → Design (designer agent) → Approve (maintainer) → Apply (/opsx:apply) → Verify (reviewer + .deb) → archive
```

## Continue with Claude Code

```bash
git clone https://github.com/trosinde/Annota.git && cd Annota && claude
```

- Next change: `/opsx:apply <id>`; design gate: `/design <id>`
- Agents: `designer`, `gtk-engineer`, `reviewer`, `debian-packager`
- Review all mockups in one page: `python3 tools/build-review-gallery.py` → `docs/design/review.html`

Specs and planned changes live in [`openspec/`](../../openspec/); the feature catalogue maps every
capability to its phase: [`feature-catalog.md`](../feature-catalog.md).
