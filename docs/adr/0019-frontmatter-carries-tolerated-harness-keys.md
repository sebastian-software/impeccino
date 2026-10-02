# 0019: Frontmatter carries harness keys that runtimes tolerate

**Status:** Accepted · **Date:** 2026-10-02 · Supersedes [0008](0008-spec-only-skill-frontmatter.md)

## Context

0008 limited SKILL.md to the Agent Skills spec fields so Codex's bundled skill validator (`quick_validate.py`) would pass. That validator is an authoring aid from Codex's skill creator, not the runtime: Codex 0.159.2 loads a skill with `user-invocable` and `argument-hint` without a warning, and every other harness ignores keys it does not know (docs/HARNESSES.md). The strict frontmatter served a lint run and cost Claude Code its command hint.

## Decision

SKILL.md keeps the spec fields and adds the harness keys that runtimes tolerate and that carry information:

- `user-invocable: true`
- `argument-hint` listing the commands and a target, so Claude Code shows it in the slash-command picker.

`allowed-tools` stays out: Claude Code blocks skill activation in non-interactive sessions when it is set. A test keeps the allowlist and the hint's command list in sync with `skill/scripts/command-metadata.json`.

## Consequences

- Claude Code shows the command hint again.
- Strict validators (`quick_validate.py`, `skills-ref validate`) report the two extra keys. That is expected; the runtimes load the skill.

## Revisit when

A harness starts rejecting unknown keys at load time.
