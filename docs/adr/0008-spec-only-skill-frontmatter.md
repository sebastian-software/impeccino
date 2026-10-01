# 0008: Skill frontmatter stays within the Agent Skills spec

**Status:** Accepted · **Date:** 2026-10-01

## Context

The Claude variant carried `user-invocable`, `argument-hint`, and
`allowed-tools`. Codex's runtime loads such a skill without complaint
(verified with Codex 0.159.2), but Codex's bundled skill validator
(`quick_validate.py`) rejects unknown top-level keys (issue #701).

## Decision

SKILL.md uses only spec fields: `name`, `description`, `license`,
`compatibility`, and `metadata` (with `metadata.version`).

## Consequences

- Claude loses the `argument-hint` input hint; `user-invocable` defaults to
  true anyway.

## Revisit when

The hint matters more than a clean validator run; restoring it does not
break Codex at runtime.
