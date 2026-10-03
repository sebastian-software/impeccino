# Project Instructions for Claude

## Architecture

There is **one** user-invocable skill, `impeccino`, with **22 commands** underneath it. Users type `/impeccino polish`, `/impeccino audit`, etc. The skill is defined in `skill/`:

- `SKILL.md` — frontmatter (Agent Skills spec fields plus `user-invocable` and `argument-hint`, with the auto-trigger-optimized description), shared design laws, and the **Commands** router table. `skill/` installs as-is in every harness; nothing in it is generated.
- `agents/` — the two shipped roles (`impeccino-finish-reviewer`, `impeccino-documenter`) as Claude Code agent files. SKILL.md's **Shipped agents** section covers hosts without them.
- `reference/` — one `<command>.md` per command (`audit.md`, `polish.md`, `critique.md`, etc.), the shared playbooks the router loads outside the command table (`new-work.md`, `craft-floor.md`, `operate.md`, `routing.md`), and the native platform references (`ios.md`, `android.md`). When a sub-command is matched, the router loads its reference file.
- `scripts/command-metadata.json` — single source of truth for each command's description, argument hint, and (eventually) category. The engine's `pin` verb reads from this.
- `scripts/impeccino` (+ `impeccino.cmd`, `VERSION`): the launcher every skill verb goes through. See **Engine binary** below.
- `impeccino pin` — an engine verb that creates/removes lightweight redirect shims so users can have `/audit` as a standalone shortcut that delegates to `/impeccino audit`.

### Engine binary (the runtime behind every verb)

The skill has no runtime of its own. Every command the skill text runs is `"<skill-base-dir>/scripts/impeccino" <verb>` (Setup step 1 says `impeccino context`; `impeccino.cmd` is the Windows twin for shells without `sh`). `skill/scripts/impeccino` is a POSIX `sh` launcher: it uses `$IMPECCINO_BIN` when explicitly set, otherwise it checks the sibling binary and the version-pinned user-cache binary, then downloads the pinned release into that cache. Automatic candidates must answer the exact engine/version probe; the explicit developer override is not version-checked. The cache is `$XDG_CACHE_HOME/impeccino` when that is absolute, else `~/.cache/impeccino`; on Windows it is `%LOCALAPPDATA%\impeccino` (also used by the MSYS launcher). `IMPECCINO_HOME` overrides the cache root. The launcher exports `IMPECCINO_SKILL_DIR` (the skill dir, for `reference/*.md` and `command-metadata.json`) and `IMPECCINO_SELF` (how the binary spells itself in the commands it prints).

The binary is built from **this repo's Cargo workspace** (`Cargo.toml` at the root, `crates/*`; `cargo build --release -p impeccino`). Its verbs are `context`, `doctor`, `pin`, `surface-brief`, `palette`, `signals` (alias of context-signals), `concept-seed`, `detect` (files, directories, and URLs through agent-browser), `hook`, `hook-before-edit`, and `hooks` (alias of hook-admin). The browser and comp verbs (`live*`, `detect-csp`, `serve-question`, `component-review`, `generate-image`, `comp-spec`, `comp-diff`, `font-match`, `build-phase`, `capture-server`, `embed-prompt`) print a "was removed" message and exit 1 ([ADR 0011](docs/adr/0011-no-own-browser-stack.md), [ADR 0012](docs/adr/0012-no-image-comps.md)), and so do `critique-storage` and `ignores` ([ADR 0020](docs/adr/0020-project-state-is-top-level-files.md)). Observable behavior is pinned by `tests/oracle/`, which is the behavioral contract ([ADR 0015](docs/adr/0015-history-lives-in-git.md)). **Read `docs/ENGINE.md` before touching `crates/`**: it maps the crates.

