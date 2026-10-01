# 0007: Hooks are a per-project opt-in

**Status:** Accepted · **Date:** 2026-10-01

## Context

The installer merged design-detector hooks into each harness's project
settings at install time. The hooks run the detector after every edit and a
deep pass on Stop, which only pays off in projects doing UI work, and
Impeccable already asks for consent per project.

## Decision

Hooks are not installed. A project opts in with `/impeccable hooks on`,
which the engine handles itself (it writes and removes the project's hook
manifests). Without hooks, `context` asks for one manual detector run.

## Consequences

- No hook manifests in the repository or in any package.
- A skill manager may still offer hooks, but it is not required to.
