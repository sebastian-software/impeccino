# Harness Skills Capabilities Reference

Source of truth for what each AI coding harness supports in terms of agent skills.
Reference for how harnesses treat skills, subagents, and hooks. Impeccable no longer installs into these harnesses itself (see adr/0003); install-path and hook-manifest notes below describe the harnesses, not an installer.

Impeccable runs nothing in the browser ([ADR 0011](adr/0011-nothing-runs-in-the-browser.md)). Screenshots and rendered checks come from the harness's own browser tool (Claude in Chrome, Playwright MCP, the Codex Browser, and similar), and decisions go through the host's structured question tool. When a harness has no browser tool, the skill asks the user for the captures or names the fallback signal it used.

Last verified: 2026-04-28 (subagent landscape spot-checked 2026-06-28; Mistral Vibe row verified 2026-07-16; Grok Build skills row verified 2026-07-21; Grok Build hook stdin captured 2026-08-24; DeepSeek Harness row verified 2026-09-06)

> This file is point-in-time. Capabilities move fast; verify live before relying
> on any "only X supports Y" claim. Notably, the subagent table below lists
> Impeccable's *emission targets*, not the support landscape (see its note).

## Official Documentation

| Harness | Docs URL |
|---------|----------|
| Claude Code | https://code.claude.com/docs/en/skills |
| Cursor | https://cursor.com/docs/context/skills |
| DeepSeek Harness | https://github.com/deepseek-ai/deepseek-harness |
| Gemini CLI | https://geminicli.com/docs/cli/skills/ |
| Codex CLI | https://developers.openai.com/codex/skills |
| GitHub Copilot (Agents) | https://code.visualstudio.com/docs/copilot/customization/agent-skills |
| Kiro | https://kiro.dev/docs/skills/ |
| OpenCode | https://opencode.ai/docs/skills/ |
| Pi | https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/skills.md |
| Qoder | https://docs.qoder.com/extensions/skills |
| Trae | TBD (no official skills docs found yet) |
| Rovo Dev | https://support.atlassian.com/rovo/docs/extend-rovo-dev-cli-with-agent-skills |
| Mistral Vibe | https://docs.mistral.ai/vibe/code/cli/skills |
| Grok Build | https://docs.x.ai/build/features/skills-plugins-marketplaces |
| Hermes Agent | https://hermes-agent.nousresearch.com/docs/ |
| Antigravity | https://antigravity.google/docs/skills |

## Spec Compliance