- **The rule engine is in the workspace.** Every `check_*` / `scan_*` lives in `crates/core`, Apache-2.0 like everything else; `crates/foundation` holds what they are written against (JS semantics, color, the registry, inline ignores, the plain-data input and output types) and `crates/core` re-exports it, so consumers name one crate. The engine is one native binary: there is no WebAssembly build, no in-page bundle, and no DOM layer ([ADR 0013](docs/adr/0013-no-wasm-or-browser-extension.md)). There is no build-time download and no exact toolchain pin: `cargo build --release -p impeccino` works offline on stable.
- **`skill/scripts/VERSION`** pins the engine release (`engine-v<X>` on this repo's GitHub Releases, built by `.github/workflows/release-engine.yml` when `pnpm run release:engine` pushes the tag). The launcher reads it to name the download and the cache dir (ADR 0009); the workspace `Cargo.toml` version must match it (`impeccino --version` prints the crate version, and a test checks the two agree). Bumping it is a release-time decision, like the other manifest versions.
- **Binaries are never tracked.** `skill/scripts/bin/` is gitignored, so `skill/` ships launcher-only and users get the binary on first run (ADR 0010).
- **Tests get a binary** from `IMPECCINO_BIN`, then `skill/scripts/bin/<os-arch>/` (`pnpm run fetch:engine`; `IMPECCINO_BIN=<local build> pnpm run fetch:engine` copies a local build there), then `target/release/impeccino` from a plain `cargo build --release -p impeccino`. `tests/lib/engine-bin.mjs` is the one resolver; suites that need the binary skip cleanly without it.
- **The oracle is the behavior gate.** `tests/oracle/` holds the exact engine behavior goldens; `tests/oracle.test.mjs` replays them against the binary in `pnpm run test`. New cases and intentional behavior changes are recorded from the current source binary (`record.mjs --bin`) and reviewed by hand. `tests/oracle/vectors/calls/` is the frozen function-level snapshot; it cannot be regenerated.
- **What stays JavaScript here:** the repository and test tooling. Impeccino injects nothing into the user's running app ([ADR 0011](docs/adr/0011-no-own-browser-stack.md)); screenshots come from agent-browser or the harness's browser tool, decisions from the host's structured question tool, and `detect <url>` measures rendered pages through agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)).

**Do not add standalone skills** unless there's a strong reason. The consolidation was deliberate: the `/` menu pollution problem is real and gets worse as users install more plugins.

**Do not reintroduce per-domain reference files.** The former `typography.md`, `color-and-contrast.md`, `spatial-design.md`, `motion-design.md`, `interaction-design.md`, `responsive-design.md`, `ux-writing.md`, `cognitive-load.md`, `personas.md`, `heuristics-scoring.md`, `build-floor.md`, and `live-generation.md` guidance now lives in command references and `craft-floor.md`, where it is loaded only when it applies.

### Modes (Persuade / Operate / Read / Experience)

The skill uses four modes, named in SKILL.md's `## Modes` section, instead of the former brand/product **register** axis. A mode names what the visitor's success looks like on the surface in hand:

- **Persuade** — the visitor decides and acts; design is the product. Landing pages, marketing, campaigns, pricing.
- **Operate** — the visitor completes a task. App UI, dashboards, editors, admin, settings, tools.
- **Read** — the visitor understands something. Docs, articles, guides, help, changelogs.
- **Experience** — the visitor is inside the work itself. Portfolios, galleries, showcases.

Three differences from register that matter when editing skill text:

1. **Mode is per surface, not per project.** A tool's landing page is Persuade even though the product is Operate; a fashion house's documentation is Read. Choose from the requested surface.
2. **Mode is not stored in PRODUCT.md.** It persists only in that surface's brief, its section of the project's `SURFACES.md`. There is no `## Register` field and no `extractRegister()`; PRODUCT.md's only bare-value field is `## Platform`. A `## Register` section left over from v3 is reported at boot as deprecated (see the engine's staleness module, `crates/context/src/staleness.rs`) and read by nothing.
3. **There are no register reference files.** `reference/brand.md` and `reference/product.md` are gone. `reference/operate.md` carries the deeper Operate and Read guidance; `reference/new-work.md` owns new surfaces.

**a11y lives in `audit.md`**, not in SKILL.md or the mode guidance. Models over-cautious themselves into safe, underdesigned output when reminded about accessibility at design time. The audit command is the dedicated place for that check.

### Project state (no config file)

