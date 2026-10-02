# 0008: Frontmatter carries the harness keys runtimes tolerate

**Status:** Accepted · **Date:** 2026-10-02

## Context

The per-harness variants carried different frontmatter: the Claude variant had `user-invocable`, `argument-hint`, and `allowed-tools`, the Codex variant only spec fields. One universal SKILL.md needs one set.

Codex's runtime (0.159.2) loads a skill with `user-invocable` and `argument-hint` without a warning, and the other harnesses ignore keys they do not know (docs/HARNESSES.md). Only strict validators reject them: Codex's bundled `quick_validate.py` (an authoring aid from its skill creator, upstream issue #701) and `skills-ref validate`.

## Decision

SKILL.md uses the Agent Skills spec fields (`name`, `description`, `license`, `compatibility`, `metadata` with `metadata.version`) plus the harness keys that carry information and that runtimes tolerate:

- `user-invocable: true`
- `argument-hint` listing the commands and a target, which Claude Code shows in the slash-command picker.

`compatibility` names what the machine needs (the launcher's first-run download, agent-browser for rendered-page scans). `allowed-tools` stays out: Claude Code blocks skill activation in non-interactive sessions when it is set. A test keeps the allowlist and the hint's command list in sync with `skill/scripts/command-metadata.json`.

## Consequences

- Claude Code shows the command hint.
- Strict validators report the two extra keys. That is expected; the runtimes load the skill.

## Revisit when

A harness starts rejecting unknown keys at load time.
