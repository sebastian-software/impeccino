# 0003: No self-installer

**Status:** Accepted · **Date:** 2026-10-01

## Context

`npx impeccable install|update|link|check` (crate `crates/skills`) detected
harnesses, downloaded a signed `universal.zip`, copied one variant per
harness, merged hook manifests into project settings, fetched the engine,
and kept no lock file or uninstall. Every third-party skill shipping its own
installer does not scale: each one invents placement, trust, updates, and
removal, and none of them knows about the others.

## Decision

Impeccable does not install itself. It ships like a macOS app bundle: one
self-contained folder (`skill/`) that a skill manager such as Dalo, a
submodule, or a plain copy puts in place. Placement, pinning, updates,
approvals, and removal belong to that manager.

Removed with it: `crates/skills`, the `install`/`link`/`update`/`check` verbs
(the CLI now points to `skill/`), the `universal.zip` bundle and its Ed25519
signing, the per-provider dist layouts, and the provider build that fed them.

## Consequences

- Users without a skill manager copy `skill/` by hand (README, Option 5).
- The npm package remains a detector CLI (`npx impeccable detect`).
- The website's per-harness downloads (separate `impeccable-site` repo) lose
  their source and need to point at `skill/` or a release tag.
