# 0005: No marketplace, editor, or npm packages

**Status:** Accepted · **Date:** 2026-10-03

## Context

The build produced a Claude Code/Grok plugin, a Cursor plugin, an OpenAI plugin, and a VS Code extension, each with its own manifest, path rewriting, version checks, and tests. Marketplaces bundle skill, agents, and hooks for users without a skill manager. For anyone with one, a plugin is a second copy of the same skill (the dual-install problem of upstream issue #523), and editor extensions serve a workflow that has moved to agent harnesses.

The npm package `impeccable` was a third channel: a Node shim (`cli/`) plus five `@impeccable/cli-<os>-<arch>` platform packages, so `npx impeccable detect` ran without the skill. It needed its own release component, version pins, a publish script, and a registry check in the release gate, for a detector the skill's launcher already runs.

A native binary still exists behind the skill. Documenting it as a standalone terminal or CI product creates a second user interface and compatibility expectations even without an npm package.

## Decision

Impeccino ships no marketplace, editor, or npm packages. Removed: `plugin/`, `cursor-plugin/`, `.claude-plugin/`, `.cursor-plugin/`, `vscode/`, `cli/`, `README.npm.md`, the platform-package publish script, the `cli-v` release component, and their build, validation, and tests.

The public interface is the skill and its agent workflows, including opt-in hooks and skill shortcuts. The launcher, engine CLI, and Rust crates are internal implementation details. Release binaries exist to supply the pinned skill runtime; they are not a separate CLI distribution for end users. User documentation describes skill installation and workflows; engine invocation, build, test, and release details belong in contributor documentation.

## Consequences

- The skill reaches users through Dalo or skills.sh (0003).
- Engine calls remain available for agents, hooks, repository checks, and contributor debugging. Internal interface changes must update those callers and their regression coverage together; existing commands are not removed by this decision.
- Standalone terminal/CI usage and downstream Rust integration are outside the supported product surface.
- A skill manager that wants a native plugin can generate one from `skill/`.
- The Chrome/Firefox detector extension was out of scope here; 0013 removed it.
