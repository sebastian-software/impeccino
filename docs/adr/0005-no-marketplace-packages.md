# 0005: No marketplace or editor packages

**Status:** Accepted · **Date:** 2026-10-01

## Context

The build produced a Claude Code/Grok plugin, a Cursor plugin, an OpenAI
plugin, and a VS Code extension, each with its own manifest, path rewriting,
version checks, and tests. Marketplaces bundle skill, agents, and hooks for
users without a skill manager. For anyone with one, a plugin is a second copy
of the same skill (the dual-install problem of issue #523), and editor
extensions serve a workflow that has moved to agent harnesses.

## Decision

Impeccable ships no marketplace or editor packages. `plugin/`,
`cursor-plugin/`, `.claude-plugin/`, `.cursor-plugin/`, `vscode/`, and their
build and validation code are removed.

## Consequences

- One-click marketplace installs are gone; the skill reaches users through
  skill managers or a copy (0003).
- A skill manager that wants a native plugin can generate one from `skill/`.
- The Chrome/Firefox detector extension (`extension/`) is a separate product
  and is not affected by this decision.
