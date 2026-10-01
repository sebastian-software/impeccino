# Impeccable

Design guidance for AI coding agents. 1 skill, 24 commands, live browser iteration, and 61 deterministic detector rules for AI-generated frontend design.

> **About this branch.** This is `light`, an experimental fork of [pbakaus/impeccable](https://github.com/pbakaus/impeccable) by [Sebastian Werner](https://github.com/swernerx). It keeps the skill, the engine, and the detector, and removes everything that existed only to install and package the skill: per-harness variants, the self-installer, the update check, and the marketplace packages. Skill managers such as [Dalo](https://dalo.sh) take over placement, pinning, and updates. Read [What changed in this branch](#what-changed-in-this-branch) and the [Light ADRs](docs/adr/README.md) for the reasoning. It is not an official release.

> **Quick start:** Add the `skill/` folder to your harness with a skill manager such as [Dalo](https://dalo.sh), or copy it in (see [Installation](#installation)), then run `/impeccable init` inside your AI coding tool. Full docs: [impeccable.style](https://impeccable.style).

## Why Impeccable?

Anthropic's [frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design) was the first widely-used design skill for Claude. Impeccable started from there.

Every model trained on the same SaaS templates. Skip the guidance and you get the same handful of tells on every project: Inter for everything, purple-to-blue gradients, cards nested in cards, gray text on colored backgrounds, the rounded-square icon tile above every heading.

Impeccable adds:
- **One setup flow.** `/impeccable init` records durable product truth in `PRODUCT.md`, so later commands know the audience, purpose, operating context, constraints, voice, and evidence without confusing those facts with surface-level visual direction.
- **24 commands.** A shared design vocabulary with your AI: `polish`, `audit`, `critique`, `distill`, `animate`, `bolder`, `quieter`, and more.
- **61 deterministic detector rules** plus LLM-only critique checks. The CLI and browser extension run the deterministic rules with no LLM and no API key.

## What's Included

### The Skill: impeccable

The skill installs as one command:

```bash
/impeccable <command> <target>
```

Start every new project with:

```bash
/impeccable init
```

`init` inspects the project, asks only for material gaps in durable product truth, and writes `PRODUCT.md`. Visitor mode and visual direction are chosen later for each surface; incumbent or newly built visual systems are recorded separately in `DESIGN.md`.

### 24 Commands

All commands are accessed through `/impeccable`:

| Command | What it does |
|---------|--------------|
| `/impeccable craft` | Full shape-then-build flow with visual iteration |
| `/impeccable init` | One-time setup: gather durable product context, write PRODUCT.md, configure live mode when applicable, recommend next steps |
| `/impeccable document` | Generate root DESIGN.md from existing project code |
| `/impeccable extract` | Pull reusable components and tokens into the design system |
| `/impeccable shape` | Plan UX/UI before writing code |
| `/impeccable critique` | UX design review: hierarchy, clarity, emotional resonance |
| `/impeccable audit` | Run technical quality checks (a11y, performance, responsive) |
| `/impeccable polish` | Final pass, design system alignment, and shipping readiness |
| `/impeccable bolder` | Amplify boring designs |
| `/impeccable quieter` | Tone down overly bold designs |
| `/impeccable distill` | Strip to essence |
| `/impeccable harden` | Error handling, i18n, text overflow, edge cases |
| `/impeccable onboard` | First-run flows, empty states, activation paths |
| `/impeccable animate` | Add purposeful motion |
| `/impeccable colorize` | Introduce strategic color |
| `/impeccable typeset` | Fix font choices, hierarchy, sizing |
| `/impeccable layout` | Fix layout, spacing, visual rhythm |
| `/impeccable delight` | Add moments of joy |
| `/impeccable overdrive` | Add technically extraordinary effects |
| `/impeccable clarify` | Improve unclear UX copy |
| `/impeccable adapt` | Adapt for different devices |
| `/impeccable optimize` | Performance improvements |
| `/impeccable live` | Visual variant mode: iterate on elements in the browser |
| `/impeccable generate` | Generate variants of a named element in the live browser, no manual picking |

Use `/impeccable pin <command>` to create standalone shortcuts (e.g., `pin audit` creates `/audit`).

#### Usage Examples

```
/impeccable audit blog           # Audit blog hub + post pages
/impeccable critique landing     # UX design review
/impeccable polish settings      # Final pass before shipping
/impeccable harden checkout      # Add error handling + edge cases
```

Or use `/impeccable` directly with a description:
```
/impeccable redo this hero section
```

### Anti-Patterns

The skill includes explicit guidance on what to avoid:

- Don't use overused fonts (Arial, Inter, system defaults)
- Don't use gray text on colored backgrounds
- Don't use pure black/gray (always tint)
- Don't wrap everything in cards or nest cards inside cards
- Don't use bounce/elastic easing (feels dated)

## See It In Action

Visit [the Neo Mirai case study](https://impeccable.style/cases/neo-mirai) to see a before/after case study of a real project transformed with Impeccable commands.

## What's in this repository

| Path | What it is |
| --- | --- |
| [`skill/`](skill/) | **The skill.** This folder is what you install, unchanged, in every harness. `SKILL.md` holds the setup flow, the design laws, and the command router; `reference/` has one playbook per command plus shared playbooks; `agents/` has the four shipped roles as Claude Code agent files; `scripts/` holds the launcher (`impeccable`, `impeccable.cmd`), the pinned engine version (`VERSION`), command metadata, and the live-mode page scripts. |
| [`crates/`](crates/) | **The engine.** A Rust workspace that builds the `impeccable` binary behind every skill command: project context, the 61-rule detector, live mode, design hooks, comp tooling, and `doctor`. `crates/wasm` compiles the same rules for the browser extension. Release binaries are published as `engine-v<version>` GitHub releases; the launcher fetches the one named in `skill/scripts/VERSION`. |
| [`cli/`](cli/) | **The npm package** `impeccable`: a small shim that runs the engine for `npx impeccable detect` without an AI harness. It does not install the skill. |
| [`extension/`](extension/) | **The browser extension** (Chrome and Firefox) that runs the detector on any page. |
| [`browser-bundle/`](browser-bundle/), [`ui/`](ui/) | Page-side code the engine bundles: the live-mode overlay and the component review UI. |
| [`tests/`](tests/) | Bun and Node suites, the oracle corpus that pins every engine verb's output (`tests/oracle/`), framework fixtures for live mode, and opt-in LLM-backed behavior checks. |
| [`scripts/`](scripts/) | Tooling: `check.js` (`bun run check`), the release script, engine fetch and release checks, and the test runner. There is no skill build. |
| [`docs/`](docs/) | Developer documentation, the editorial style guide, harness notes, and the [Light ADRs](docs/adr/README.md). |

The website [impeccable.style](https://impeccable.style) lives in a separate private repository and is not part of this one.

## What changed in this branch

The upstream repository compiles the skill into 19 harness-specific variants, commits those variants back into git, ships its own installer, and publishes plugin packages for several marketplaces. Comparing the variants showed that almost all differences were cosmetic (command sigil, the name of the question tool, a hardcoded scripts path), and that several components existed only to undo each other. This branch tests how far a single folder gets.

| Area | Upstream | This branch | ADR |
| --- | --- | --- | --- |
| Skill source | `SKILL.src.md` with placeholders and provider blocks, compiled per harness | `skill/` is one universal skill; harness notes are labelled paragraphs | [0001](docs/adr/0001-one-universal-skill-folder.md) |
| Generated files | 19 harness folders and two plugin subtrees committed, synced by a workflow | Nothing generated is tracked | [0002](docs/adr/0002-no-generated-output-in-git.md) |
| Installation | `npx impeccable install / update / link / check` with a signed `universal.zip` | A skill manager, a submodule, or a copy places `skill/` | [0003](docs/adr/0003-no-self-installer.md) |
| Updates | `context` checks impeccable.style and suggests an update | No update check; the installer of the skill owns updates | [0004](docs/adr/0004-no-update-check.md) |
| Distribution | Claude Code, Grok, Cursor, and OpenAI plugins, VS Code extension | No marketplace or editor packages | [0005](docs/adr/0005-no-marketplace-packages.md) |
| Subagents | Compiled into Claude, Codex, Cursor, and Copilot formats plus fallback copies | Claude Code agent files in `skill/agents/`; other hosts spawn a general-purpose subagent with the same instructions | [0006](docs/adr/0006-agents-as-claude-code-files.md) |
| Hooks | Merged into project settings at install time | Opt-in per project with `/impeccable hooks on` | [0007](docs/adr/0007-hooks-are-a-project-opt-in.md) |
| Frontmatter | Claude-only keys in the Claude variant | Agent Skills spec fields only | [0008](docs/adr/0008-spec-only-skill-frontmatter.md) |
| Engine pin | Root `ENGINE_VERSION`, copied by the build | `skill/scripts/VERSION` only | [0009](docs/adr/0009-engine-version-in-one-file.md) |
| Engine binary | Fetched by the installer or the launcher | Still fetched by the launcher, for now | [0010](docs/adr/0010-launcher-fetches-the-engine.md) |

Unchanged: the design guidance itself, all 24 commands, the engine and its detector rules, live mode, the npm detector CLI, and the browser extension.

**Verified so far.** `skill/` linked into a scratch project loads in Claude Code and in Codex: both resolve and run the launcher, and Codex names commands with `$`. The Rust workspace tests, the core and live suites, and the full oracle corpus against an engine built from this branch pass.

**Open.** The generic-subagent fallback has not been exercised in a full build run. The LLM-backed behavior suite has not run against the labelled harness paragraphs. Apache-2.0 notices are not yet inside `skill/`. The website still links the removed installer and downloads.

## Installation

`skill/` is the whole skill, in one form for every harness, like an app bundle you drag into place. Impeccable has no installer of its own ([ADR 0003](docs/adr/0003-no-self-installer.md)): a skill manager, a submodule, or a plain copy puts the folder into your harness's skills directory as `impeccable`.

The skill needs no runtime. Its launcher (`scripts/impeccable`, plus `impeccable.cmd` for Windows) runs the Impeccable engine, a self-contained binary that is downloaded once on first run into `~/.impeccable/bin/` for the version pinned in `skill/scripts/VERSION`.

### Option 1: Skill manager (recommended)

Point your skill manager at this repository and the `skill/` folder. With [Dalo](https://dalo.sh):

```bash
dalo source add-catalog impeccable https://github.com/pbakaus/impeccable.git
dalo source select impeccable impeccable
dalo sync
```

The manager pins the commit, links the folder into every harness you use, and owns updates and removal. To give Claude Code the dedicated subagents, link `skill/agents/*.md` into `.claude/agents/` as well; without them the skill runs each role in a fresh general-purpose subagent.

### Option 2: Git submodule

```bash
git submodule add https://github.com/pbakaus/impeccable .impeccable
mkdir -p .claude/skills .agents/skills
ln -s ../../.impeccable/skill .claude/skills/impeccable   # Claude Code
ln -s ../../.impeccable/skill .agents/skills/impeccable   # Codex
```

Update with `git submodule update --remote .impeccable`.

### Option 3: Copy

```bash
cp -r skill your-project/.claude/skills/impeccable   # Claude Code
cp -r skill your-project/.agents/skills/impeccable   # Codex
```

Use your harness's skills folder: `.claude/skills/` (Claude Code), `.agents/skills/` (Codex), `.cursor/skills/` (Cursor), `.github/skills/` (GitHub Copilot), `.gemini/skills/` (Gemini CLI), `.opencode/skills/` (OpenCode), `.grok/skills/` (Grok Build), or the user-level equivalent under `~`. Some harnesses gate project skills behind a trust step (for example `hermes skills trust`).

## Usage

Once installed, every command runs through the single `/impeccable` skill:

```
/impeccable audit        # Find issues
/impeccable polish       # Final cleanup
/impeccable distill      # Remove complexity
/impeccable critique     # Full design review
```

Type `/impeccable` alone to see the full command list.

Most commands accept an optional argument to focus on a specific area:

```
/impeccable audit the header
/impeccable polish the checkout form
```

If you reach for one command often, pin it with `/impeccable pin audit` to get `/audit` as a standalone shortcut.

**Note:** Codex uses skills here, not `/prompts:` commands. Open `/skills` or type `$impeccable`. Repo-local installs live in `.agents/skills/`; user-wide installs live in `~/.agents/skills/`. GitHub Copilot uses `.github/skills/`. Restart the tool if a newly installed skill does not appear.

## Keeping `.impeccable` out of git

As you run commands, Impeccable writes working files under `.impeccable/`: critique and polish screenshots, live-mode session and preview state, runtime caches, and per-developer config. Most of it is ephemeral and should not be committed, while a few files are shared project artifacts that belong in the repo. Add this block to your project's `.gitignore`:

```gitignore
# impeccable-ignore-start
# Ephemeral output, runtime state, and per-dev overrides.
# The **/ prefix covers .impeccable at the repo root or in a nested workspace.
# Shared artifacts stay tracked: config.json, live/config.json,
# design.json, surfaces/*.md, critique/*.md.
**/.impeccable/config.local.json
**/.impeccable/hook.cache.json
**/.impeccable/hook.pending.json
**/.impeccable/*.png
**/.impeccable/review/
**/.impeccable/questions/
**/.impeccable/live/server.json
**/.impeccable/live/sessions/
**/.impeccable/live/previews/
**/.impeccable/live/annotations/
**/.impeccable/live/cache/
**/.impeccable/live/manual-edit-apply-transaction.json
**/.impeccable/live/manual-edit-events.jsonl
**/.impeccable/live/manual-edit-evidence/
**/.impeccable/live/pending-manual-edits.json
**/.impeccable/live/deferred-svelte-component-accepts.json
**/.impeccable/live/*.png
# impeccable-ignore-end
```

The block is wrapped in `# impeccable-ignore-start` / `# impeccable-ignore-end` markers so you can recognize and refresh it later. The `**/` prefix makes each pattern match whether the active project's `.impeccable/` directory is at the repository root or under a nested workspace path like `apps/web/`.

**Keep these tracked** (they are shared project artifacts, do not add them to `.gitignore`):

- `.impeccable/config.json` (unified shared config)
- `.impeccable/live/config.json` (live-mode framework wiring)
- `.impeccable/design.json` (shared design spec)
- `.impeccable/surfaces/*.md` (route- or artifact-specific strategy and direction contracts)
- `.impeccable/critique/*.md` (review reports)

If an ephemeral file (a screenshot, `config.local.json`) was committed before you added the block, `.gitignore` will not untrack it automatically. Run `git rm --cached <path>` to stop tracking it without deleting your local copy.

## Design hook

The design hook runs the Impeccable detector on direct UI file edits and surfaces findings back into the agent flow. It is a per-project opt-in ([ADR 0007](docs/adr/0007-hooks-are-a-project-opt-in.md)): run `/impeccable hooks on` in a project, and the engine writes the harness's own hook manifest; `/impeccable hooks off` and `reset` undo it. Without the hook, the skill asks for one manual detector run when a change is finished.

Hook surfaces the engine manages:

- Claude Code: `.claude/settings.local.json` (gitignored, machine-local). A hook moved into the shared `settings.json` is honored in place.
- Codex: `.codex/hooks.json`, with a `commandWindows` sibling that calls `impeccable.cmd` for cmd.exe. Approve it in `/hooks`.
- Cursor: `.cursor/hooks.json`, which blocks bad proposed writes before they land.
- Gemini CLI: `.gemini/settings.json`.

Every hook command goes through the skill's launcher, guarded so a missing launcher is a silent no-op. Unrelated hook entries and settings are preserved. Hook lifecycle settings live under the `hook` key of `.impeccable/config.json`; detector ignores live under `detector`, shared by `/impeccable hooks` and `npx impeccable detect`.

In Claude Code, command hooks run independently of model-tool approval, so the first edit or Stop event can download and cache the engine even if the session denies the model's launcher command. Review hooks before unattended runs; to disable all Claude Code hooks for a run, pass `--settings '{"disableAllHooks": true}'`.

For debugging, set `hook.auditLog` in `.impeccable/config.json` to a path (or the legacy `IMPECCABLE_HOOK_LOG` env var) to write one NDJSON line per hook invocation. Leave it unset for normal use.

## Build path: comp-first or code-first

When a new surface gets designed, Impeccable either generates a full-fidelity comp first and builds to match it, or builds straight in code with the ambition written into a development-only direction contract in the surface brief and checked at the finish. Comp-first composes bolder and takes longer; code-first is leaner and faster. `/impeccable init` asks once and records the answer as `buildPath` in `.impeccable/config.json`:

```json
{ "buildPath": "comp" }
```

The values are `comp` and `code`, and nothing else is read. Set it in the gitignored `.impeccable/config.local.json` to override the team's committed value on one machine, which is what you want when your harness has no image generation. In a monorepo, commit it once at the repo root and any workspace that wants something else sets its own. The choice appears at all only where image generation is available, since without it there is nothing to comp.

You do not have to re-run `init` to set it on a project that predates the setting, and you do not have to edit the file by hand either. Whatever is recorded is a default rather than a lock: every decision page carries a footer toggle, and flipping it binds that session only. Flip it on a project that has recorded nothing and Impeccable asks once, after the round, whether to keep it, then writes your answer. That is the whole migration path for an existing project: use the toggle when the default is wrong, and answer the question that follows.

Full hook docs: [impeccable.style/docs/hooks](https://impeccable.style/docs/hooks).

The Stop pass suppresses confirmed pre-existing findings when a verified before-edit baseline is available (currently Claude Edit/Write results for text scans). Other findings are marked new or attribution unknown; unknown is not evidence that your session caused the problem. Explicit `detect` scans remain unchanged.

## Live mode and production sites

Live mode edits a local checkout through a development server or local static HTML. Injecting its localhost HTTP helper into a deployed production site, including an HTTPS site, is not supported. Do not disable browser security or weaken production CSP to make it work.

Use live mode only in projects you trust to run locally. Applying copy edits automatically runs `package.json`'s optional `scripts["impeccable:manual-edit-validate"]` command in a shell, with your user permissions; review that script before using live mode in an unfamiliar checkout.

For production inspection, use `npx impeccable detect https://example.com` or the browser extension. These inspect the rendered page; they do not provide live variant editing or write changes back to your source.

## CLI

Impeccable includes a standalone CLI for detecting anti-patterns without an AI harness. `npx impeccable` is a small shim that runs the same engine binary the skill uses (installed as a platform-specific optional dependency, or fetched once into `~/.impeccable/bin/`); Node is needed only for `npx` itself, and you can also download the binary directly and put it on your PATH. The CLI does not install the skill.

```bash
npx impeccable detect src/                   # scan a directory
npx impeccable detect index.html             # scan an HTML file
npx impeccable detect https://example.com    # scan a URL (uses an installed Chrome, Chromium, or Edge)
npx impeccable detect --json .               # CI-friendly JSON output
npx impeccable detect --no-config src/       # raw scan, ignoring project config/context
npx impeccable ignores list                  # show detector ignores
npx impeccable ignores add-file "src/legacy/**"
npx impeccable ignores add-value overused-font Inter --reason "Brand font"
```

The detector catches 61 deterministic issues across AI slop (side-tab borders, purple gradients, bounce easing, dark glows) and general design quality (line length, cramped padding, small touch targets, skipped headings, and more).

Human-readable findings are diagnostics written to stderr, so redirect them with `2> findings.txt`. Use `--json` for machine-readable results on stdout. Exit `0` means the scan completed without primary findings, exit `2` means it completed with primary findings, and exit `1` means at least one requested target could not be scanned; operational failure takes precedence for a partial multi-target scan. URL scans inspect the rendered DOM, computed layout, and accessible linked stylesheets; browser security still prevents reading cross-origin CSS without CORS. A clean detector run is evidence, not proof of visual or accessibility quality: it does not replace inspecting the rendered experience across relevant viewports.

By default, `detect` respects the same `.impeccable/config.json` and `.impeccable/config.local.json` detector config as the design hook: `detector.ignoreRules`, `detector.ignoreFiles`, `detector.ignoreValues`, and `detector.designSystem.enabled`. Hook lifecycle settings such as `hook.enabled` only affect automatic hook execution.

For a waiver that should travel with one file instead of the repo config, add an inline comment in the file: `<!-- impeccable-disable overused-font: exported brand doc -->`. The marker works in any comment syntax, scopes to the whole file (or one line with `impeccable-disable-line` / `impeccable-disable-next-line`), and is bypassed by `--no-inline-ignores` or `--no-config`.

Full detector docs: [impeccable.style/docs/detector](https://impeccable.style/docs/detector).

## Supported Tools

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
- [Veto](https://github.com/oleg-koval/veto)
- [Google Antigravity](https://antigravity.google)

## Community & Ecosystem

Join the community and ecosystem conversations:

- GitHub Discussions: file bugs, request features, and help newcomers.
- [Impeccable on npm](https://www.npmjs.com/package/impeccable): grab the CLI, follow releases, and star the package.
- Follow @pbakaus on Twitter for release notes, sample lint reports, and video highlights of new rules.

## Contributing

See [DEVELOP.md](docs/DEVELOP.md) for contributor guidelines and build instructions.

## License

Apache 2.0. See [LICENSE](LICENSE).

---

Created by [Paul Bakaus](https://www.paulbakaus.com)
