# 0010: The launcher still fetches the pinned engine

**Status:** Accepted, provisional · **Date:** 2026-10-01

## Context

Every skill verb runs a 12 MB per-platform engine binary. Binaries stay out
of git (0002), so `skill/` ships only the launcher. On first run the launcher
downloads `engine-v<VERSION>` for the host platform into `~/.impeccino/bin/`
and verifies it against the release's `.sha256` file.

## Decision

Keep the launcher download for now; it is the one part of delivery that
still lives outside the skill manager.

## Consequences

- The checksum comes from the same release as the binary, so it guards
  against corruption, not a compromised release.
- A skill manager cannot audit or pin the binary bytes, only the version.

## Revisit when

A skill manager can supply per-platform runtime assets, or `skill/` carries
the expected per-platform digests so a pinned commit also pins the bytes.
