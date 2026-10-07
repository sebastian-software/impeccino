# Developer Guide

Source format and local setup for Impeccino contributors. [AGENTS.md](../AGENTS.md) is the contributor guide and owns validation and release instructions.

## Architecture

`skill/` is the skill, and it installs as-is in every harness: there is no build, installer, or per-harness variant. The decisions behind this are recorded as Light ADRs in [adr/](adr/README.md). For harness behavior (frontmatter, subagents, hooks), see [HARNESSES.md](HARNESSES.md).

The public interface is the skill and its agent workflows. The CLI and Rust workspace are internal runtime implementation; [ENGINE.md](ENGINE.md) documents their invocation, integration contract, build, and release flow. Keep direct engine examples in contributor documentation rather than presenting a second user workflow.

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

## Local setup and validation

Install repository tooling with `pnpm install --frozen-lockfile`. Build the local engine with `cargo build --release -p impeccino`, then set `IMPECCINO_BIN="$PWD/target/release/impeccino"` when running the default suites. Follow [AGENTS.md's validation rules](../AGENTS.md#testing-guidelines) for the affected area; the [suite README](../tests/skill-behavior/README.md) describes provider-backed setup checks.

To try an edit in a harness, link `skill/` into a project as `.claude/skills/impeccino` or `.agents/skills/impeccino`. The workflow test harness uses Playwright Chromium (`npx playwright install chromium` once); the skill itself uses its host browser tools and agent-browser (ADR 0016).

## Upstream Tags

If you keep an `upstream` remote, Git may follow tags from commits fetched from it. Those tags can affect local commands such as `git describe`. Stop future automatic tag following for that remote with:

```bash
git config remote.upstream.tagOpt --no-tags
```

This affects future fetches only. A direct `git fetch --tags upstream` still fetches tags.

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
