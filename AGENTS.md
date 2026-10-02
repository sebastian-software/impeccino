# Repository Guidelines

## Project Structure & Module Organization

`skill/` is the Impeccino skill, and it installs as-is in every harness: `SKILL.md`, `reference/`, `scripts/`, and `agents/` (Claude Code agent files). Nothing in it is generated; it carries no build-time placeholders or provider blocks, and harness differences are written as labelled prose ("In Codex: ..."). `skill/scripts/` holds the launcher (`impeccino`, `impeccino.cmd`), the pinned engine `VERSION`, and `command-metadata.json`. Every skill verb (`<skill-base-dir>/scripts/impeccino <verb>`) runs in the engine binary, built from this repo's Cargo workspace under `crates/`; `skill/scripts/VERSION` pins the released binary the launcher fetches. Read `docs/ENGINE.md` before changing runtime code. Repository tooling (checks, releases, the test runner) lives in `scripts/`. Regression coverage lives in the Rust crates and `tests/`, including fixtures under `tests/fixtures/` and behavior goldens under `tests/oracle/`. Impeccino injects nothing into the user's app ([ADR 0011](docs/adr/0011-no-own-browser-stack.md)): there is no live mode, no WebAssembly build, and no browser extension ([ADR 0013](docs/adr/0013-no-wasm-or-browser-extension.md)); rendered pages are measured through agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)). In a user project it keeps no config file and no state directory: PRODUCT.md, DESIGN.md with its DESIGN.json sidecar, and SURFACES.md are the whole footprint, and runtime state lives in the user cache ([ADR 0020](docs/adr/0020-project-state-is-top-level-files.md)). Nothing generated is tracked. The decisions behind this layout are recorded as Light ADRs in `docs/adr/`.

## Build, Test, and Development Commands

- `cargo build --release -p impeccino` - build this checkout's runtime into `target/release/impeccino`.
- `cargo test --workspace` - run the Rust workspace tests.
- `pnpm run check` - repository checks: count claims, skill frontmatter limits, and the prose gates. There is no skill build.
- `pnpm exec vitest run tests/skill-source.test.js` - run one test file.
- `pnpm run fetch:engine` - download the pinned engine binary for this machine into `skill/scripts/bin/<os>-<arch>/` (or set `IMPECCINO_BIN` to a local build). The oracle suite skips without it.
- `pnpm run test` - run the default suites, `core` and `oracle` (the oracle replays against the engine binary).
- `pnpm run test:skill-behavior` - opt-in LLM-backed checks that the SKILL.md Setup flow actually drives the agent (the model lineup is `DEFAULT_MODELS` in `tests/skill-behavior/providers.mjs`; needs `.env` with provider keys).
- `pnpm run test:skill-workflow` - opt-in provider-backed completed workflows (full build, finish handoff); its test harness drives Playwright Chromium for screenshots (`npx playwright install chromium` once).

Run `pnpm run check` after changing anything in `skill/` or user-facing counts.

## One Skill for Every Harness

`skill/` is what Dalo and skills.sh consume, unchanged (ADR 0003). Impeccino has no installer, update check, or marketplace package (ADRs 0003 to 0005); hooks are a per-project opt-in through `/impeccino hooks on` (ADR 0007). `tests/skill-source.test.js` pins the rules below.

Write skill text so it holds in every harness:

- Name commands as `/impeccino <command>`; SKILL.md tells hosts with another sigil (Codex: `$impeccino`) to translate.
- Run the launcher as `"<skill-base-dir>/scripts/impeccino" <verb>`, quoted, because install paths can contain spaces. Agents never see SKILL.md, so they receive `<scripts-path>` from the parent instead.
- Ask through "the host's structured question tool", not a named tool.
- Put harness- or model-specific guidance in a labelled paragraph (`In Codex: ...`, `**GPT models (Codex):**`) instead of a build-time block.
- SKILL.md frontmatter uses the Agent Skills spec fields (`name`, `description`, `license`, `compatibility`, `metadata`) plus `user-invocable` and `argument-hint`, which runtimes tolerate (ADR 0008); strict validators such as Codex's `quick_validate.py` flag the extras, which is expected. Keep `allowed-tools` out. `metadata.version` is the skill version.

Nothing generated is tracked (ADR 0002). If a change seems to need a derived file in git, derive it where it is consumed instead.

## Sandbox gotchas for Codex agents

Some repo workflows need to run outside the sandbox in the desktop app:

- GitHub SSH operations that depend on the 1Password SSH agent, such as `gh pr checkout`, may fail in the sandbox with `sign_and_send_pubkey` or no 1Password approval prompt. Rerun them outside the sandbox instead of falling back to unrelated workarounds.
- The oracle suite spawns the engine binary many times; `pnpm run test` runs it through Vitest (`tests/oracle.test.mjs`), and `node tests/oracle/run.mjs` replays it directly.

## Coding Style & Naming Conventions

Use ESM, semicolons, and the existing two-space indentation style in JS, HTML, and CSS. Prefer small, single-purpose modules over large abstractions. Keep filenames descriptive and lowercase with hyphens where needed; skill entrypoints stay as `SKILL.md`, build and test helpers use `.js` or `.mjs`. In source frontmatter, use clear kebab-case names and concise descriptions. There is no dedicated formatter or linter configured here, so match surrounding code closely.

For Rust, follow the surrounding crate's conventions and workspace formatting configuration. Keep changes scoped; do not reformat unrelated modules.

## Testing Guidelines

