# 0014: Releases are tags with generated notes

**Status:** Accepted · **Date:** 2026-10-02

## Context

`scripts/release.mjs` required a changelog entry in `site/pages/changelog.astro`, a file in the private website repository, rendered a tweet from it, and checked that impeccable.style served the new version. Releasing from this repository depended on a checkout nobody else has.

## Decision

A release is an annotated tag per component (`skill-v`, `cli-v`, `engine-v`). GitHub generates the notes from the commits since the component's previous tag. No component builds or uploads artifacts in the release script; engine binaries are built by `release-engine.yml`.

## Consequences

- Commit messages are the release notes, so they should describe user-facing impact.
- The website changelog, if kept, is maintained in the website repository.
