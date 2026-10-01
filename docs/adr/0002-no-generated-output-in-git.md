# 0002: Nothing generated is tracked

**Status:** Accepted · **Date:** 2026-10-01

## Context

The repository tracked 19 generated harness folders (about 1,000 files),
the Claude and Cursor plugin subtrees, and a copy of the engine version in
`skill/scripts/VERSION`. A workflow committed regenerated output back to
`main` after every source change. Generated files drift, inflate diffs and
reviews, and need their own sync machinery.

## Decision

Only source is tracked. `skill/` is source and the install payload at once
(0001), so git-based consumers (skill managers, submodules, `npx skills`)
read it directly. Root harness folders are ignored local developer state.

## Consequences

- The generated-output sync workflow is gone.
- Anything that needs a derived file must derive it at the consumer, not
  commit it. This rules out committed Codex agent TOMLs (0006) and copied
  LICENSE/NOTICE files inside `skill/` (open, see below).
- Open: Apache-2.0 §4 asks redistributors to pass on the license and notices.
  `skill/` relies on the repository's root LICENSE and NOTICE.md today; moving
  the canonical files into `skill/` would satisfy both without a copy.
