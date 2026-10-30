# ADR 0001: Signed APT repository for releases

Status: accepted (OpenSpec change `add-signed-apt-repo`)

## Decision

Releases are additionally distributed through a GPG-signed APT repository published to GitHub
Pages, so `apt` verifies every install and upgrade against Annota's release-signing key. The
unsigned `.deb` on the GitHub Release stays available as a direct-download alternative.

Key type: Ed25519, 2-year expiry. Repo tooling: `apt-ftparchive` (stateless, rebuilt from scratch
on every tag — no persistent repo database). Publishing: GitHub Actions `actions/deploy-pages`,
combining the existing `docs/` tree with a repo tree generated at release time under `docs/apt/`
(never committed to `main`).

See `openspec/changes/add-signed-apt-repo/design.md` for the full rationale and alternatives
considered.

## Generating the release-signing key (maintainer-only, run on a trusted machine)

Do this on a machine you trust with the private key material, not in a disposable CI runner or
cloud sandbox:

```bash
gpg --quick-generate-key "Annota Release Signing <releases@trosinde.github.io>" ed25519 sign 2y
KEYID=$(gpg --with-colons --list-secret-keys releases@trosinde.github.io \
  | awk -F: '/^sec/ {print $5; exit}')

# Public key: committed to the repo and served on GitHub Pages
gpg --export --armor "$KEYID" > packaging/annota-archive-keyring.asc

# Private key: goes into the GitHub Actions secret, never into git
gpg --export-secret-keys --armor "$KEYID" | base64 -w0 \
  | gh secret set ANNOTA_RELEASE_SIGNING_KEY --repo trosinde/Annota
```

Commit only `packaging/annota-archive-keyring.asc`.

## Rotating or revoking the key

1. Generate the new key the same way, with a new identity comment noting the rotation date
   (e.g. `Annota Release Signing 2028 <releases@trosinde.github.io>`).
2. Append (do not replace) its public part into `packaging/annota-archive-keyring.asc`, so a
   keyring import picks up both the old and the new key — `gpg --dearmor` and `apt`'s
   `signed-by` accept a keyring file containing multiple keys.
3. Update the `ANNOTA_RELEASE_SIGNING_KEY` secret to the new private key.
4. Sign the next release with the new key; name both key fingerprints in that release's notes
   ("signed with <new fingerprint>; previous releases were signed with <old fingerprint>").
5. Keep the old public key in the published keyring for at least one more release cycle, so users
   who have not re-run the "import keyring" step yet can still verify their already-installed
   version's provenance, then drop it from `packaging/annota-archive-keyring.asc`.
6. To revoke a compromised key immediately instead of rotating on a normal cycle: generate a
   revocation certificate (`gpg --gen-revoke`) ahead of time and keep it offline; if revocation is
   ever needed, publish the revoked key immediately, generate the replacement, and treat it as an
   emergency release announcement, not a routine rotation.
