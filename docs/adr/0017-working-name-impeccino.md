# 0017: Working name "Impeccino"

**Status:** Proposed, not applied · **Date:** 2026-10-02

## Context

This branch has diverged far enough from upstream Impeccable (0001 to 0016) that it is no longer a candidate for an upstream pull request. It reads as its own product: one skill folder managed by a skill manager, no browser stack, no image pipeline.

## Decision

"Impeccino" ("the little Impeccable") is recorded as the working name. Nothing is renamed yet: the skill, the `/impeccable` command, the binary, the crates, the npm package, environment variables, the `.impeccable/` project folder, and inline `impeccable-disable` comments all keep their current names.

## Consequences

- Applying the name later is a single, mechanical change, but a deep one: decide its depth then (user-visible names only, or everything including `.impeccable/` and `IMPECCABLE_*`), and whether the repository becomes its own project instead of a fork.
- Until then, docs keep calling the product Impeccable.
