# 0020: Install with Dalo or skills.sh

**Status:** Accepted · **Date:** 2026-10-02

## Context

After 0003 removed the self-installer, the README offered three routes: a skill manager, a Git submodule, and a manual copy. A submodule and a copy each leave pinning, updates, removal, and placement into several harness folders to the user, which is the work a skill manager exists to do. The submodule route also mounted the repository at `.impeccable/`, the folder the skill uses for project state.

## Decision

The README documents two routes:

- [Dalo](https://dalo.sh), recommended: `dalo source add … --ref <branch> --subpath skill`, then `dalo sync`. Dalo pins the commit, links the skill into every harness, owns updates and removal, and gates hooks behind its own approval.
- [skills.sh](https://skills.sh) (`npx skills add <repo>`) for people without Dalo: it finds `skill/`, installs it as `impeccable`, and updates with `npx skills update`.

Submodule and copy instructions are removed. They still work, since `skill/` is a plain folder, but they are not documented.

## Consequences

- One install story per audience: teams and power users use Dalo; everyone else uses skills.sh.
- Messages from retired installer verbs point to these two routes.