Impeccino writes no config file and no state directory into a user project ([ADR 0020](docs/adr/0020-project-state-is-top-level-files.md)). Its project state is `PRODUCT.md`, `DESIGN.md` with its `DESIGN.json` sidecar beside it, and `SURFACES.md` (one marked section per surface; the `surface-brief` verb finds and replaces a section by the target in its `<!-- impeccino:surface {...} -->` marker). Detector and hook decisions come from the project's own files: `<!-- impeccino-disable <rule> -->` comments in DESIGN.md waive a rule project-wide, fonts DESIGN.md declares never count as overused, and `.gitignore` plus `.gitattributes` (`linguist-generated`, `linguist-vendored`) keep files out. The hook is on where its manifest entries are installed; `IMPECCINO_HOOK_DISABLED`, `IMPECCINO_HOOK_QUIET`, and `IMPECCINO_HOOK_LOG` are the only knobs. Runtime state (the hook's session cache, the staleness throttle, the engine binary) lives in the per-user cache; review screenshots go to a temp directory. Critiques are not archived. A leftover `.impeccino/` is reported by the boot (`legacy-state-dir`, one stat) and by doctor; there is no migration code. Do not reintroduce a config file: a decision that needs recording belongs in PRODUCT.md, DESIGN.md, or the project's ADRs.

### Platform (web / ios / android / adaptive)

A second axis, **orthogonal to mode**. Mode answers "what does the visitor come here to do"; platform answers "what's the delivery target and which native conventions apply":

- **web** — a website or web app (including responsive mobile web). The default. No extra rulebook and no reference file: the General rules in SKILL.md cover it.
- **ios** — a native iOS / iPadOS app. Loads `reference/ios.md` (Apple HIG distilled).
- **android** — a native Android app. Loads `reference/android.md` (Material Design 3 distilled).
- **adaptive** — a cross-platform app shipping both iOS and Android from one codebase (Flutter, React Native, KMP) that adapts per OS. Loads **both** `reference/ios.md` and `reference/android.md`. A Flutter/RN app that uses one look on both platforms (Material-everywhere is the Flutter default) is not adaptive; it takes that single platform's value.

PRODUCT.md carries a `## Platform` section with a bare value (`web` / `ios` / `android` / `adaptive`). The `context` verb parses it; a **missing field defaults to `web`** so legacy projects are unaffected. A line that names both native targets (e.g. `ios, android`) is also read as `adaptive`; any other unrecognized value falls back to web **and** `impeccino context` prints a WARNING directive naming the bad value, so a toolchain name or typo never silently gets web guidance. `impeccino context` inlines the native reference(s) directly into its output when the value is `ios`, `android`, or `adaptive` (both), so native conventions land in context without a second model-directed read. `init` (Step 3) confirms an ambiguous platform as part of the product-truth interview, and Step 4 records it as the bare value.

