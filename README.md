<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/impeccino-logo-dark.svg">
    <img src="docs/brand/impeccino-logo-light.svg" alt="Impeccino" width="440">
  </picture>
</p>

# Impeccino

Design guidance for AI coding agents. 1 skill, 22 commands, and 61 deterministic detector rules for AI-generated frontend design.

Impeccino ("the little Impeccable") is a slimmed-down derivative of [Impeccable](https://github.com/pbakaus/impeccable) by Paul Bakaus. It keeps the design guidance, the engine, and the detector, and leaves out everything that existed only to install, package, or run the skill in a browser of its own. It publishes one shared `skill/` folder; [Dalo](https://dalo.sh) or [skills.sh](https://skills.sh) puts it in place. [How Impeccino differs from Impeccable](#how-impeccino-differs-from-impeccable) and the [Light ADRs](docs/adr/README.md) explain each cut.

The public interface is the skill and its agent workflows. The native engine and its command-line interface are internal implementation details ([ADR 0005](docs/adr/0005-no-marketplace-packages.md)).

> **Quick start:** Install the skill with Dalo or skills.sh (see [Installation](#installation)), then run `/impeccino init` inside your AI coding tool.

## Why Impeccino?

Anthropic's [frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design) was the first widely-used design skill for Claude. Impeccable started from there, and Impeccino from Impeccable.

Every model trained on the same SaaS templates. Skip the guidance and you get the same handful of tells on every project: Inter for everything, purple-to-blue gradients, cards nested in cards, gray text on colored backgrounds, the rounded-square icon tile above every heading.

Impeccino adds:
- **One setup flow.** `/impeccino init` records durable product truth in `PRODUCT.md`, so later commands know the audience, purpose, operating context, constraints, voice, and evidence without confusing those facts with surface-level visual direction.
- **22 commands.** A shared design vocabulary with your AI: `polish`, `audit`, `critique`, `distill`, `animate`, `bolder`, `quieter`, and more.
- **61 deterministic detector rules** plus LLM-only critique checks. The skill and opt-in design hooks use the internal engine to check source files without model calls or API keys; layout rules measure the rendered page through agent-browser.

## What's Included

### The Skill: impeccino

The skill installs as one command:

```bash
/impeccino <command> <target>
```

Start every new project with:

```bash
/impeccino init
```

`init` inspects the project, asks only for material gaps in durable product truth, and writes `PRODUCT.md`. Visitor mode and visual direction are chosen later for each surface; incumbent or newly built visual systems are recorded separately in `DESIGN.md`.

### 22 Commands

All commands are accessed through `/impeccino`:

| Command | What it does |
|---------|--------------|
| `/impeccino craft` | Full shape-then-build flow with visual iteration |
| `/impeccino init` | One-time setup: gather durable product context, write PRODUCT.md, recommend next steps |
| `/impeccino document` | Generate root DESIGN.md from existing project code |
| `/impeccino extract` | Pull reusable components and tokens into the design system |
| `/impeccino shape` | Plan UX/UI before writing code |
| `/impeccino critique` | UX design review: hierarchy, clarity, emotional resonance |
| `/impeccino audit` | Run technical quality checks (a11y, performance, responsive) |
| `/impeccino polish` | Final pass, design system alignment, and shipping readiness |
| `/impeccino bolder` | Amplify boring designs |
| `/impeccino quieter` | Tone down overly bold designs |
| `/impeccino distill` | Strip to essence |
| `/impeccino harden` | Error handling, i18n, text overflow, edge cases |
| `/impeccino onboard` | First-run flows, empty states, activation paths |
| `/impeccino animate` | Add purposeful motion |
| `/impeccino colorize` | Introduce strategic color |
| `/impeccino typeset` | Fix font choices, hierarchy, sizing |
| `/impeccino layout` | Fix layout, spacing, visual rhythm |
| `/impeccino delight` | Add moments of joy |
| `/impeccino overdrive` | Add technically extraordinary effects |
| `/impeccino clarify` | Improve unclear UX copy |
| `/impeccino adapt` | Adapt for different devices |
| `/impeccino optimize` | Performance improvements |

Use `/impeccino pin <command>` to create standalone shortcuts (e.g., `pin audit` creates `/audit`).

#### Usage Examples

```
/impeccino audit blog           # Audit blog hub + post pages
/impeccino critique landing     # UX design review
/impeccino polish settings      # Final pass before shipping
/impeccino harden checkout      # Add error handling + edge cases
```

Or use `/impeccino` directly with a description:
```
/impeccino redo this hero section
```

### Anti-Patterns

The skill includes explicit guidance on what to avoid:

- Don't use overused fonts (Arial, Inter, system defaults)
- Don't use gray text on colored backgrounds
- Don't use pure black/gray (always tint)
- Don't wrap everything in cards or nest cards inside cards
- Don't use bounce/elastic easing (feels dated)

## What's in this repository

| Path | What it is |
| --- | --- |
| [`skill/`](skill/) | **The skill.** This is the single published skill folder. `SKILL.md` holds the setup flow, the design laws, and the command router; `reference/` has one playbook per command plus shared playbooks; `agents/` has the two shipped roles (finish reviewer and documenter) as Claude Code agent files; `scripts/` holds the launcher (`impeccino`, `impeccino.cmd`), the pinned engine version (`VERSION`), and command metadata. |
| [`crates/`](crates/) | **The engine.** A Rust workspace that builds the `impeccino` binary behind every skill command: project context, the 61-rule detector for source files and rendered pages (through agent-browser), the design hook, and `doctor`. It is one native binary with no WebAssembly build. Release binaries are published as `engine-v<version>` GitHub releases; the launcher fetches the one named in `skill/scripts/VERSION`. |
| [`tests/`](tests/) | Vitest suites, the oracle corpus that pins every engine verb's output (`tests/oracle/`), and opt-in LLM-backed behavior and workflow checks. |
| [`scripts/`](scripts/) | Tooling: `check.js` (`pnpm run check`), the release script, engine fetch and release checks, and the test runner. There is no skill build. |
| [`docs/`](docs/) | Developer documentation, the editorial style guide, harness notes, and the [Light ADRs](docs/adr/README.md). |

## How Impeccino differs from Impeccable

Impeccable compiles the skill into 19 harness-specific variants, commits those variants back into git, ships its own installer, and publishes plugin packages for several marketplaces. Comparing the variants showed that almost all differences were cosmetic (command sigil, the name of the question tool, a hardcoded scripts path), and that several components existed only to undo each other. Impeccino tests how far a single folder gets.

| Area | Impeccable | Impeccino | ADR |
| --- | --- | --- | --- |
| Skill source | `SKILL.src.md` with placeholders and provider blocks, compiled per harness | `skill/` is one universal skill; harness notes are labelled paragraphs | [0001](docs/adr/0001-one-universal-skill-folder.md) |
| Generated files | 19 harness folders and two plugin subtrees committed, synced by a workflow | Nothing generated is tracked | [0002](docs/adr/0002-no-generated-output-in-git.md) |
| Installation | `npx impeccable install / update / link / check` with a signed `universal.zip` | Dalo, or skills.sh for people without Dalo | [0003](docs/adr/0003-no-self-installer.md) |
| Updates | `context` checks impeccable.style and suggests an update | No update check; the installer of the skill owns updates | [0004](docs/adr/0004-no-update-check.md) |
| Distribution | Claude Code, Grok, Cursor, and OpenAI plugins, VS Code extension, the npm detector CLI | No marketplace, editor, or npm packages | [0005](docs/adr/0005-no-marketplace-packages.md) |
| Subagents | Compiled into Claude, Codex, Cursor, and Copilot formats plus fallback copies | Claude Code agent files in `skill/agents/`; other hosts spawn a general-purpose subagent with the same instructions | [0006](docs/adr/0006-agents-as-claude-code-files.md) |
| Hooks | Merged into project settings at install time | Opt-in per project with `/impeccino hooks on` | [0007](docs/adr/0007-hooks-are-a-project-opt-in.md) |
| Frontmatter | Claude-only keys in the Claude variant | Spec fields plus `user-invocable` and `argument-hint` for hosts that support them; see the [harness reference](docs/HARNESSES.md) | [0008](docs/adr/0008-frontmatter-carries-tolerated-harness-keys.md) |
| Engine pin | Root `ENGINE_VERSION`, copied by the build | `skill/scripts/VERSION` only | [0009](docs/adr/0009-engine-version-in-one-file.md) |
| Engine binary | Fetched by the installer or the launcher | Still fetched by the launcher, for now | [0010](docs/adr/0010-launcher-fetches-the-engine.md) |
| Browser | Live mode in the user's dev server, a local decision page, a component review page, URL scans over its own Chrome connection | No browser stack of its own: screenshots come from the agent's browser, decisions from the structured question tool | [0011](docs/adr/0011-no-own-browser-stack.md) |
| Rendered-page rules | Run in the live overlay, the extension, or URL scans over Impeccable's own Chrome connection | Rendered-page checks use agent-browser: a read-only measurement in the page, the same rules evaluated natively, screenshot pixels for the rest | [0016](docs/adr/0016-rendered-pages-through-agent-browser.md) |
| Build path | Comp-first (image-generated mock, comp fidelity tooling) or code-first, chosen by `buildPath` | Code-led build only, carried by the direction contract | [0012](docs/adr/0012-no-image-comps.md) |
| Concept roll | `concept-seed` deals visual worlds from Impeccable's hosted catalog and reports the user's choice back to it | `concept-seed` assigns one of the agent's own grounded directions on your machine; no catalog, no network, no ping | [0019](docs/adr/0019-concept-seed-is-local.md) |
| Rule engine targets | Native binary plus a WebAssembly build for the browser extension and the in-page overlay | One native binary; no WebAssembly build, no browser extension | [0013](docs/adr/0013-no-wasm-or-browser-extension.md) |
| Releases | Changelog entry in the website repository, rendered into the release notes; Windows binaries signed with the upstream maintainer's certificate | Per-component tags (`skill-v`, `engine-v`) with notes GitHub generates from the commits; each engine asset carries a GitHub build attestation and its digest is pinned in the skill | [0014](docs/adr/0014-releases-are-tags.md) |
| Docs | Finished plans, port contracts, release notes, and demos kept in `docs/` | `docs/` holds current guidance and ADRs; the oracle corpus is the behavioral contract; history lives in git | [0015](docs/adr/0015-history-lives-in-git.md) |
| Project state | A hidden `.impeccable/` folder: shared and personal config, design sidecar, surface briefs, critique archive, hook caches, screenshots | `PRODUCT.md`, `DESIGN.md`, `DESIGN.json`, `SURFACES.md` at the top level, all committed; no config file (detector decisions live in DESIGN.md and the project's ignore rules); runtime state in the user cache | [0020](docs/adr/0020-project-state-is-top-level-files.md) |

Unchanged: the design guidance itself, every command that does not need a browser or an image model (22 commands; `live` and `generate` are gone), and the engine's context, hook, and detector. The detector keeps all 61 rules: the nine that need a rendered page now run through agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)).

**Verified so far.** Skill loading and launcher invocation have been verified in Claude Code and Codex; both resolve and run the launcher, and Codex names commands with `$`. Dalo and skills.sh both find and install it. The Rust workspace tests, the core suite, the full oracle corpus, and rendered-page scans through agent-browser pass. The harness notes are a [point-in-time reference](docs/HARNESSES.md), not a record of end-to-end Impeccino verification.

**Open.** The generic-subagent fallback has not been exercised in a full build run. The LLM-backed behavior suite has not run against the labelled harness paragraphs.

## Installation

`skill/` is the whole skill in one shared form, like an app bundle you drag into place. Impeccino has no installer of its own ([ADR 0003](docs/adr/0003-no-self-installer.md)); install it with Dalo or skills.sh.

No separate engine installation is needed. The skill's launcher downloads and caches the pinned native engine on first use, verifies its pinned digest, and runs it for the agent. If the pinned engine is not already available locally, the download needs network access and a writable cache directory. Rendered-page checks also need agent-browser; see [Quality checks through the skill](#quality-checks-through-the-skill). Runtime distribution and provenance are documented for contributors in [ENGINE.md](docs/ENGINE.md).

### Dalo (recommended)

```bash
dalo source add impeccino https://github.com/sebastian-software/impeccino.git --subpath skill
dalo sync
```

[Dalo](https://dalo.sh) pins the commit, links the skill into every harness you use, owns updates and removal, and gates the design hook behind its own approval. To give Claude Code the dedicated subagents, link `skill/agents/*.md` into `.claude/agents/` as well; without them the skill runs each role in a fresh general-purpose subagent.

### skills.sh

```bash
npx skills add sebastian-software/impeccino
```

[skills.sh](https://skills.sh) finds `skill/`, installs it as `impeccino` into the harness folders you pick (`-g` for user level), and updates it with `npx skills update`.

## Usage

Once installed, every command runs through the single `/impeccino` skill:

```
/impeccino audit        # Find issues
/impeccino polish       # Final cleanup
/impeccino distill      # Remove complexity
/impeccino critique     # Full design review
```

Type `/impeccino` alone to see the full command list.

Most commands accept an optional argument to focus on a specific area:

```
/impeccino audit the header
/impeccino polish the checkout form
```

If you reach for one command often, pin it with `/impeccino pin audit` to get `/audit` as a standalone shortcut.

**Note:** Codex uses skills here, not `/prompts:` commands. Open `/skills` or type `$impeccino`. Repo-local installs live in `.agents/skills/`; user-wide installs live in `~/.agents/skills/`. GitHub Copilot uses `.github/skills/`. Restart the tool if a newly installed skill does not appear.

## What lands in your project

Nothing you need to ignore. Impeccino keeps its project state in four top-level files, all meant to be committed: `PRODUCT.md` (product truth), `DESIGN.md` (the visual system), `DESIGN.json` next to DESIGN.md (the design sidecar), and `SURFACES.md` (one section per surface: its mode and direction contract). There is no config file and no `.impeccino/` directory ([ADR 0020](docs/adr/0020-project-state-is-top-level-files.md)).

Runtime state stays out of the project: the hook's session cache and the weekly staleness throttle live in your user cache next to the engine binary, and review screenshots go to a temporary directory. A `.impeccino/` left by an older version is reported once at session start, with a note on where each file now belongs.

## Design hook

The design hook runs the Impeccino detector on direct UI file edits and surfaces findings back into the agent flow. It is a per-project opt-in ([ADR 0007](docs/adr/0007-hooks-are-a-project-opt-in.md)): run `/impeccino hooks on` in a project, and the engine writes the harness's own hook manifest; `/impeccino hooks off` and `reset` undo it. Without the hook, the skill asks for one manual detector run when a change is finished.

Hook surfaces the engine manages:

- Claude Code: `.claude/settings.local.json` (gitignored, machine-local). A hook moved into the shared `settings.json` is honored in place.
- Codex: `.codex/hooks.json`, with a `commandWindows` sibling that calls `impeccino.cmd` for cmd.exe. Approve it in `/hooks`.
- Cursor: `.cursor/hooks.json`, which blocks bad proposed writes before they land.
- GitHub Copilot: `.github/hooks/impeccino.json`, a team-shared file the Copilot CLI reads once it is committed to the default branch.

The hook also understands Grok Build's events, and `context` recognizes a Grok manifest at `.grok/hooks/impeccino.json`; `hooks on` does not write that one. Gemini CLI has no hook manifest; the skill asks for a manual detector run there.

Every hook command goes through the skill's launcher, guarded so a missing launcher is a silent no-op. Unrelated hook entries and settings are preserved. The hook is on where its entries are installed; there is nothing else to configure. `IMPECCINO_HOOK_DISABLED=1` turns an installed hook off for one shell, and `IMPECCINO_HOOK_QUIET=1` silences the clean-edit acks.

In Claude Code, command hooks run independently of model-tool approval, so the first edit or Stop event can download and cache the engine even if the session denies the model's launcher command. Review hooks before unattended runs; to disable all Claude Code hooks for a run, pass `--settings '{"disableAllHooks": true}'`.

For debugging, set `IMPECCINO_HOOK_LOG` to a path to write one NDJSON line per hook invocation. Leave it unset for normal use.

The Stop pass suppresses confirmed pre-existing findings when a verified before-edit baseline is available (currently Claude Edit/Write results for text scans). Other findings are marked new or attribution unknown; unknown is not evidence that your session caused the problem. Explicit checks requested through the skill still report findings independently of session attribution.

## Quality checks through the skill

Use `/impeccino audit <target>` to have the agent review the target. For web projects, the audit checks source files and rendered pages; native iOS and Android audits use platform-specific guidance without browser scans. Web workflows and opt-in hooks use the internal engine's 61 deterministic detector rules. Rendered web checks need [agent-browser](https://github.com/vercel-labs/agent-browser) installed (`npm install -g agent-browser && agent-browser install`); source checks do not.

The agent interprets findings alongside the design guidance and visual inspection. A clean detector run is evidence, not proof of visual or accessibility quality. Ask the agent to record justified waivers and inspect the experience across relevant viewports.

## Harnesses

- [Cursor](https://cursor.com)
- [Claude Code](https://claude.ai/code)
- [GitHub Copilot](https://github.com/features/copilot)
- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)
- [Gemini CLI](https://github.com/google-gemini/gemini-cli)
- [Codex CLI](https://github.com/openai/codex)
- [Grok Build](https://x.ai/cli)
- [Hermes Agent](https://hermes-agent.nousresearch.com)
- [OpenCode](https://opencode.ai)
- [Pi](https://pi.dev)
- [Kiro](https://kiro.dev)
- [Trae](https://trae.ai)
- [Rovo Dev](https://www.atlassian.com/software/rovo)
- [Qoder](https://qoder.com)
- [Mistral Vibe](https://docs.mistral.ai/vibe/code/overview)
- [Google Antigravity](https://antigravity.google)

## Contributing

See [DEVELOP.md](docs/DEVELOP.md) for contributor guidelines and build instructions.

## License

Apache 2.0. See [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md). Impeccino is derived from [Impeccable](https://github.com/pbakaus/impeccable), Copyright Paul Bakaus, also under Apache 2.0.
