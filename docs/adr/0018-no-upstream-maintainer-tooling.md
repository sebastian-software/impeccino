# 0018: No upstream maintainer tooling

**Status:** Accepted · **Date:** 2026-10-02

## Context

The fork inherited tooling that serves the upstream project and its maintainer, not this repository:

- `.github/CODEOWNERS` requested a review from the upstream maintainer on every pull request.
- The PR "sheriff" workflow (`sheriff.yml`, `scripts/github/sheriff.mjs`) labeled, warned, and auto-closed pull requests for upstream's triage process.
- `release-engine.yml` signed the Windows binary through Azure Artifact Signing with the upstream maintainer's company certificate. Without those credentials the release workflow fails.
- `PRODUCT.md` in the root was the product context of the impeccable.style website, which lives in another repository; `skills-lock.json` was an empty skills.sh lock file; `.gitignore` listed website, video, talk, and eval folders that do not exist here.

## Decision

Removed: CODEOWNERS, the sheriff workflow, script, and tests, the Windows signing job and `docs/WINDOWS-SIGNING.md`, the root `PRODUCT.md`, `skills-lock.json`, and the stale `.gitignore` entries.

## Consequences

- The engine release publishes unsigned binaries for every platform. The launcher verifies each download against its `.sha256` sidecar from the same release, which guards against corruption, not a compromised release (0010).
- Windows may show a SmartScreen prompt for a downloaded binary; the launcher runs it without one.
- Contributor triage, if this repository needs it, gets its own setup.
