# 0003: No self-installer; install with Dalo or skills.sh

**Status:** Accepted · **Date:** 2026-10-02

## Context

`npx impeccino install|update|link|check` (crate `crates/skills`) detected harnesses, downloaded a signed `universal.zip`, copied one variant per harness, merged hook manifests into project settings, fetched the engine, and kept no lock file or uninstall. Every third-party skill shipping its own installer does not scale: each one invents placement, trust, updates, and removal, and none of them knows about the others.

A submodule or a manual copy avoids the installer but leaves pinning, updates, removal, and placement into several harness folders to the user, which is the work a skill manager exists to do.

## Decision

Impeccino does not install itself. It ships like a macOS app bundle: one self-contained folder (`skill/`) that a skill manager puts in place. The README documents two routes:

- [Dalo](https://dalo.sh), recommended: `dalo source add <id> <repo> --ref <branch> --subpath skill`, then `dalo sync`. Dalo pins the commit, links the skill into every harness, owns updates and removal, and gates hooks behind its own approval.
- [skills.sh](https://skills.sh) (`npx skills add <repo>`) for people without Dalo: it finds `skill/`, installs it as `impeccino`, and updates with `npx skills update`.

Removed: `crates/skills`, the `install`/`link`/`update`/`check` verbs (they now print the two routes), the `universal.zip` bundle and its Ed25519 signing, the per-provider dist layouts and the build that fed them, and the submodule and copy instructions.

## Consequences

- One install story per audience: teams and power users use Dalo, everyone else uses skills.sh.
- A submodule or a copy still works, since `skill/` is a plain folder, but it is not documented.
