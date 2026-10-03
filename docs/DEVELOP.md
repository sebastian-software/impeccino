# Developer Guide

Documentation for contributors to Impeccino.

## Architecture

`skill/` is the skill, and it installs as-is in every harness: there is no build, installer, or per-harness variant. The decisions behind this are recorded as Light ADRs in [adr/](adr/README.md). For harness behavior (frontmatter, subagents, hooks), see [HARNESSES.md](HARNESSES.md).

## Source Format

### Skill (`skill/SKILL.md`)

```yaml
---
name: impeccino
description: What this skill provides
license: Apache-2.0
metadata:
  version: 0.1.0
---

Your skill instructions here...
```

Frontmatter uses the [Agent Skills spec](https://agentskills.io/specification) fields plus the harness keys runtimes tolerate (ADR 0008):

- `name` (required): skill identifier (1-64 chars, lowercase, numbers, hyphens)
- `description` (required): what the skill provides (1-1024 chars)
- `license`, `compatibility` (optional); `compatibility` names what the machine needs
- `metadata` (optional): `metadata.version` carries the skill version
- `user-invocable`, `argument-hint`: provider extensions supported by some hosts; see [HARNESSES.md](HARNESSES.md) for current support

The body is the same for every harness (ADR 0001). Write the launcher as `"<skill-base-dir>/scripts/impeccino" <verb>`, commands as `/impeccino <command>`, questions as "the host's structured question tool", and harness- or model-specific guidance as a labelled paragraph (`In Codex: ...`).

## Checks

```bash
pnpm run check         # Count claims, skill frontmatter limits, prose gates
pnpm run fetch:engine  # Pinned engine binary for this machine into skill/scripts/bin/
```

To try an edit in a harness, link `skill/` into a project as `.claude/skills/impeccino` or `.agents/skills/impeccino`.

## Testing

```bash
pnpm run test                  # Default suites: core + oracle (no API keys needed; the oracle skips without an engine binary)
pnpm run test:skill-behavior   # Opt-in: LLM-backed checks that the SKILL.md Setup flow actually drives the agent (~5 min, costs cents, needs `.env`)
pnpm run test:skill-workflow   # Opt-in: provider-backed completed workflows; the test harness needs `npx playwright install chromium` once
```

The skill-behavior suite runs the models in `DEFAULT_MODELS` (`tests/skill-behavior/providers.mjs`) with the source `skill/SKILL.md` inlined as the system prompt and a workspace-scoped `bash`/`read`/`write`/`list` tool set. It then asserts on the tool-call trace, not on free-form output. Use it whenever you edit `skill/SKILL.md`'s Setup section or any Setup-touching reference (`init.md`, `document.md`, `new-work.md`, sub-command refs). Per-scenario assertions and the current baseline live in `tests/skill-behavior/README.md`. Provider keys live in repo-root `.env` (gitignored); missing keys skip cleanly.

Impeccino itself runs nothing in a browser ([ADR 0011](adr/0011-no-own-browser-stack.md)). The skill-workflow suite uses Playwright Chromium only inside its test harness, standing in for the browser tool a real harness provides.

## Skill authoring

Follow [AGENTS.md](../AGENTS.md) for cross-harness writing rules, command conventions, and change-specific validation.

## Reference Documentation

- [Agent Skills Specification](https://agentskills.io/specification) - Open standard
- [HARNESSES.md](HARNESSES.md) - Provider capabilities matrix
- [Cursor Skills](https://cursor.com/docs/context/skills)
- [Claude Code Skills](https://code.claude.com/docs/en/skills)
- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)
- [Gemini CLI Skills](https://geminicli.com/docs/cli/skills/)
- [Codex CLI Skills](https://developers.openai.com/codex/skills/)
- [VS Code Copilot Skills](https://code.visualstudio.com/docs/copilot/customization/agent-skills)
- [Kiro Skills](https://kiro.dev/docs/skills/)
- [OpenCode Skills](https://opencode.ai/docs/skills/)
- [Pi Skills](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md)
- [Qoder Skills](https://docs.qoder.com/extensions/skills)
- [Mistral Vibe Skills](https://docs.mistral.ai/vibe/code/cli/skills)
- [Grok Build Skills, Plugins & Marketplaces](https://docs.x.ai/build/features/skills-plugins-marketplaces)
- [Grok Build Hooks](https://docs.x.ai/build/features/hooks)

## Repository Structure

```
impeccino/
  skill/                           # The skill; installs as-is
    SKILL.md                       # Frontmatter, shared design laws, command router
    reference/                     # One <command>.md per command + shared playbooks
    scripts/                       # Launcher, engine VERSION pin, command metadata
    agents/                        # Claude Code agent files for the shipped roles
  crates/                          # Engine binary (Rust workspace); see docs/ENGINE.md
  scripts/
    check.js                       # Repository checks (`pnpm run check`)
    release.mjs                    # Per-component release tags (engine, skill)
  tests/                           # Vitest suites; tests/oracle/ is the engine's behavioral contract
  docs/
    adr/                           # Light ADRs
    ENGINE.md                      # Crate map and engine build
    HARNESSES.md                   # Harness capabilities reference
    RUNTIME-ENV.md                 # Environment the engine reads
    STYLE.md                       # Editorial style guide
    DEVELOP.md                     # This file
  README.md                        # User documentation
```

## Troubleshooting

### A harness does not pick up the skill
- Check that the folder is named `impeccino` inside the harness's skills directory.
- Some harnesses gate project skills behind a trust step; see [HARNESSES.md](HARNESSES.md).
- Run `pnpm exec vitest run tests/skill-source.test.js` to confirm the frontmatter and portability rules.

## Questions?

Open an issue before larger changes; small fixes can go straight to a PR.
