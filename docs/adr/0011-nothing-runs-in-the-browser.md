# 0011: Nothing runs in the browser

**Status:** Accepted · **Date:** 2026-10-02

## Context

Impeccable carried its own browser stack next to the skill:

- **Live mode** injected page scripts into the user's dev server (element picker, variant cycling, manual copy edits), served them from a localhost helper, patched CSP headers, and kept framework adapters for Astro, Next, Nuxt, SvelteKit, TanStack, Vite, and more.
- **The decision page** (`serve-question`) opened a local web page to pick a design direction.
- **The component review page** showed plans and assets for sign-off in the browser.
- **URL scans** (`detect https://...`, crate `browser`) drove headless Chrome over CDP.

Agent harnesses now bring their own browser: Claude in Chrome, Playwright MCP, the Codex Browser, and similar tools render pages, take screenshots, and click through flows inside the agent's loop. A second browser stack inside the skill duplicated that, reached into the user's running app, and cost a large share of the repository's code and test infrastructure (live E2E suites, framework fixtures, process reapers for leaked servers).

## Decision

Nothing in Impeccable runs in a browser. The agent uses its harness's browser tool for screenshots and rendered checks, the structured question tool for decisions, and `impeccable detect` on source files.

Removed: live mode (`live*` verbs, `detect-csp`, `crates/live`, the page scripts in `skill/scripts/`, the manual-edit-applier agent), `serve-question`, `component-review` (`ui/`), URL scanning (`crates/browser`), the capture service in `crates/cli`, and their tests and CI jobs.

## Consequences

- `critique` takes its screenshots with the host's browser tool and runs the detector on source files.
- The rules that need a rendered page stay. Impeccable does not drive a browser to evaluate them; the agent's own browser measures the page and the engine evaluates the rules ([0016](0016-rendered-page-rules-via-the-harness-browser.md)).
- `detect` refuses URLs and points to `page-probe`.
- Older skill copies that call a removed verb get a clear "was removed" message.
