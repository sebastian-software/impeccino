# 0014: Release Please manages one product version

**Status:** Accepted · **Date:** 2026-10-02 · **Updated:** 2026-10-08

## Context

The skill and its engine ship together, but separate manual version bumps let their release numbers diverge. The installed skill also needs the engine's exact digests. Those bytes do not exist until the engine build finishes, so the skill tag must follow engine publication.

## Decision

Use one root Release Please package and the `simple` strategy for this virtual Cargo workspace, following the [project-infra workspace template](https://github.com/sebastian-software/project-infra/blob/main/skills/project-infra/assets/ci/release-please/rust-workspace.json). Release Please updates `.release-please-version`, its manifest, the Rust workspace version, every local lockfile version, and `CHANGELOG.md` in one release PR. Conventional Commit PR titles define the version bump and release notes.

Merging that PR creates a draft `engine-v<version>` release. An explicit dispatch runs the engine workflow on the immutable tag, preserving the attestation's source ref and commit. Five successful builds, the ARM64 artifact smoke check, notices, and build attestations precede publication. Recovery can fill a draft with missing assets; a different existing asset is refused. Public releases are never modified.

After engine publication, the workflow verifies each asset's signing workflow, source digest, and tag ref, then writes its pins and advances both skill metadata and the launcher version to that same product version. A mechanical fast-forward commit to main contains those verified files. The workflow publishes `skill-v<version>` from that commit. The two distribution tags retain their existing names and share one version; the skill receives no independent version bump.

The organization `RELEASE_PLEASE_TOKEN` creates PRs whose CI starts automatically and pushes the pin commit. Main must allow that bot's fast-forward push. This is an explicit release-only exception to ordinary feature PR delivery. A concurrent change makes the push fail; the workflow never force-pushes. Recovery on a newer product version refuses to roll the skill back.

## Consequences

The directly installed `skill/` folder continues to pin attested engine bytes (0010). A release PR still tests the previously published installed engine pin while CI builds and tests the candidate engine from source. The installed skill advances only when the new assets exist and verify.

The first shared release is 0.3.0. Historical skill 0.1.0 and engine 0.2.0 tags remain valid. Bootstrap used the engine 0.2.0 release commit and was removed after Release Please created its first release and tag. Subsequent releases use the engine tag history. Manual release scripts remain recovery tools.
