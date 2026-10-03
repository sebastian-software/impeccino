# 0020: Project state is top-level files, and there is no config file

**Status:** Accepted · **Date:** 2026-10-02

## Context

Impeccino kept its project state in a hidden `.impeccino/` directory: a shared and a personal config file, the DESIGN.md sidecar, one Markdown file per surface brief, an archive of critique snapshots plus an ignore list for them, the hook's session cache, and review screenshots. The README carried a `.gitignore` block so users could sort the files to commit from the ones to ignore. Two problems sat underneath:

- Decisions hid in tool config. "Inter is our brand font" or "the side rail is the ledger's signature" lived in `detector.ignoreValues` of a JSON file the design documents never mentioned, so critique, polish, and the documenter could not see them, and the next DESIGN.md rewrite could contradict them.
- Shared truth and throwaway output shared one folder. Briefs and the sidecar were documents worth reviewing; caches, screenshots, and critique snapshots were not. Nobody should need a gitignore recipe to tell them apart.

The rename to Impeccino (0017) made this the moment to change the layout: no released project reads `.impeccino/` yet.

## Decision

Project state is a few top-level files, all committed; runtime state leaves the project; there is no config file.

| What | Where |
|---|---|
| Product truth | `PRODUCT.md` (unchanged) |
| Visual system | `DESIGN.md` (unchanged) |
| Design sidecar | `DESIGN.json`, next to DESIGN.md (the only location) |
| Surface briefs | `SURFACES.md`, one section per surface: a `## <target>` heading and a `<!-- impeccino:surface {"target":...,"related":[...]} -->` marker. The marker is the authority, so `surface-brief write` replaces exactly one section and a body may carry headings of its own |
| Project-wide detector waivers | `<!-- impeccino-disable <rule>: reason -->` in DESIGN.md, next to the rule that justifies it |
| Deliberate values | DESIGN.md tokens: a declared font never counts as overused; the design-system rules accept every declared value |
| Files not to scan | `.gitignore`, `.git/info/exclude`, and `.gitattributes` `linguist-generated` / `linguist-vendored`, inside a git repository |
| Rejected critique findings | PRODUCT.md (brand commitments, product principles), DESIGN.md (named rules, do's and don'ts), or the project's own ADRs |
| Hook on or off | whether its entries are installed in the harness settings (0007); `IMPECCINO_HOOK_DISABLED`, `IMPECCINO_HOOK_QUIET`, `IMPECCINO_HOOK_LOG` are the only knobs |
| Hook session cache | user cache: `$XDG_CACHE_HOME/impeccino/projects/<project>-<hash>/` (else `~/.cache/...`; `%LOCALAPPDATA%\impeccino\...` on Windows); `IMPECCINO_CACHE_ROOT` still overrides |
| Staleness throttle | `<user cache>/impeccino/staleness-check.json`; `IMPECCINO_NO_STALENESS_CHECK=1` opts out |
| Engine binary | `<user cache>/impeccino/bin/<VERSION>/` (was `~/.impeccino/bin/`); `IMPECCINO_HOME` still overrides |
| Review screenshots | a temporary directory outside the project |
| Critique reports | the chat only; no archive, no trend |

Removed with the config file: the `ignores` verb, the `hooks ignore-rule|ignore-file|ignore-value` actions, the consent record, `projectRoots` (workspaces come from the package manager), and the hook's tuning keys, which are now fixed defaults (server templates such as `.blade.php`, `.twig`, `.erb`, `.hbs` are scanned with the HTML engine by default). Removed with the archive: the `critique-storage` verb and the `critique` block of `signals`. Both verbs answer with a "was removed" message.

The cut is hard: there is no migration code. A leftover `.impeccino/` at a project root is one `mention` finding at boot (a single stat, within Tier 1's budget) and in `doctor`, naming where each part belongs.

## Consequences

- Decisions live in the documents people and agents already read. Critique, audit, and polish drop findings that contradict a recorded decision and offer to record a new one when the user rejects a finding; the documenter carries waivers and declared tokens through every DESIGN.md rewrite.
- A project holds nothing to ignore, so the README's gitignore block is gone. The engine writes `SURFACES.md`, the `PRODUCT.md` schema stamp (`doctor --fix`), and the harness manifests `hooks on` installs; agents write DESIGN.md and DESIGN.json.
- A waiver is only as precise as a rule id. Per-value and per-glob suppressions are gone; a single spot uses an in-file `impeccino-disable-line` comment, a whole file uses git's own rules.
- `polish` no longer inherits a critique on its own; the user hands it one, or it runs its own pass.
- Runtime state no longer travels with a checkout, so a fresh clone starts with an empty hook session, which is what a session is.

## Revisit when

A project needs a detector setting that no design decision or git rule can express. Record why in a new ADR before adding a file for it.
