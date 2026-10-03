<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/brand/impeccino-logo-dark.svg">
    <img src="docs/brand/impeccino-logo-light.svg" alt="Impeccino" width="440">
  </picture>
</p>

# Impeccino

Design guidance for AI coding agents. 1 skill, 22 commands, and 61 deterministic detector rules for AI-generated frontend design.

Impeccino ("the little Impeccable") is a slimmed-down derivative of [Impeccable](https://github.com/pbakaus/impeccable) by Paul Bakaus. It keeps the design guidance, the engine, and the detector, and leaves out everything that existed only to install, package, or run the skill in a browser of its own. It publishes one shared `skill/` folder; [Dalo](https://dalo.sh) or [skills.sh](https://skills.sh) puts it in place. [How Impeccino differs from Impeccable](#how-impeccino-differs-from-impeccable) and the [Light ADRs](docs/adr/README.md) explain each cut.

> **Quick start:** Install the skill with Dalo or skills.sh (see [Installation](#installation)), then run `/impeccino init` inside your AI coding tool.

## Why Impeccino?

Anthropic's [frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design) was the first widely-used design skill for Claude. Impeccable started from there, and Impeccino from Impeccable.

Every model trained on the same SaaS templates. Skip the guidance and you get the same handful of tells on every project: Inter for everything, purple-to-blue gradients, cards nested in cards, gray text on colored backgrounds, the rounded-square icon tile above every heading.

Impeccino adds:
- **One setup flow.** `/impeccino init` records durable product truth in `PRODUCT.md`, so later commands know the audience, purpose, operating context, constraints, voice, and evidence without confusing those facts with surface-level visual direction.
- **22 commands.** A shared design vocabulary with your AI: `polish`, `audit`, `critique`, `distill`, `animate`, `bolder`, `quieter`, and more.
- **61 deterministic detector rules** plus LLM-only critique checks. The CLI and the design hook run them on source files with no LLM and no API key; the rules that need layout run on the rendered page with `detect <url>`, through agent-browser.

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
| [`crates/`](crates/) | **The engine.** A Rust workspace that builds the `impeccino` binary behind every skill command: project context, the 61-rule detector for source files and rendered pages (`detect <url>` through agent-browser), the design hook, and `doctor`. It is one native binary with no WebAssembly build. Release binaries are published as `engine-v<version>` GitHub releases; the launcher fetches the one named in `skill/scripts/VERSION`. |
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
| Rendered-page rules | Run in the live overlay, the extension, or URL scans over Impeccable's own Chrome connection | `detect <url>` drives agent-browser: a read-only measurement in the page, the same rules evaluated natively, screenshot pixels for the rest | [0016](docs/adr/0016-rendered-pages-through-agent-browser.md) |
| Build path | Comp-first (image-generated mock, comp fidelity tooling) or code-first, chosen by `buildPath` | Code-led build only, carried by the direction contract | [0012](docs/adr/0012-no-image-comps.md) |
| Rule engine targets | Native binary plus a WebAssembly build for the browser extension and the in-page overlay | One native binary; no WebAssembly build, no browser extension | [0013](docs/adr/0013-no-wasm-or-browser-extension.md) |
| Releases | Changelog entry in the website repository, rendered into the release notes; Windows binaries signed with the upstream maintainer's certificate | Per-component tags (`skill-v`, `engine-v`) with notes GitHub generates from the commits; each engine asset carries a GitHub build attestation and its digest is pinned in the skill | [0014](docs/adr/0014-releases-are-tags.md) |
| Docs | Finished plans, port contracts, release notes, and demos kept in `docs/` | `docs/` holds current guidance and ADRs; the oracle corpus is the behavioral contract; history lives in git | [0015](docs/adr/0015-history-lives-in-git.md) |

Unchanged: the design guidance itself, every command that does not need a browser or an image model (22 commands; `live` and `generate` are gone), and the engine's context, hook, and detector. The detector keeps all 61 rules: the nine that need a rendered page now run through `detect <url>` and agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)).

**Verified so far.** Skill loading and launcher invocation have been verified in Claude Code and Codex; both resolve and run the launcher, and Codex names commands with `$`. Dalo and skills.sh both find and install it. The Rust workspace tests, the core suite, the full oracle corpus, and rendered-page scans through agent-browser pass. The harness notes are a [point-in-time reference](docs/HARNESSES.md), not a record of end-to-end Impeccino verification.

**Open.** The generic-subagent fallback has not been exercised in a full build run. The LLM-backed behavior suite has not run against the labelled harness paragraphs. `concept-seed` still draws its visual-world catalog from Impeccable's public API (`impeccable.style/api`); see [#7](https://github.com/sebastian-software/impeccino/issues/7).

## Installation

`skill/` is the whole skill in one shared form, like an app bundle you drag into place. Impeccino has no installer of its own ([ADR 0003](docs/adr/0003-no-self-installer.md)); install it with Dalo or skills.sh.

The skill needs no runtime. Its launcher (`scripts/impeccino`, plus `impeccino.cmd` for Windows) runs the Impeccino engine, a self-contained binary that is downloaded once on first run into `~/.impeccino/bin/` from this repository's GitHub Releases, for the version in `skill/scripts/VERSION`. The download must match the digest pinned in `skill/scripts/engine.sha256`, and every release binary carries a GitHub build attestation (`gh attestation verify <file> -R sebastian-software/impeccino`). The engine release workflow also publishes `THIRD-PARTY-NOTICES.txt`; skill bundles include `LICENSE` and `NOTICE.md`. If you manage tools with [mise](https://mise.jdx.dev), `mise use github:sebastian-software/impeccino` (with `version_prefix = "engine-v"`) installs the same binary, and the launcher picks it up from your PATH. Rendered-page scans (`detect <url>`) also need [agent-browser](https://github.com/vercel-labs/agent-browser).

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

## Keeping `.impeccino` out of git

As you run commands, Impeccino writes working files under `.impeccino/`: critique and polish screenshots, hook caches, and per-developer config. Most of it is ephemeral and should not be committed, while a few files are shared project artifacts that belong in the repo. Add this block to your project's `.gitignore`:

```gitignore
# impeccino-ignore-start
# Ephemeral output, runtime state, and per-dev overrides.
# The **/ prefix covers .impeccino at the repo root or in a nested workspace.
# Shared artifacts stay tracked: config.json, design.json,
# surfaces/*.md, critique/*.md.
**/.impeccino/config.local.json
**/.impeccino/hook.cache.json
**/.impeccino/hook.pending.json
**/.impeccino/*.png
**/.impeccino/review/
**/.impeccino/questions/
# impeccino-ignore-end
```

The block is wrapped in `# impeccino-ignore-start` / `# impeccino-ignore-end` markers so you can recognize and refresh it later. The `**/` prefix makes each pattern match whether the active project's `.impeccino/` directory is at the repository root or under a nested workspace path like `apps/web/`.

**Keep these tracked** (they are shared project artifacts, do not add them to `.gitignore`):

- `.impeccino/config.json` (unified shared config)
- `.impeccino/design.json` (shared design spec)
- `.impeccino/surfaces/*.md` (route- or artifact-specific strategy and direction contracts)
- `.impeccino/critique/*.md` (review reports)

If an ephemeral file (a screenshot, `config.local.json`) was committed before you added the block, `.gitignore` will not untrack it automatically. Run `git rm --cached <path>` to stop tracking it without deleting your local copy.

## Design hook

The design hook runs the Impeccino detector on direct UI file edits and surfaces findings back into the agent flow. It is a per-project opt-in ([ADR 0007](docs/adr/0007-hooks-are-a-project-opt-in.md)): run `/impeccino hooks on` in a project, and the engine writes the harness's own hook manifest; `/impeccino hooks off` and `reset` undo it. Without the hook, the skill asks for one manual detector run when a change is finished.

Hook surfaces the engine manages:

- Claude Code: `.claude/settings.local.json` (gitignored, machine-local). A hook moved into the shared `settings.json` is honored in place.
- Codex: `.codex/hooks.json`, with a `commandWindows` sibling that calls `impeccino.cmd` for cmd.exe. Approve it in `/hooks`.
- Cursor: `.cursor/hooks.json`, which blocks bad proposed writes before they land.
- GitHub Copilot: `.github/hooks/impeccino.json`, a team-shared file the Copilot CLI reads once it is committed to the default branch.

The hook also understands Grok Build's events, and `context` recognizes a Grok manifest at `.grok/hooks/impeccino.json`; `hooks on` does not write that one. Gemini CLI has no hook manifest; the skill asks for a manual detector run there.

Every hook command goes through the skill's launcher, guarded so a missing launcher is a silent no-op. Unrelated hook entries and settings are preserved. Hook lifecycle settings live under the `hook` key of `.impeccino/config.json`; detector ignores live under `detector`, shared by `/impeccino hooks` and `impeccino detect`.

In Claude Code, command hooks run independently of model-tool approval, so the first edit or Stop event can download and cache the engine even if the session denies the model's launcher command. Review hooks before unattended runs; to disable all Claude Code hooks for a run, pass `--settings '{"disableAllHooks": true}'`.

For debugging, set `hook.auditLog` in `.impeccino/config.json` to a path (or the legacy `IMPECCINO_HOOK_LOG` env var) to write one NDJSON line per hook invocation. Leave it unset for normal use.

The Stop pass suppresses confirmed pre-existing findings when a verified before-edit baseline is available (currently Claude Edit/Write results for text scans). Other findings are marked new or attribution unknown; unknown is not evidence that your session caused the problem. Explicit `detect` scans remain unchanged.

## Detector without an agent

The detector is part of the skill's engine; there is no separate CLI package. To run it in CI or a terminal, call the launcher of an installed skill (it fetches the engine on first run), or download the binary for your platform from the `engine-v<version>` release:

```bash
.claude/skills/impeccino/scripts/impeccino detect src/          # scan a directory
.claude/skills/impeccino/scripts/impeccino detect --json .      # CI-friendly JSON output
.claude/skills/impeccino/scripts/impeccino ignores list         # show detector ignores
.claude/skills/impeccino/scripts/impeccino ignores add-file "src/legacy/**"
```

The detector catches 61 deterministic issues across AI slop (side-tab borders, purple gradients, bounce easing, dark glows) and general design quality (low contrast, cramped padding, tiny text, skipped headings, and more). `detect` reads files, directories, and URLs. For a URL (`http`, `https`, or `file`), it loads the page headlessly through [agent-browser](https://github.com/vercel-labs/agent-browser) and adds the rules that need layout (line length, text overflow and occlusion, viewport edges, heading rhythm), rendered contrast including text over images, and script errors: `impeccino detect --viewport 390x844 http://localhost:3000/`. Set `AGENT_BROWSER_SESSION` to scan in a session that is already signed in. Rendered scans need agent-browser installed (`npm install -g agent-browser && agent-browser install`); source scans do not.

Human-readable findings are diagnostics written to stderr, so redirect them with `2> findings.txt`. Use `--json` for machine-readable results on stdout. Exit `0` means the scan completed without primary findings, exit `2` means it completed with primary findings, and exit `1` means at least one requested target could not be scanned; operational failure takes precedence for a partial multi-target scan. A clean detector run is evidence, not proof of visual or accessibility quality: it does not replace inspecting the rendered experience across relevant viewports.

By default, `detect` respects the same `.impeccino/config.json` and `.impeccino/config.local.json` detector config as the design hook: `detector.ignoreRules`, `detector.ignoreFiles`, `detector.ignoreValues`, and `detector.designSystem.enabled`. Hook lifecycle settings such as `hook.enabled` only affect automatic hook execution.

For a waiver that should travel with one file instead of the repo config, add an inline comment in the file: `<!-- impeccino-disable overused-font: exported brand doc -->`. The marker works in any comment syntax, scopes to the whole file (or one line with `impeccino-disable-line` / `impeccino-disable-next-line`), and is bypassed by `--no-inline-ignores` or `--no-config`.

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
