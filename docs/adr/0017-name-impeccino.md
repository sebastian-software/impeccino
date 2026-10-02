# 0017: The project is called Impeccino

**Status:** Accepted · **Date:** 2026-10-02

## Context

This project started as a branch of Impeccable (github.com/pbakaus/impeccable) and diverged far enough (0001 to 0016) that it is no longer a candidate for an upstream pull request. It reads as its own product: one skill folder managed by a skill manager, no installer, no packages, no browser stack, no image pipeline. Keeping the name would confuse users of both projects and suggest the upstream maintainer stands behind these changes.

## Decision

The project is called Impeccino ("the little Impeccable") and lives in its own repository, github.com/sebastian-software/impeccino. The rename is a hard cut with no compatibility layer: the skill and its `/impeccino` command, the shipped agents (`impeccino-finish-reviewer`, `impeccino-documenter`), the binary and launcher, the crates (`impeccino-*`), environment variables (`IMPECCINO_*`), the project folder (`.impeccino/`, since replaced by top-level files, 0020), inline ignore comments (`impeccino-disable`), document stamps (`<!-- impeccino:... -->`), and release assets (`impeccino-<os>-<arch>`).

## Consequences

- Projects that used Impeccable keep their PRODUCT.md and DESIGN.md (the names are unchanged), but `.impeccable/` files, `IMPECCABLE_*` variables, and `impeccable-disable` comments are not read. Rename them by hand.
- The Apache-2.0 license and Impeccable's copyright notice stay; NOTICE.md and the README credit Impeccable.
- Engine releases are published from the new repository; the launcher downloads from there.
- `concept-seed` still uses Impeccable's public catalog API (`impeccable.style/api`).