All harnesses follow the [Agent Skills specification](https://agentskills.io/specification) to varying degrees. The spec defines these frontmatter fields: `name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`.

Provider-specific extensions beyond the spec: `user-invocable`, `argument-hint`, `disable-model-invocation`, `allowed-tools` (extended syntax), `model`, `effort`, `context`, `agent`, `hooks`, `subtask`, `mcp`.

## Frontmatter Support

Fields marked with * are spec-standard. Others are provider extensions.

| Field | Claude Code | Cursor | Gemini | Codex | Copilot | Grok | Hermes | Kiro | OpenCode | Pi | Qoder | Rovo Dev | Mistral Vibe | Antigravity | DSH |
|-------|:-----------:|:------:|:------:|:-----:|:-------:|:----:|:------:|:----:|:--------:|:--:|:-----:|:--------:|:------------:|:-----------:|:------:|
| `name`* | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| `description`* | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| `license`* | Yes | Yes | Ignored | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Ignored |
| `compatibility`* | Yes | Yes | Ignored | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Ignored |
| `metadata`* | Yes | Yes | Ignored | No | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| `allowed-tools`* | Yes | No | Ignored | No | No | Yes | No | No | No | Yes | Yes | Yes | Yes | Yes | No |
| `user-invocable` | Yes | No | No | No | Yes | Yes | No | No | No | No | Yes | Yes | Yes | No | Yes |
| `argument-hint` | Yes | No | No | No | Yes | Yes | No | No | No | No | Yes | Yes | No | No | No |
| `disable-model-invocation` | Yes | Yes | No | No | Yes | Yes | No | No | Yes | Yes | TBD | TBD | No | No | Yes |
| `model` | Yes | No | No | No | No | Yes | No | No | No | No | No | No | No | No | No |
| `effort` | Yes | No | No | No | No | Yes | No | No | No | No | No | No | No | No | No |
| `context` | Yes | No | No | No | No | No | No | No | No | No | No | No | No | No | No |
| `agent` | Yes | No | No | No | No | No | No | No | No | No | No | No | No | No | No |
| `hooks` | Yes | No | No | Yes | No | Yes | No | No | No | No | No | No | No | No | No |

Notes:
- Gemini CLI validates only `name` and `description`; other spec fields are parsed but ignored.
- Codex CLI uses a separate `agents/openai.yaml` sidecar for skill metadata (icons, branding, MCP tools, invocation control). Codex also auto-discovers subagents bundled inside an installed skill's `agents/` folder (TOML). Impeccable ships no TOML agents ([ADR 0006](adr/0006-agents-as-claude-code-files.md)): in Codex its roles run as general-purpose subagents with the agent file as instructions. Standalone custom agents can still live under `.codex/agents/` or `~/.codex/agents/`, but Impeccable installs nothing there.
- Codex CLI hooks ship under `[features].hooks = true` (still flagged), require `/hooks` trust ceremony per-update, and are disabled on Windows.
- Grok Build is Claude Code compatible with zero config: it also reads `.claude/skills/`, `.claude/settings.json` hooks, and Claude plugin layouts. Native paths are `.grok/skills/`, `.grok/hooks/*.json`, and `.grok/agents/`. Skill frontmatter supports `when-to-use` in addition to the fields above. Project hooks require `/hooks-trust` (or `--trust`). See https://docs.x.ai/build/features/skills-plugins-marketplaces and https://docs.x.ai/build/features/hooks.
- Hermes Agent reads the Agent Skills spec as-is. Spec-defined fields (`name`, `description`, `license`, `compatibility`, `metadata`) are parsed and stored; harness-specific extensions (`user-invocable`, `argument-hint`, `allowed-tools`, `disable-model-invocation`, `model`, `effort`, `context`, `agent`, `hooks`) are unknown keys and silently ignored. Hermes has no hook surface, no per-skill tool ACL, and no slash-command equivalent of `user-invocable` (skills are loaded via `/skill <name>` or auto-loaded; sub-commands like `/impeccable polish` are routed from the skill body, not declared in frontmatter). Hermes adds two frontmatter fields not in the spec: `platforms:` (OS filter; default = all) and `environments:` (relevance gate over `kanban`, `docker`, `s6`). Unknown fields are silently ignored.
- Kiro recognizes `user-invocable` and `disable-model-invocation` per community reports but does not formally document them.
- Antigravity supports standard Agent Skills spec frontmatter fields (`name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`).
- OpenCode 1.18.10 recognises only the spec subset on SKILL.md (`name`, `description`, `license`, `compatibility`, `metadata`). Claude-style extensions (`user-invocable`, `argument-hint`, `allowed-tools`, `model`, `agent`) are silently ignored; Impeccable still emits them today for other harnesses, but they have no effect in OpenCode. Use `commands/<name>.md` (see Placeholder / Variable Substitution below) for slash UX; OpenCode honours only `description`, `agent`, `model`, `variant`, `subtask` on command files.
- DeepSeek Harness parses the Agent Skills frontmatter and requires `name` and `description`; it reads `metadata`, `user-invocable`, and `disable-model-invocation`. Spec fields it does not consume (`license`, `compatibility`, `allowed-tools`) and Claude-style extensions (`argument-hint`, `model`, `effort`, `context`, `agent`, `hooks`) are silently ignored. Hooks are in-process plugins configured via cordis.yml, not on-disk manifests, so there is no hook surface to install. Subagents exist but are composed from preset config, not an on-disk skill-adjacent format. Verified against the [filesystem skill provider](https://github.com/deepseek-ai/deepseek-harness/blob/master/packages/skill/skill-filesystem/README.md).
- Unknown fields are silently ignored by all harnesses.

## Hook surface used by Impeccable

Hooks are a per-project opt-in ([ADR 0007](adr/0007-hooks-are-a-project-opt-in.md)): `/impeccable hooks on` makes the engine write the manifest for each harness whose skill folder it finds. Every hook command runs the skill's launcher (`scripts/impeccable hook` or `hook-before-edit`).

| Harness | Edit hook | Stop pass | Manifest location | Notes |
|---------|:---------:|:---------:|-------------------|-------|
| Claude Code | Yes (`PostToolUse`) | Yes | `.claude/settings.local.json` | Machine-local and gitignored; a hook moved into the shared `.claude/settings.json` is honored in place. |
| Codex CLI | Yes (`PostToolUse`) | Yes | `.codex/hooks.json` | Runs `.agents/skills/impeccable/scripts/impeccable` from the git root, with a `commandWindows` sibling for cmd.exe. Requires normal `/hooks` trust approval. |
| Cursor | Yes (`preToolUse`) | No | `.cursor/hooks.json` | Runs `hook-before-edit` to block bad proposed writes before they land. Reloads on save; restart Cursor if hooks do not pick up. |
| GitHub Copilot | Yes (post-tool-use) | No | `.github/hooks/impeccable.json` | Team-shared committed file; the Copilot CLI loads it once it is on the default branch, and the cloud agent reads it from the repo. |
| Grok Build | Yes (`PostToolUse`) | Yes | `.grok/hooks/impeccable.json` | `hooks on` does not write this file; `context` recognizes it when present. Claude-compatible matchers (`Edit\|Write\|MultiEdit`) alias to Grok `search_replace`. PostToolUse runs the scan and warms the session cache; Grok ignores that stdout. Stop `additionalContext` is the user-visible pass. Ignore Grok's observe-only Stop with `reason: "shutdown"`. Requires `/hooks-trust` or `--trust`. |
| Gemini CLI | No | No | n/a | No hook manifest anymore; it existed only for the build-completion hook, which left with image comps ([ADR 0012](adr/0012-no-image-comps.md)). The skill asks for a manual detector run. |
| All other harnesses | No | No | n/a | No documented hook surface today. The skill still works and asks for a manual detector run. |

## Skill Directory Structure

| Harness | Native directory | Also reads |
|---------|-----------------|------------|
| Claude Code | `.claude/skills/` | - |
| Cursor | `.cursor/skills/` | `.agents/skills/`, `.claude/skills/` |
| DeepSeek Harness | `.dsh/skills/` (project), `~/.dsh/skills/` (global; `$DSH_HOME/skills` when set) | `.agents/skills/` (project), `~/.agents/skills/` (global) |
| Gemini CLI | `.gemini/skills/` | `.agents/skills/` |
| Codex CLI | `.agents/skills/` (primary) | - |
| GitHub Copilot | `.github/skills/` | `.agents/skills/`, `.claude/skills/` |
| Kiro | `.kiro/skills/` | - |
| OpenCode | `.opencode/skills/` | `.agents/skills/`, `.claude/skills/` |
| Pi | `.pi/skills/` (project), `~/.pi/agent/skills/` (global) | `.agents/skills/` |
| Qoder | `.qoder/skills/` | `~/.qoder/skills/` (user-level) |
| Trae China | `.trae-cn/skills/` | TBD |
| Trae International | `.trae/skills/` | TBD |
| Rovo Dev | `.rovodev/skills/` | `~/.rovodev/skills/` (user-level) |
| Mistral Vibe | `.vibe/skills/` (project), `~/.vibe/skills/` (global) | `.agents/skills/` (project), `~/.agents/skills/` (global) |
| Grok Build | `.grok/skills/` (project), `~/.grok/skills/` (global) | `.agents/skills/`, `.claude/skills/`, `.cursor/skills/` (Claude/Cursor compat, configurable) |
| Hermes Agent | `.hermes/skills/` (project), `~/.hermes/skills/` (global) | `skills.external_dirs` config (no automatic `.agents/skills/` fallback) |
| Antigravity | `.agent/skills/` (project), `~/.gemini/config/skills/` (global) | `.agents/skills/` (project), `~/.agents/skills/` (global) |

All harnesses support the `{skill-name}/SKILL.md` directory structure with optional `reference/`, `scripts/`, and `assets/` subdirectories.

## Native Subagent Directory Structure

> **Scope:** this table lists harnesses with a documented on-disk subagent
> format, not every harness that supports subagents. Cursor (auto-delegation +
> `/name` invocation, https://cursor.com/docs/subagents), GitHub Copilot, and
> Google Antigravity ship subagents too.

| Harness | Native directory | File format |
|---------|------------------|-------------|
| Claude Code | `.claude/agents/` | Markdown with YAML frontmatter |
| Grok Build | `.grok/agents/` (project) | Markdown with YAML frontmatter (Claude-compatible) |
| Codex CLI | `<skill>/agents/` (nested, auto-discovered) | TOML |

Impeccable ships two agents, `impeccable-finish-reviewer` and `impeccable-documenter`, as plain Claude Code agent files in `skill/agents/` and emits no provider-native copies ([ADR 0006](adr/0006-agents-as-claude-code-files.md)). Claude Code uses them when they are linked into `.claude/agents/`; Grok reads the same markdown from `.grok/agents/`. Any other host with subagents spawns a general-purpose subagent with the agent file as its instructions.

**Spawn / permission model** (matters more than directory support when building skills):

| Harness | Who can spawn a subagent |
|---------|--------------------------|
| Claude Code | Programmatically, from within the skill/agent flow. |
| Grok Build | Programmatically via `spawn_subagent` (built-in types plus project/user agents under `.grok/agents/`). |
| Codex CLI | Only if the user has allowed sub-agents / parallel work; otherwise the skill must ask once, then stop (see the harness-specific gate in `skill/reference/critique.md`). |
| Cursor | Agent-chosen: auto-delegated by the Agent, or user-invoked via `/name`. Not reliably skill-spawnable. |
| Others | Varies; treat as unavailable unless verified, and degrade loudly. |

## Placeholder / Variable Substitution

Claude Code supports runtime variable substitution directly in SKILL.md bodies: `$ARGUMENTS`, `$0`-`$N`, `${CLAUDE_SKILL_DIR}`, `${CLAUDE_SESSION_ID}`. No other harness supports substitution in skills.

Some harnesses have separate "custom commands" systems (distinct from skills) with their own substitution:

| Harness | Command system | Substitution syntax |
|---------|---------------|-------------------|
| OpenCode | `.opencode/commands/` (Markdown) | `$ARGUMENTS`, `$1`-`$N`, `` !`shell` ``, `@file` |
| Gemini CLI | `.gemini/commands/` (TOML) | `{{args}}`, `!{shell}`, `@{file}` |
| Codex CLI | `.codex/prompts/` | `$ARGNAME` |

Impeccable uses none of these: `skill/` has no build and no placeholders, and harness differences are labelled paragraphs ([ADR 0001](adr/0001-one-universal-skill-folder.md)).
