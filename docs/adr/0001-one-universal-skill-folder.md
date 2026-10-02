# 0001: One universal skill folder

**Status:** Accepted · **Date:** 2026-10-01

## Context

The skill source used build-time placeholders (`{{scripts_path}}`,
`{{command_prefix}}`, `{{ask_instruction}}`, `{{config_file}}`,
`{{available_commands}}`) and provider blocks (`<codex>`, `<gemini>`,
`<claude>`). A build compiled 19 variants, one per harness folder. Measured
against Claude Code and Codex, almost every difference was cosmetic: `/` vs
`$` as the command sigil, the name of the question tool, and a hardcoded
scripts path that the `<skill-base-dir>` rule in SKILL.md had already made
redundant. Three separate path rewriters (Claude plugin, Cursor plugin,
VS Code) existed only to undo `{{scripts_path}}` again, and the hardcoded
project path made a project install silently run another copy's launcher
(issue #523). The skill-behavior evals already rendered a neutral variant.

## Decision

`skill/` is the skill, in one form, for every harness:

- The launcher is `"<skill-base-dir>/scripts/impeccino" <verb>`, quoted
  because install paths can contain spaces.
- Commands are written `/impeccino <command>`; one SKILL.md line tells hosts
  with another sigil (Codex: `$impeccino`) to translate.
- Questions go through "the host's structured question tool".
- Harness- and model-specific guidance is a labelled paragraph
  (`In Codex: ...`, `**GPT models (Codex):**`) instead of a stripped block.

## Consequences

- No compile step; editing `skill/` is the release candidate.
- Every model now reads the labelled guidance meant for other harnesses and
  models (about 20 lines). The evals should confirm this does not shift
  behavior.
- Verified by linking `skill/` into a scratch project: Claude Code and Codex
  both resolve and run the launcher, and Codex names commands with `$`.

## Revisit when

A harness needs content that cannot be expressed as a labelled paragraph.
