# 0004: No update check

**Status:** Accepted · **Date:** 2026-10-01

## Context

`impeccable context` called `impeccable.style/api/version` once a day and
told the agent to offer `npx impeccable update` (`UPDATE_AVAILABLE`). With a
skill manager pinning an exact commit, that notice is noise at best and an
instruction to bypass the pin at worst, and it is a network call on every
session start.

## Decision

The engine no longer checks for updates. Whoever installed the skill updates
it.

## Consequences

- No phone-home from `context`; `IMPECCABLE_NO_UPDATE_CHECK` and the
  `updateCheck` config key have nothing left to switch off (the key stays
  tolerated in existing configs).
- `doctor` describes tool-version drift as the installer's job.
