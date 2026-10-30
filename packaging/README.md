# packaging/

`annota-archive-keyring.asc` — the public half of Annota's APT release-signing key(s), committed
here so it can be fetched by users and served from GitHub Pages. Current key: Ed25519,
`Annota Release Signing <releases@trosinde.github.io>`, fingerprint
`49EB 5EA3 FEAF 7602 E0CE  C8AD DB0B 5DE0 E31E 2C9A`, expires 2028-10-29. The matching private key
lives only in the `ANNOTA_RELEASE_SIGNING_KEY` GitHub Actions secret. See
`docs/adr/0001-signed-apt-repo.md` for the rotation/revocation runbook.
