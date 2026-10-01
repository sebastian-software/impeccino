# 0015: History lives in git

**Status:** Accepted · **Date:** 2026-10-02

## Context

`docs/` held finished plans and snapshots: release and readiness notes for 4.4.0, a plan-review contract, the live rewrite plan, the JavaScript-to-Rust porting guide and port contract (`CLI-CONTRACT.md`), comp-fidelity reviews, and a landing-page demo. They described work that is done or features that are gone, and readers could not tell current guidance from history.

## Decision

`docs/` keeps only current guidance (development, engine, harness notes, style, signing) and these ADRs. Finished plans, snapshots, and demos are removed; git history keeps them.

## Consequences

- The oracle corpus (`tests/oracle/`) is the behavioral contract of the engine's verbs.
- Website material belongs in the website repository.
