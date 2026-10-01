# 0009: The engine version lives in skill/scripts/VERSION

**Status:** Accepted · **Date:** 2026-10-01

## Context

The root `ENGINE_VERSION` file pinned the engine release, and the build
copied it to `skill/scripts/VERSION`, which the launcher reads. Without a
build (0003), two files would drift.

## Decision

`skill/scripts/VERSION` is the only engine pin. Release, fetch, CI, and the
release-engine workflow read it; a test pins the npm platform packages to it.

## Consequences

- Pinning a skill commit also pins the engine version.
