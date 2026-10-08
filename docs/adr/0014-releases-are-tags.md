# 0014: Release Please manages one product version

**Status:** Accepted · **Date:** 2026-10-02 · **Updated:** 2026-10-08

## Context

The skill and its engine ship together, but separate manual version bumps let their release numbers diverge. The installed skill also needs the engine's exact digests. Those bytes do not exist until the engine build finishes, so the skill tag must follow engine publication.

## Decision

Use one root Release Please package and the `simple` strategy for this virtual Cargo workspace, following the [project-infra workspace template](https://github.com/sebastian-software/project-infra/blob/main/skills/project-infra/assets/ci/release-please/rust-workspace.json). Release Please updates `.release-please-version`, its manifest, the Rust workspace version, every local lockfile version, and `CHANGELOG.md` in one release PR. Conventional Commit PR titles define the version bump and release notes.

Merging that PR creates a draft `engine-v<version>` release. An explicit dispatch runs the engine workflow on the immutable tag, preserving the attestation's source ref and commit. Five successful builds, the ARM64 artifact smoke check, notices, and build attestations precede publication. Recovery can fill a draft with missing assets; a different existing asset is refused. Public releases are never modified.

After engine publication, the workflow verifies each asset's signing workflow, source digest, and tag ref, then writes its pins and advances both skill metadata and the launcher version to that same product version. The main-branch `release-skill.yml` workflow opens a mechanical pin PR containing only those three verified files. After that PR is merged and its main push CI succeeds, it reverifies the committed pins and publishes `skill-v<version>` from the tested commit. The two distribution tags retain their existing names and share one version; the skill receives no independent version bump.

The organization `RELEASE_PLEASE_TOKEN` creates release and pin PRs whose CI starts automatically. Main retains its required status checks; no direct push or protection bypass is needed. Pin PRs are merged through the normal review process. The workflow never force-pushes. Recovery on a newer product version refuses to roll the skill back.

## Consequences

The directly installed `skill/` folder continues to pin attested engine bytes (0010). A release PR still tests the previously published installed engine pin while CI builds and tests the candidate engine from source. The installed skill advances only when the new assets exist and verify.

The shared flow uses 0.3.1 for the corrected release. Historical skill 0.1.0 and engine 0.2.0 tags remain valid. The protected engine 0.3.0 tag stays reserved after an empty draft failed; correcting a tagged workflow requires a new patch release. Tags are never moved, including unpublished ones. Bootstrap used the engine 0.2.0 release commit and was removed after Release Please created its first release and tag. Subsequent releases use the engine tag history. Skill delivery can be retried on main with `gh workflow run release-skill.yml --ref main -f engine_tag=engine-v<version>` without moving the engine tag or rebuilding published binaries. Manual release scripts remain recovery tools.