`ios.md` and `android.md` are distilled from the MIT-licensed [ehmo/platform-design-skills](https://github.com/ehmo/platform-design-skills); attribution is in `NOTICE.md`.

Where a command's native guidance diverges too much to share a file, it gets a **native variant**: `reference/<command>.native.md`, listed in SKILL.md's Commands table and routed **instead of** the web file when `setup.platform` is native (Setup step 2). One variant covers ios, android, and adaptive; per-OS specifics stay in the platform refs, which Setup loads regardless. Variants today: `audit.native.md`, `adapt.native.md` (their web files carry a one-line web-only guard that redirects stray native readers). `audit.native.md` mirrors `audit.md`'s report skeleton; change the skeleton in both together. Commands whose divergence the platform refs already cover (`animate`, `layout`) carry nothing extra; don't add in-file translation notes, they make native runs pay for web content.

**`impeccino detect` and the design hook are web-only.** They run HTML, CSS, and web-source rules, so SKILL.md's routing skips `impeccino detect` for any native (`ios` / `android` / `adaptive`) project, and the `hook` and `hook-before-edit` verbs skip their scan when PRODUCT.md declares a native platform — a React Native project is made of exactly the `.tsx` / `.ts` / `.js` files the hook watches.

### Artifact staleness and the doctor pass

Impeccino writes files into user projects, so a released version has to cope with artifacts an older one wrote. Three kinds of drift travel under "out of date" and they are handled separately:

1. **Tool version drift** (installed skill older than published). Owned by whatever installed the skill; the engine no longer checks (ADR 0004).
2. **Schema drift** (an artifact carries fields nothing reads, is missing fields now expected, or sits in a retired location). Deterministic; the engine's staleness module.
3. **Truth drift** (the code moved on and the document no longer describes it). Not mechanical. `document` and `init` own the rewrite; the deep pass measures a proxy and is required to say it is a proxy.

**Two tiers, and the split is a performance contract, not a preference.**

- **Tier 1** runs inside `impeccino context` at boot. It may only spend what a boot already spends: markdown already in memory, a bounded set of stats, and the small JSON files the boot reads regardless. **No directory walks, no git, no cross-workspace sweep.** The one walk it uses is the target-candidate discovery the boot has already paid for. Adding an expensive check here taxes every session in every project.
- **Tier 2** is the deep pass behind `impeccino doctor`, run on demand. Git log, per-workspace sweep, ignore-list validation against the live rule registry, hook launcher resolution.

**Findings are data.** `{ id, artifact, path, severity, summary, fix }`, so the boot directive, the text report, and `--json` all render one set. Severity says what should happen, not how bad it is: `auto` (fix silently on the next write to that file), `mention` (state once, carry on), `route` (name the command that owns the repair). `doctor --fix` applies only `auto`, and only where no judgment is involved.

**Emission discipline.** Boot output is already heavy, so Tier 1 emits **one** `CONTEXT_STALE` directive for the whole set, and `mention` and `route` findings are throttled to once a week per project (cached in `<user cache>/impeccino/staleness-check.json`, outside the project, so no gitignore entry is owed). `auto` findings are never throttled and never shown to the user. Opt out with `IMPECCINO_NO_STALENESS_CHECK=1`; there is no config file to carry a switch. **An oracle case that asserts on other boot directives should pin that env var.**

**Provenance stamps.** PRODUCT.md carries `<!-- impeccino:product-schema N -->` (schema constants live in the engine; template in `init.md`). Without it, every check is a heuristic reconstruction of what era a file came from. **Stamps are schema versions, not release versions**: routine releases do not make PRODUCT.md stale; the schema version changes only when the shape does. **DESIGN.md deliberately carries no stamp** because it follows the external design.md spec that Stitch's linter validates, and every DESIGN.md signal (sidecar `schemaVersion`, sidecar mtime, section coverage, git drift) is measurable without one.

**When you retire a PRODUCT.md field, add it to the engine's deprecated-sections list** with the reason (and record the new boot output as an oracle case). The reason is not decoration: told only that a field is deprecated, models preserve it "just in case", which is how a retired axis keeps steering current output.

**`doctor` is a utility command, not a design command.** It has a route in SKILL.md and instructions in `reference/doctor.md`, but no row in the `## Commands` table, no entry in `command-metadata.json`, and no place in the `pin` verb's valid-command list. It is not part of the 22 commands counted from that table. Keep maintenance tooling out of the design menu.

## concept-seed is local

`impeccino concept-seed` assigns an index into the model's own ordered list of grounded directions (a surface round deals three) from a hash of the seed key. It has no catalog, makes no network calls, and sends no choice ping; `--chosen` and `--kind` answer with a "was removed" note ([ADR 0019](docs/adr/0019-concept-seed-is-local.md)). Impeccable's concept catalog belongs to its paid service: never fetch it or copy its data into this repo.

## Prose: read docs/STYLE.md before writing user-facing copy

Editorial brief is at `docs/STYLE.md`. Read it before editing the READMEs or any user-facing copy. The rules exist because the project has been called out for AI prose before.

`pnpm run check`'s `validateProse` step (in `scripts/check.js`) enforces a denylist: em dashes (`—` and HTML entities), the `--` em-dash substitute, `load-bearing`, `highest-leverage`, `biggest unlock`, `seamless`, `robust`, `delve`, `elevate`, `empower`, `underscore`, `pivotal`, `tapestry`, `data-driven`, `reflex defaults`, `collapses into monoculture`, `in today's`, `gone are the days`, `whether you're`, `let's dive in`, `in summary`, `in conclusion`, `moreover`, `furthermore`. Each rule prints a rationale and a suggested replacement when it fires. **Do not silently work around the regex.** If a banned word has earned a real meaning here, raise it as a `docs/STYLE.md` amendment.

`validateProse` scans `README.md` only. Docs use the same editorial brief but are not part of this automated gate.

**`skill/` is checked too, by a second gate.** `validateProse` skips it because the full ruleset does not fit LLM-facing reference instructions. `validateSkillProse` then scans `skill/**/*.md` (markdown only, not the launcher under `skill/scripts/`) and fails the build on em dashes plus the subset of phrases with no technical reading: `load-bearing`, `highest-leverage`, `biggest unlock`, `reflex defaults`, `collapses into monoculture`, `data-driven`, `delve`, `tapestry`, `in today's`, `gone are the days`, `let's dive in`, `in summary`, `in conclusion`. The words it does *not* enforce in `skill/` (`seamless`, `robust`, `elevate`, and friends) are the ones with legitimate technical uses. Net effect: an em dash in `skill/reference/*.md` fails `pnpm run check`; an em dash in a `scripts/*.js` code comment does not.

The deeper structural issues (negation pivot, triadic auto-pilot, uniform paragraph rhythm, hollow confidence) require human judgment. `docs/STYLE.md` lists them. Use them on every editorial pass.

## No build

`skill/` is the skill and installs as-is; there is no build, installer, update check, or marketplace package. The decisions are recorded as living Light ADRs in `docs/adr/`; update an existing one when a decision changes, and add one when a change alters how the skill is built or delivered.

```bash
pnpm run check            # Count claims, skill frontmatter limits, prose gates
pnpm run fetch:engine     # Download the pinned engine binary for this machine into skill/scripts/bin/
```

### One skill for every harness

There are no build-time placeholders or provider blocks. Write skill text that holds in every harness:

- Commands are `/impeccino <command>`; SKILL.md tells hosts with another sigil (Codex: `$impeccino`) to translate.
- The launcher is `"<skill-base-dir>/scripts/impeccino" <verb>`, quoted because install paths can contain spaces.
- Questions go through "the host's structured question tool", not a named tool.
- Harness- or model-specific guidance is a labelled paragraph (`In Codex: ...`, `**GPT models (Codex):**`).
- SKILL.md frontmatter uses the Agent Skills spec fields plus `user-invocable` and `argument-hint`, which runtimes tolerate (ADR 0008); keep `allowed-tools` out, since Claude Code then blocks non-interactive activation.
- Agents are plain Claude Code agent files in `skill/agents/`; SKILL.md's "Shipped agents" section covers hosts that lack them.

`tests/skill-source.test.js` pins these rules. Nothing generated is tracked (ADR 0002).

Local state files inside harness directories (e.g. `.claude/scheduled_tasks.lock`, `.claude/settings.local.json`) ARE gitignored.

### Contributing, Issue, and PR Guidelines

Open an issue before larger changes so the direction is agreed first; small fixes can go straight to a PR. AI agents disclose AI assistance in commits and PR descriptions.

## Testing

```bash
pnpm run test                  # Default suites: core (unit, skill source, tooling) + oracle
pnpm run test:skill-behavior   # Opt-in: LLM-backed checks that the skill text actually drives the agent's setup flow
pnpm run test:skill-workflow   # Opt-in: provider-backed completed workflows (full build, finish handoff)
```

Every JavaScript test runs on Vitest (`vitest.config.mjs`), grouped into suites by `scripts/run-tests.mjs`. The oracle replay (`tests/oracle.test.mjs`), which spawns the engine binary, skips cleanly when no binary is found (`pnpm run fetch:engine` or `IMPECCINO_BIN`). CI runs the core suite on Node 22, 24, and 26. Runtime unit and integration tests live under `crates/` and run with `cargo test --workspace`; the oracle goldens pin observable verb behavior across the same workspace.

### The runner ends what it starts

`scripts/run-tests.mjs` runs each suite command as its own process-group leader, ends that group on `SIGINT` / `SIGTERM` / `SIGHUP`, and SIGKILLs it when it exceeds the wall-clock cap (`IMPECCINO_TEST_WALL_CLOCK_MS`, or a suite's `wallClockMs`), so a suite blocked in a synchronous call still ends. Nothing in the runner may use `spawnSync`: a blocked event loop cannot run those handlers. The live-server reaper and leak check that used to sit here left with live mode ([ADR 0011](docs/adr/0011-no-own-browser-stack.md)).

### Which opt-in suite a change owes

The default suite does not cover everything. When a change touches one of these areas, run the matching opt-in suite before shipping. The canonical mapping is the `triggers` lists in `scripts/test-suites.mjs`; this table mirrors it for the areas that need a manual run.

| Area touched | Run | Cost |
|---|---|---|
| `SKILL.md` Setup, Setup-adjacent reference files, engine version bump (`skill/scripts/VERSION`) | `pnpm run test:skill-behavior` | ~5 min, bills the provider keys in `.env` |
| `skill/` changes that affect a completed build or the finish handoff (new-work flow, agents, finish review, documentation) | `pnpm run test:skill-workflow` | bills a provider key; its test harness needs Playwright Chromium (`npx playwright install chromium` once) |

For verb-level behavior changes in `crates/`, run focused crate tests and `cargo test --workspace`, then `cargo build --release -p impeccino`. Run `IMPECCINO_BIN="$PWD/target/release/impeccino" pnpm run test` to exercise the changed source rather than an older downloaded release. Add a new oracle case when the contract grows and review golden changes by hand.



### Skill-behavior tests

`tests/skill-behavior/scenarios.test.mjs` is the LLM-backed safety net for edits to `skill/SKILL.md` and the Setup-adjacent reference files (`init.md`, `document.md`, `new-work.md`, sub-command refs). It inlines the source `skill/SKILL.md` into the system prompt of a real LLM, gives the agent `bash` / `read` / `write` / `list` tools scoped to a temp workspace, and asserts on the tool-call trace — not on the model's free-form output. The trace is the source of truth. The end-to-end flows live in the separate opt-in `skill-workflow` suite (`tests/skill-workflow/full-build.test.mjs`, `finish-handoff.test.mjs`), which reuses this harness, gives the agent screenshot tools backed by Playwright Chromium in the test harness, and asserts on completed builds, fresh captures, and documentation artifacts.

```bash
pnpm run test:skill-behavior                                        # full suite, ~5 min, ~$0.50-1.50 across providers
IMPECCINO_SKILL_BEHAVIOR_MODELS=gemini-3.7-flash pnpm run test:skill-behavior   # scope to one provider
IMPECCINO_SKILL_BEHAVIOR_VERBOSE=1 pnpm run test:skill-behavior    # dump per-scenario trace JSON to stderr (use when iterating)
```

**Frontier tiers, more than one family.** The lineup is `DEFAULT_MODELS` in `tests/skill-behavior/providers.mjs`, currently `claude-sonnet-5`, `gpt-5.6-terra`, and `gemini-3.7-flash`. `gpt-5.6-luna` and `deepseek-v4-flash` were dropped in 2026-08: below the frontier tier they fail scenarios for model-floor reasons rather than skill-text defects, and a suite that is always red is a suite nobody reads. **Don't substitute Claude alone**: many of the most useful findings come from divergence between families, so keep at least two. The dropped models stay selectable via `IMPECCINO_SKILL_BEHAVIOR_MODELS` when a Setup or routing change warrants a wider sweep.

**Auth** lives in repo-root `.env` (gitignored). Local runs skip providers whose keys are unset. CI runs the billed suite only from the manual `skill_behavior` workflow checkbox, and that job fails if no provider scenario actually runs.

**The scenario list and the baseline live in `tests/skill-behavior/README.md`**, not here. Read that table before changing Setup or routing text, and update it in the same change. Duplicating it in this file is how it went stale before.

**Cost.** Each run is real LLM calls, billed to the keys in `.env`. Production-tier models put a full sweep around $0.50-1.50. Keep it out of automatic CI; the workflow's manual checkbox is available when a billed CI run is wanted.

**Adding a scenario.** Write the fixture in `tests/skill-behavior/fixtures.mjs`, add the `it()` block in `scenarios.test.mjs` (the harness uses the source `skill/` dir via a symlink, so no rebuild needed), and update the baseline table in the suite's README. The harness's `fileLoaded(trace, filename)` helper checks both `read` and bash `cat` — different models prefer different tools.

**The harness copies `skill/` as-is**, the way a skill manager installs it, so SKILL.md and reference edits show up immediately; the launcher under `skill/scripts/` resolves the binary the same way tests do.

## Internal engine interface

The public interface is the skill and its agent workflows. The native CLI and Rust crates are internal implementation details ([ADR 0005](docs/adr/0005-no-marketplace-packages.md)). Direct calls are for skill/hook integration and contributor debugging; see [ENGINE.md](docs/ENGINE.md) for the internal detector contract. Update callers and regression coverage together when changing it. Do not introduce standalone terminal/CI workflows into user documentation.

## Versioning

**Feature PRs do not bump versions.** Bumping is a release step, not part of the change that earns the release: a version in a feature branch conflicts with every other open branch. Land the code first; the maintainer bumps when cutting the release. This holds even though the "Bump when: ..." notes below name the source dirs — those say *which* component a change belongs to, not *when* to edit the manifest. The only PR that touches a manifest version is one whose purpose is the release itself.

There are two independently versioned components: the engine and the skill ([ADR 0014](docs/adr/0014-releases-are-tags.md)). Only bump the one(s) that actually changed:

**Engine** (`skill/scripts/VERSION`, and the workspace `Cargo.toml` version to match):
- The engine release the launcher downloads. Bump both together; `tests/skill-source.test.js` fails on a mismatch.

**Skill**:
- `skill/SKILL.md` → `metadata.version`
- Bump when: skill content changes (`skill/`, reference files, command metadata, etc.). A skill release is a tag on the commit skill managers pin; there is nothing to build or upload.

After bumping, see **Releases** below for how to tag and publish.

## Releases

A release is an annotated tag per component: `engine-v` and `skill-v`. GitHub generates the notes from the commits since that component's previous tag ([ADR 0014](docs/adr/0014-releases-are-tags.md)), so commit messages are the release notes and should describe user-facing impact.

Workflow for either component:

1. Bump the manifest version (see Versioning above).
2. Commit and push.
3. Run `pnpm run release:<engine|skill>`. Preview first with `node scripts/release.mjs <component> --dry-run`.

The script refuses to run if the working tree is dirty, HEAD is ahead of origin, or the tag already exists. The engine release only tags and pushes; `release-engine.yml` builds the five binaries, attests each, generates `THIRD-PARTY-NOTICES.txt` from the locked dependency graph for every release target, and publishes everything as one immutable release. Binaries are not code-signed. Skill releases attach nothing.

If you need to fix release notes after the fact: `gh release edit <tag> --notes-file <md>`.

### Release order is enforced

The skill launcher resolves the engine binary for the pinned version in `skill/scripts/VERSION`, so the order is:

1. `pnpm run release:engine` publishes `engine-v<version>`.
2. `pnpm run pin:engine` (`scripts/pin-engine.mjs`) downloads the five assets, verifies each build attestation with `gh attestation verify` (needs the signed-in `gh` CLI), and writes `skill/scripts/engine.sha256`. Commit it together with the `VERSION` bump; skill managers pin commits, so a bump must point at a published and pinned engine.
3. `pnpm run release:skill`, which refuses without pins for the engine version (`node scripts/pin-engine.mjs --check` checks them offline).

The launchers and `fetch-engine.mjs` accept a download only if it matches `engine.sha256`; a version without pins is refused, so local engine work runs through `IMPECCINO_BIN` ([ADR 0010](docs/adr/0010-launcher-fetches-the-engine.md)).

`scripts/check-engine-release.mjs` verifies the five binaries exist for the pinned version (ranged GET per asset; honors `IMPECCINO_DOWNLOAD_BASE`) and names exactly what is missing. `scripts/release.mjs` runs it, and the pin check, as hard gates before tagging the skill. `IMPECCINO_SKIP_ENGINE_CHECK=1` bypasses it only when the assets exist but the probe is unreachable. CI's `engine-release-ready` job runs both checks.

## Adding New Commands

All commands live under `/impeccino`. To add a new one:

1. Create `skill/reference/<command>.md` with the command's instructions (this is what the LLM loads when the command is invoked)
2. Add a row to the `## Commands` table in `skill/SKILL.md` and add the command to its `argument-hint` frontmatter field
3. Add the command to the `/impeccino <command>` lists in `skill/reference/audit.md`, `audit.native.md`, and `critique.md`
4. Add it to the `pin` verb's valid-command list (`crates/context`) and record the pin/unpin oracle case
5. Add its metadata (description + argumentHint) to `skill/scripts/command-metadata.json`

`pnpm run check` counts commands from the `## Commands` table automatically. Update the command list and count in these README locations when the total changes:

- `README.md` — intro, command count, commands table

`checkCounts` in `scripts/check.js` checks command counts in `README.md` and detector-rule counts in both `README.md` and `AGENTS.md`.

## Adding or modifying anti-pattern detection rules

The rule logic lives in `crates/core`; the detector ships 61 rules. Source-file rules run in the text and static HTML engines; rules that need layout run in `crates/core/src/browser` over a page snapshot that `impeccino detect <url>` measures through agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)). There is no WebAssembly build or bundle to refresh ([ADR 0013](docs/adr/0013-no-wasm-or-browser-extension.md)); when a rendered-page rule reads a new computed-style property, add it to both `STYLE_PROPS` in `crates/foundation/src/browser/snapshot.rs` and `crates/cli/assets/page-snapshot.js` (a test checks they agree). Everything a rule change touches:

| Where | What it is |
|---|---|
| `crates/foundation` | What checks are written against: the rule registry (`registry.rs`), findings, color, inline ignores, CSS measures, and the plain-data input and output types |
| `crates/core` | The checks themselves, plus the re-exports that let consumers name one crate |
| `crates/html`, `crates/detect` | The engines: HTML parsing, cascade, the static document model, file walking, config, output. They call the checks through `impeccino_core::checks::*` |
| `tests/fixtures/antipatterns/{rule-id}.html` | Hand-edited fixture (two columns, should-flag / should-pass, unique headings, explicit pixel dimensions) |
| `tests/oracle/golden/*` | Recorded from the binary with `node tests/oracle/record.mjs --bin detect-`, reviewed by hand. The oracle corpus is the observable contract of `impeccino detect` and every other verb |
| `tests/oracle/vectors/calls/` | Frozen function-level vectors; replayed by `crates/core/tests/vectors.rs` through `impeccino_core::vectors::call` |
| `skill/SKILL.md` and `reference/*.md` | Hand-edited if the rule introduces new design guidance |

Order for a new rule: fixture here first, registry row in `crates/foundation/src/registry.rs`, the check in `crates/core` against that fixture (text engine and, where it applies, the static HTML engine), oracle case + golden, then `cargo build --release -p impeccino` and `IMPECCINO_BIN="$PWD/target/release/impeccino" pnpm run test`. `pnpm run check` reads the rule count from `crates/foundation/src/registry.rs` and validates the counts quoted in `README.md` and `AGENTS.md`.

### Rule packs (downstream crates adding rules)

A crate that depends on this workspace can add rules without forking it: implement `impeccino_core::rule_pack::RulePack` (the text hook) and, for the static engine, `impeccino_html::StaticRulePack`, call `impeccino_core::rule_pack::install(&PACK)` at startup, and hand the pack to the engine through `TextOptions` / `ScanOptions`, `DetectHtmlOptions`, or `StaticHtmlEngine`. Every hook runs after the built-ins and before inline ignores, so built-in output with no pack installed is byte-identical, which the oracle enforces. The registry keeps `ANTIPATTERNS` as the built-in list and `registry::extend` appends a pack's rows, panicking on an id collision. The DOM hooks and the wasm `detect` feature left with the WebAssembly build ([ADR 0013](docs/adr/0013-no-wasm-or-browser-extension.md)). Full contract in `docs/ENGINE.md` ("Rule packs"). The shipped `impeccino` binary installs no pack, and nothing in this repo should start doing so.
