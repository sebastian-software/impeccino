# 0005: No marketplace, editor, or npm packages

**Status:** Accepted · **Date:** 2026-10-02

## Context

The build produced a Claude Code/Grok plugin, a Cursor plugin, an OpenAI plugin, and a VS Code extension, each with its own manifest, path rewriting, version checks, and tests. Marketplaces bundle skill, agents, and hooks for users without a skill manager. For anyone with one, a plugin is a second copy of the same skill (the dual-install problem of upstream issue #523), and editor extensions serve a workflow that has moved to agent harnesses.

The npm package `impeccable` was a third channel: a Node shim (`cli/`) plus five `@impeccable/cli-<os>-<arch>` platform packages, so `npx impeccable detect` ran without the skill. It needed its own release component, version pins, a publish script, and a registry check in the release gate, for a detector the skill's launcher already runs.

## Decision

Impeccable ships no marketplace, editor, or npm packages. Removed: `plugin/`, `cursor-plugin/`, `.claude-plugin/`, `.cursor-plugin/`, `vscode/`, `cli/`, `README.npm.md`, the platform-package publish script, the `cli-v` release component, and their build, validation, and tests.

## Consequences

- The skill reaches users through Dalo or skills.sh (0003).
- The detector runs without an agent through an installed skill's launcher or a downloaded release binary (`<skill>/scripts/impeccable detect src/`).
- A skill manager that wants a native plugin can generate one from `skill/`.
- The Chrome/Firefox detector extension was out of scope here; 0013 removed it.
