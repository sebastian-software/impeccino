# 0006: Agents ship as Claude Code files with a generic fallback

**Status:** Accepted · **Date:** 2026-10-01

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
source. SKILL.md's "Shipped agents" section says: use the installed agent of
that name; if the host has subagents but not this agent, spawn a fresh
general-purpose subagent with the agent file as its instructions; only a
host without subagents runs the role inline. Agents never see SKILL.md, so
the parent passes them `<scripts-path>`.

## Consequences

- No generated Codex TOMLs (0002). Codex uses the fallback and loses the
  per-agent reasoning effort.
- Claude keeps tool limits and turn ceilings when a manager links
  `skill/agents/*.md` into `.claude/agents/`.
- The fallback path is not yet exercised by an end-to-end run.
