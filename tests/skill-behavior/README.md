# Skill-behavior tests

LLM-backed scenarios that verify how the impeccino skill drives context,
command-reference, new-work, and native-platform loading. Each scenario runs
against the default Anthropic, OpenAI, and Google models. DeepSeek remains
available through `IMPECCINO_SKILL_BEHAVIOR_MODELS`.

These are the tests you re-run when you refactor anything in SKILL.md's
`## Setup` section. They fail when the agent stops following the loading
contract.

## Run

```bash
pnpm run test:skill-behavior
IMPECCINO_SKILL_BEHAVIOR_VERBOSE=1 pnpm run test:skill-behavior   # dump per-scenario traces
IMPECCINO_SKILL_BEHAVIOR_MODELS=claude-sonnet-5 pnpm run test:skill-behavior   # scope to one model
IMPECCINO_SKILL_BEHAVIOR_EFFORT=xhigh pnpm run test:skill-behavior             # OpenAI reasoning effort (default: high)
```

Requires `.env` at repo root with at least one of `ANTHROPIC_API_KEY`,
`OPENAI_API_KEY`, `GOOGLE_CLOUD_API_KEY`, `DEEPSEEK_API_KEY`. Providers without a key are
skipped, not failed.

Also requires the engine binary (`pnpm run fetch:engine`, or `IMPECCINO_BIN`).
The staged skill dir ships the launcher (`scripts/impeccino`); the harness
exports `IMPECCINO_BIN` into every bash call the agent makes, so the launcher
resolves the binary in the generated fixture without a download. Without a
binary the suites skip.

To run a single scenario against one model:

```bash
IMPECCINO_SKILL_BEHAVIOR_MODELS=claude-sonnet-5 IMPECCINO_SKILL_BEHAVIOR_VERBOSE=1 \
  pnpm exec vitest run --testTimeout=600000 -t "scenario 6" tests/skill-behavior/scenarios.test.mjs
```

## How it works

### Protocol versus full completion

`test:skill-behavior` now runs only `scenarios.test.mjs`. Routing cases stop
at the successful reference/context checkpoint they assert, with a ten-step
ceiling; shell access is context-only. They do **not** claim that a page was
built or reviewed. Editing/fallback controls retain their original assertions.
Failed file reads do not count as project exploration.

Full workflows moved to `tests/skill-workflow/full-build.test.mjs`:

```bash
pnpm run fetch:engine
pnpm exec playwright install chromium
pnpm run test:skill-workflow
```

This separately billed suite defaults to Claude only; use
`IMPECCINO_SKILL_BEHAVIOR_MODELS` to explicitly choose another model or sweep.
It preflights a local server and Chromium before each provider turn, exposing
real desktop/mobile screenshot and PNG viewing tools. Text-only fixtures use
system fonts and block external browser requests. No extra skill prose is added.
The API harness is not the actual Claude Code host, nor is its shell sandboxed.

Each workflow has a 50-step/840-second ceiling. Reaching a budget or output
limit fails explicitly; routing checkpoints cannot satisfy completion. UI
workflows require desktop and mobile captures matching the final local sources after
its last edit. Approval/brief-before-code and redesign documentation-at-finish
checks remain, as does exactly one context load across the completed turn.
CI runs this lane only when its manual `skill_workflow` checkbox is enabled.
Ordinary protocol CI now fetches its engine instead of silently skipping for
a missing binary. Full-build results must be reported separately from routing.

## Scenarios

| # | Setup | Assertion |
|---|---|---|
| 1 | empty workspace | runs `impeccino context`; loads `reference/init.md` before implementation; automation is not an init bypass |
| 2 | PRODUCT.md only | runs `impeccino context` 1-3 times; loads `reference/new-work.md` to resolve visual authority, establish a world when needed, and develop the surface |
| 3 | PRODUCT.md + DESIGN.md | runs `impeccino context` 1-3 times; receives the committed design system and loads `reference/new-work.md` for the task-scoped concept |
| 4 | PRODUCT.md + DESIGN.md, context already loaded in turn 1 | turn 2 does **not** re-run `impeccino context` |
| 5 | PRODUCT.md without the legacy `## Register` field and no DESIGN.md | runs `impeccino context`; greenfield craft loads `reference/new-work.md`, not init, to establish the missing world |
| 6 | PRODUCT.md + DESIGN.md + a minimal `index.html`; prompt is `/impeccino polish` | loads `reference/polish.md` |
| 7 | same fixture; prompt is `/impeccino audit` | loads `reference/audit.md` |
| 8 | PRODUCT.md + DESIGN.md + a SvelteKit scaffold (`src/app.css`, components, `+page.svelte`); prompt is `/impeccino polish src/routes/+page.svelte` | reads at least one project code file (CSS / component / page) — not just the skill's reference files |
| 10 | no PRODUCT.md + a minimal `index.html`; prompt is `/impeccino polish index.html` | runs `impeccino context`, loads `reference/polish.md`, and does **not** divert into `reference/init.md` |
| 11 | empty workspace; prompt is `/impeccino shape ...` | runs `impeccino context`; resolves `reference/init.md` before planning the surface |
| 12 | empty workspace; prompt is natural-language build intent with no command word | runs `impeccino context`; resolves `reference/init.md` before implementation |
| 13 | empty workspace; prompt is `/impeccino teach` | runs `impeccino context` and diverts into `reference/init.md` because `teach` aliases `init` |
| 14 | PRODUCT.md with `## Platform: ios` (native iOS app); prompt is `/impeccino craft a tide detail screen` | `impeccino context` runs and emits the contents of `reference/ios.md` directly, placing native conventions in context without a second model-directed read |
| 15 | same iOS fixture; prompt is `/impeccino audit` | agent loads `reference/audit.native.md` (the Commands-table native variant, routed instead of `audit.md`) |
| 16 | existing surface, with and without PRODUCT.md; asks where to start | completes relevant advice without edits, interviews, menu scans, or explicit invented refinement prerequisites; reference coverage is diagnostic |
| 17 | existing surface; asks whether critique is required before polish | completes read-only advice distinguishing assessment from implementation and explaining critique is optional; reference coverage is diagnostic |
| 18 | existing surface; explicitly requests polish followed by a next-command recommendation | loads `polish.md` rather than substituting workflow advice for the requested work |
| 19 | tiny spacing edit with PRODUCT.md + DESIGN.md; Bash denied, a real-loader success control, a denied-launcher planning-only case, and a denied-launcher documentation case (PRODUCT.md + index.html, no DESIGN.md) | edits require successful playbook/craft-floor reads and a pre-edit denial warning; planning stays read-only and skips craft-floor; documentation requires successful document.md and source reads before any DESIGN.md write, with the denial disclosed before the first tool call after the denied launcher |

### Scoping a run while investigating

Both files honor Vitest's `-t <name pattern>`, which is much cheaper than a full sweep
when bisecting one scenario:

```bash
CI=1 IMPECCINO_SKILL_BEHAVIOR_MODELS=deepseek-v4-flash \
  pnpm exec vitest run --testTimeout=300000 \
  -t "bolder refinement" tests/skill-workflow/full-build.test.mjs
```

Use the suite's 900000ms timeout for full workflow cases. Pipe to a file rather than `tail`; Vitest prints the failing-test summary at the end,
and truncating it costs you the per-model attribution.
