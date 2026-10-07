# 0002: Nothing generated is tracked

**Status:** Accepted · **Date:** 2026-10-01

## Context

The repository tracked 19 generated harness folders (about 1,000 files),
the Claude and Cursor plugin subtrees, and a copy of the engine version in
`skill/scripts/VERSION`. A workflow committed regenerated output back to
`main` after every source change. Generated files drift, inflate diffs and
reviews, and need their own sync machinery.

## Decision

Build output and generated files are not tracked, except the public README composed by mdtheme from reviewed source and a pinned shared theme. `skill/` is source and the
install payload at once (0001), so git-based consumers (skill managers,
submodules, `npx skills`) read it directly. Maintained license and notice
files required by a standalone install are part of that source payload, not
generated copies. Root harness folders are ignored local developer state.

## Consequences

- The generated-output sync workflow is gone.
- Anything that needs a derived build file must derive it where it is
  consumed, not commit it. This rules out committed Codex agent TOMLs (0006)
  and generated engine output.
- Root `LICENSE` and `NOTICE.md` remain maintained source for repository and
  GitHub consumers. The installable `skill/` bundle also carries a byte-for-
  byte copy of the Apache `LICENSE` plus a scoped `NOTICE.md` with the full
  MIT notice for its platform references. These static redistribution files
  are an intentional exception to the generated-output rule, not build
  output.
- Engine release `THIRD-PARTY-NOTICES.txt` remains generated and untracked; the
  release workflow derives it from the locked Cargo dependency graph.

- `README.md` is the explicit publication exception: edit `README.md.src`, generate with the project-pinned mdtheme, review the diff, and commit both. CI verifies exact composition; no workflow commits regenerated output automatically. `mise.lock` records the tool archive checksums.
