# Design

Design approved: 2026-10-30

## Context

See `proposal.md` - Why. Today `.github/workflows/ci.yml` has one job, `build-deb`
(`ubuntu-24.04`): fmt → clippy → `meson test` → `openspec validate --all --strict` →
`build-aux/make-deb.sh` (vendors crates, `dpkg-buildpackage -us -uc -b`, `lintian --fail-on error`)
→ upload as a CI artifact, and on a `v*` tag attach the `.deb` to the GitHub Release
(`softprops/action-gh-release@v2`). Nothing is signed. There is no GPG key anywhere in the project,
no APT-repo tooling, and no GitHub Actions-based Pages deploy — `docs/index.html` is served by
GitHub Pages configured (in repo Settings → Pages) to build straight from `docs/` on `main`. This
capability has no GTK UI; nothing here needs mockups or maintainer design-mockup approval under
CLAUDE.md §5 — only the proposal/spec/tasks approval gate applies.

## Goals / Non-Goals

**Goals:**
- Every release signed end-to-end so `apt` itself verifies it, with zero manual steps for users
  beyond "add this repo, import this key" once.
- No secret ever committed to the repo; the private key lives only in GitHub Actions secrets.
- Keep the existing unsigned-`.deb`-on-the-Release path working unchanged.

**Non-Goals:**
- Multiple architectures or suites (this change covers `amd64`/`noble` only, matching the current
  CI runner; arm64 or Debian-trixie repo variants are future work).
- Launchpad PPA upload (explicitly deferred per the `debian-packager` agent brief).
- Changing `debian/control`, runtime dependencies, or the application itself.

## Decisions

**Key type and storage**: generate an Ed25519 GPG key (`gpg --quick-generate-key`, no passphrase,
`annota-release-signing@trosinde.github.io` style identity, no expiry shorter than the project's
realistic maintenance horizon — set a 2-year expiry, documented rotation covers renewal). Export
the private key armored and base64-encode it into the `ANNOTA_RELEASE_SIGNING_KEY` GitHub Actions
repository secret. The public key is committed to the repo at
`packaging/annota-archive-keyring.asc` (plain text, safe to commit) and copied into the published
repo tree so it is fetchable from the same site. Alternative considered: a key with a passphrase
stored as a second secret — rejected for this change because it adds a CI-secret-handling step
for no real benefit (the private key material itself is already the protected secret; GitHub
Actions secrets are already encrypted at rest and only injected into the job).

**APT repo tooling**: use `apt-ftparchive` (already in `devscripts`/`apt-utils`, no new heavyweight
dependency like `reprepro`/`aptly` which assume a persistent repo database) to generate
`Packages`/`Packages.gz` from a `pool/` directory holding the built `.deb`, then hand-write the
`Release` file's required fields and sign it with `gpg --detach-sign --armor` (produces
`Release.gpg`) and `gpg --clearsign` (produces `InRelease`). This is the same minimal, stateless
approach used by many single-package project repos; no database to maintain across releases since
each tag rebuilds the full (small) repo tree from scratch.

**Publishing to GitHub Pages**: switch the repository's Pages source (one-time, manual, in
repo Settings → Pages — not something this change's CI can do) from "Deploy from a branch
(`main`/`docs`)" to "GitHub Actions", and add an `actions/deploy-pages` step to the release job
that uploads an artifact combining the existing `docs/` tree with the generated `docs/apt/` repo
tree. Alternative considered: have CI commit the generated `Packages`/`Release`/`InRelease` files
straight into `docs/apt/` on `main` — rejected because it means a bot-authored commit on every
tag, binary-looking signed metadata living permanently in git history and growing with every
release, and a `GITHUB_TOKEN` with write-back permission on `main`; the Actions-deploy path keeps
`main` exactly as the source of truth and the published repo tree as a build output, matching how
the `.deb` itself is already treated (built artifact, not committed).

**Repo layout served**: `docs/apt/dists/noble/main/binary-amd64/{Packages,Packages.gz}`,
`docs/apt/dists/noble/{Release,InRelease}`, `docs/apt/pool/main/annota_<version>_amd64.deb`,
`docs/annota-archive-keyring.asc` (also mirrored at `packaging/annota-archive-keyring.asc` in the
source tree). `README.md` documents:
```
curl -fsSL https://trosinde.github.io/Annota/annota-archive-keyring.asc \
  | sudo tee /usr/share/keyrings/annota-archive-keyring.asc
echo "deb [signed-by=/usr/share/keyrings/annota-archive-keyring.asc] \
  https://trosinde.github.io/Annota/apt noble main" \
  | sudo tee /etc/apt/sources.list.d/annota.list
sudo apt update && sudo apt install annota
```

**Key rotation**: documented as a short runbook in `README.md`/`docs/adr/`: generate a new key,
publish its public part alongside (not instead of) the old one for one release cycle, sign that
release with the new key, announce in the GitHub Release notes which key signs which version, and
only remove the old public key from the published keyring once no supported release still needs
it.

## Risks / Trade-offs

- [GitHub Pages "Deploy from a branch" → "Actions" switch is a manual repo-settings change] →
  mitigated by calling it out explicitly as a one-time task in `tasks.md`; CI cannot do this for
  itself, so the first release under this change needs the maintainer to flip that setting before
  the workflow run that publishes `docs/apt/` will actually take effect.
- [A single `amd64`/`noble` repo understates what the `.deb` already claims to support (Ubuntu
  26.04, Debian 13, `Architecture: any`)] → mitigated by stating this as an explicit Non-Goal and
  leaving multi-suite/arch repo layout as clearly scoped-out future work, not a silent gap.
- [Losing the signing key loses the ability to publish trusted upgrades] → mitigated by the
  documented rotation runbook and by keeping the key passphrase-less-but-secret-protected rather
  than relying on a human remembering a passphrase during CI.

## Open Questions

None — the repository-settings switch and key-rotation runbook are tasks, not deferred unknowns;
everything above is a chosen approach reflected in `tasks.md`.
