# 0010: The launcher fetches the pinned engine from GitHub Releases

**Status:** Accepted, provisional · **Date:** 2026-10-02

## Context

Every skill verb runs a 12 MB per-platform engine binary. Binaries stay out of git (0002), so `skill/` ships only the launcher. Skills have no package manager that could deliver a binary, so something has to fetch it.

A checksum from the same release as the binary only guards against corruption, not against a replaced asset. GitHub now covers the rest natively: immutable releases lock a release's assets and tag once published, and build attestations (`actions/attest-build-provenance`, Sigstore) bind each binary's digest to this repository, the release workflow, and the commit.

## Decision

GitHub Releases are the distribution channel, with GitHub's security model: `release-engine.yml` builds five targets, attests each binary, and publishes the release only once every asset is attached, so the repository enforces immutable releases. There are no checksum files in the release; the attestation and the pins replace them.

After the engine release, `scripts/pin-engine.mjs` downloads every asset, verifies its build attestation with `gh attestation verify`, and writes the digests to `skill/scripts/engine.sha256`. The launchers (`impeccino`, `impeccino.cmd`) and `scripts/fetch-engine.mjs` accept a download only if it matches that pin; a version without pins is refused before downloading (development builds use `IMPECCINO_BIN`). A skill release refuses to tag without pins for its engine version.

## Consequences

- The skill commit a skill manager pins also pins the binary bytes, and those bytes are attested to come from this repository's workflow.
- The launcher itself only compares digests; provenance is checked once, when pinning, not on every user's machine.
- Engine assets supply the internal skill runtime (0005). Users install the skill; contributor builds can use `IMPECCINO_BIN` instead of a downloaded asset.
- A skill manager still cannot audit or approve the binary; it only sees the pin file.

## Revisit when

A skill manager can supply per-platform runtime assets itself (Dalo issue #937); the launcher's download step then becomes a fallback.
