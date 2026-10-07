# 0006: Agents ship as Claude Code files with a generic fallback

**Status:** Accepted · **Date:** 2026-10-01 · amended 2026-10-02

## Context

Four roles (finish reviewer, documenter, asset producer, manual-edit
applier) were compiled into Claude Markdown, Codex TOML nested in the skill,
Cursor and Copilot agent files, and generated `reference/degraded/` copies.
Their real value is a fresh context: the finish reviewer must judge without
the build thread's history. Installed definitions add tool limits, turn
ceilings, and reasoning effort on top. With only the skill folder installed,
Claude failed to spawn a missing agent, because the fallback only covered
harnesses without any subagent support.

## Decision

`skill/agents/*.md` are plain Claude Code agent files and the only agent
source. SKILL.md's "Shipped agents" section describes roles for tasks that request the corresponding handoff. Use an installed role
definition when the host exposes it and its delegation policy permits the call. If the role is missing but the host can
spawn subagents, the parent reads the matching file and passes its Markdown
body as instructions to a fresh general-purpose subagent, along with the task
inputs and no conversation history. If delegation is unavailable or not permitted, the
parent reads the same file and performs the role locally, disclosing that the
pass was not independent. Neither role runs an engine command, so neither
needs the launcher's path.

## Consequences

- Two roles remain: the finish reviewer and the documenter. The asset producer left with comps (0012), the manual-edit applier with live mode (0011).

- No generated Codex TOMLs (0002). Codex uses the fallback and loses the
  per-agent reasoning effort.
- Claude keeps tool limits and turn ceilings when a manager links
  `skill/agents/*.md` into `.claude/agents/`.
- Source and scripted harness tests cover the fallback contract. Real model workflow checks require provider credentials; skipped runs are not evidence of an independent review.
