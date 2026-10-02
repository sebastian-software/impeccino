# 0011: No browser stack of its own

**Status:** Accepted · **Date:** 2026-10-02

## Context

Impeccable carried its own browser stack next to the skill:

- **Live mode** injected page scripts into the user's dev server (element picker, variant cycling, manual copy edits), served them from a localhost helper, patched CSP headers, and kept framework adapters for Astro, Next, Nuxt, SvelteKit, TanStack, Vite, and more.
- **The decision page** (`serve-question`) opened a local web page to pick a design direction.
- **The component review page** showed plans and assets for sign-off in the browser.
- **URL scans** (`detect https://...`, crate `browser`) drove headless Chrome over its own CDP client and browser discovery.

Agents now have browsers of their own: Claude in Chrome, Playwright MCP, the Codex Browser, and headless CLIs such as agent-browser render pages, take screenshots, and click through flows inside the agent's loop. A second browser stack inside the skill duplicated that, reached into the user's running app, and cost a large share of the repository's code and test infrastructure (live E2E suites, framework fixtures, process reapers for leaked servers).

## Decision

Impeccino ships no browser stack and injects nothing into the user's running app. The agent takes screenshots with agent-browser or its harness's browser tool and asks decisions through the host's structured question tool. Where the engine needs a rendered page, it drives agent-browser ([0016](0016-rendered-pages-through-agent-browser.md)).

Removed: live mode (`live*` verbs, `detect-csp`, `crates/live`, the page scripts in `skill/scripts/`, the manual-edit-applier agent), `serve-question`, `component-review` (`ui/`), the Chrome discovery and CDP client (`crates/browser`), the capture service in `crates/cli`, and their tests and CI jobs.

## Consequences

- `critique` and the build's inspection rounds take screenshots with the agent's browser and run `detect <url>` next to them.
- Older skill copies that call a removed verb get a clear "was removed" message.