Tests run on Vitest (`vitest.config.mjs`); `scripts/run-tests.mjs` groups them into suites. Name tests `*.test.js` or `*.test.mjs` and place new fixtures near the behavior they cover, usually under `tests/fixtures/`. Prefer targeted test runs while iterating, then finish with `pnpm run test`.

For runtime changes under `crates/`, add a failing regression in the affected crate, run its focused tests, then `cargo test --workspace`. Rebuild with `cargo build --release -p impeccino` and run `IMPECCINO_BIN="$PWD/target/release/impeccino" pnpm run test` so the oracle exercises the changed source, not an older downloaded release. Review intended oracle changes by hand; never overwrite goldens just to make a regression pass. `tests/oracle/vectors/calls/` contains frozen function-level vectors and must not be regenerated.

For changes to `skill/SKILL.md`'s Setup section or any Setup-touching reference file (`init.md`, `document.md`, `new-work.md`, sub-command refs), also run `pnpm run test:skill-behavior`. The suite spawns current real models (the `DEFAULT_MODELS` lineup in `tests/skill-behavior/providers.mjs`) with the source SKILL.md inlined as system prompt and a workspace-scoped tool set, then asserts on the tool-call trace. Provider keys live in repo-root `.env`; missing keys skip cleanly. Scope to one provider with `IMPECCINO_SKILL_BEHAVIOR_MODELS=<id>`; add `IMPECCINO_SKILL_BEHAVIOR_VERBOSE=1` to dump per-scenario traces. Baseline and per-scenario assertions live in `tests/skill-behavior/README.md`.

Other area-to-suite obligations (the canonical mapping is the `triggers` lists in `scripts/test-suites.mjs`; CLAUDE.md carries the table): an engine version bump (`skill/scripts/VERSION`) owes `pnpm run test:skill-behavior` on top of the default run, and changes across `skill/` that alter a completed build or the finish handoff owe `pnpm run test:skill-workflow`.

## Anti-pattern detection rules

The rule engine lives in this workspace. `crates/core` holds the checks; `crates/foundation` holds the registry (`crates/foundation/src/registry.rs`) and shared types. `crates/html` and `crates/detect` provide the static HTML and the CLI/text paths. Most rules run on source files; the rules that need layout run in `crates/core/src/browser` over a snapshot that `impeccino detect <url>` measures through agent-browser ([ADR 0016](docs/adr/0016-rendered-pages-through-agent-browser.md)). A rendered-page rule that reads a new computed-style property needs it in both `STYLE_PROPS` (`crates/foundation/src/browser/snapshot.rs`) and `crates/cli/assets/page-snapshot.js`. See `docs/ENGINE.md` for the crate map; the oracle corpus (`tests/oracle/`) is the observable contract ([ADR 0015](docs/adr/0015-history-lives-in-git.md)).

Add a fixture first under `tests/fixtures/antipatterns/` with should-flag and should-pass columns, at least four flag cases and five false-positive shapes, unique headings, and explicit pixel dimensions. Add failing Rust coverage before implementing the rule. Cover each affected engine path (text and static HTML) and add or update an oracle case (`node tests/oracle/record.mjs --bin <prefix>`, golden reviewed by hand). When a rule introduces design guidance, update `skill/SKILL.md` or `skill/reference/*.md` too.

Rebuild the native binary, run the Rust and Vitest checks above, and run `pnpm run check`, which reads the rule count from `crates/foundation/src/registry.rs` and validates the counts quoted in the READMEs and this file. There is no bundle step and no generated asset to commit.

## Commit & Pull Request Guidelines

Recent history favors short, imperative subjects such as `Fix: ...`, `Add ...`, `Improve ...`, or `Bump ...`. Keep commits focused and explain the user-facing impact when it is not obvious. PRs should summarize what changed, list validation performed, and link the ADR when a change alters how the skill is built or delivered.

**Do not bump manifest versions in a feature PR.** Bumping is a release step: a version in a feature branch conflicts with every other open branch. Land the code; the maintainer bumps `skill/SKILL.md` (`metadata.version`) / `skill/scripts/VERSION` and `Cargo.toml` when cutting the release (see **Releases**). The only PR that touches a manifest version is one whose purpose is the release itself.

## Contributing, Issue, and PR Guidelines

Open an issue before larger changes so the direction is agreed first; small fixes can go straight to a PR. AI agents disclose AI assistance in commits and PR descriptions.

## Releases

A release is an annotated tag per component, and GitHub generates the notes from the commits since that component's previous tag ([ADR 0014](docs/adr/0014-releases-are-tags.md)), so commit messages should describe user-facing impact. The components: `engine-v` (`skill/scripts/VERSION` plus the matching `Cargo.toml` version; `release-engine.yml` builds and publishes the binaries) and `skill-v` (`skill/SKILL.md` `metadata.version`; a tag only, no artifacts). Order: publish the engine release, pin it into the skill with `pnpm run pin:engine` (verifies the build attestations, writes `skill/scripts/engine.sha256`), commit, then release the skill. Flow: bump the relevant manifest, commit, push, then `pnpm run release:<engine|skill>` (or `node scripts/release.mjs <component> --dry-run` first). The script refuses on a dirty tree or an unpushed HEAD, and it refuses to tag the skill until `scripts/check-engine-release.mjs` finds every engine asset for the pinned version. Fix already-shipped notes with `gh release edit <tag> --notes-file <md>`.

## Contributor Notes

Fix behavior at its source: `skill/` for skill text, `crates/` for runtime behavior, `scripts/` for tooling. Record decisions that change how Impeccino is built or delivered in `docs/adr/`; ADRs are living documents, so update the existing one when a decision changes (git keeps the old versions).
