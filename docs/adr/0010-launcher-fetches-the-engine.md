# 0010: The launcher fetches the pinned engine from GitHub Releases

**Status:** Accepted · **Date:** 2026-10-02 · **Updated:** 2026-10-10

## Context

Every skill verb runs a 12 MB per-platform engine binary. Binaries stay out of git (0002), so `skill/` ships only the launcher. Before Dalo 1.5.0, skill managers could not deliver a binary, so the launcher had to fetch it.

Dalo 1.5.0 now supplies declared GitHub release binaries with exact approvals,
digest verification, and cleanup (Dalo issue #937; Impeccino issue #101).
The launcher still serves skills.sh, older Dalo versions, and Windows.

A checksum from the same release as the binary only guards against corruption, not against a replaced asset. GitHub now covers the rest natively: immutable releases lock a release's assets and tag once published, and build attestations (`actions/attest-build-provenance`, Sigstore) bind each binary's digest to this repository, the release workflow, and the commit.

## Decision

GitHub Releases are the distribution channel, with GitHub's security model: `release-engine.yml` builds five targets, attests each binary, and publishes the release only once every asset is attached, so the repository enforces immutable releases. There are no checksum files in the release; the attestation and the pins replace them.

After the engine release, the automated shared-version flow (0014) calls `scripts/pin-engine.mjs`. The script downloads every asset, verifies its build attestation with `gh attestation verify` against the signing workflow, tag ref, and source commit, and writes the digests to `skill/scripts/engine.sha256`. The launchers (`impeccino`, `impeccino.cmd`) and `scripts/fetch-engine.mjs` accept a download only if it matches that pin; a version without pins is refused before downloading (development builds use `IMPECCINO_BIN`). A skill release refuses to tag without pins for its engine version.

That step also writes the same four macOS/Linux assets and digests to
`binaries.impeccino` in SKILL.md, updates its engine tag and VERSION, and
checks both declarations together offline. Dalo downloads only the host asset
after `dalo approve binary <source>:impeccino#binary:impeccino` and exposes it
at `<store>/bin/impeccino`. Provenance remains our release-time check; Dalo
verifies the reviewed digest.

After an explicit `IMPECCINO_BIN` override, the POSIX launcher tries that Dalo
path before its sibling and cache. It honors `DALO_STORE`; otherwise it searches
parent paths for the nearest `dalo-project.toml`, stopping at a Git boundary,
and selects that project's `.dalo` store or the default `~/.dalo`. The candidate
must match engine.sha256 before any execution and pass the exact-version probe.
Windows stays on the existing launcher path because Dalo has no Windows asset
key. No Dalo command runs at skill startup.

The declaration is `availability: optional`: pending or revoked Dalo approvals
do not prohibit the launcher's fallback download. Dalo supplies an optional
managed installation route, not runtime enforcement of its binary approval.

## Consequences

- The skill commit a skill manager pins also pins the binary bytes, and those bytes are attested to come from this repository's workflow.
- The launcher itself only compares digests; provenance is checked once, when pinning, not on every user's machine.
- Engine assets supply the internal skill runtime (0005). Users install the skill; contributor builds can use `IMPECCINO_BIN` instead of a downloaded asset.
- Dalo exposes the declared binary's approval, verification, lock, and cleanup state; other managers retain the pinned launcher download.

## Revisit when

Dalo adds Windows release binaries, or a host begins enforcing binary approvals
at runtime. Reconsider the platform mapping and optional fallback then.
