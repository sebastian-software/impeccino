# 0016: Rendered-page rules run through the harness's browser

**Status:** Superseded by [0018](0018-rendered-pages-through-agent-browser.md) · **Date:** 2026-10-02

> Amended 2026-10-02: the receiver, loader, and `page-probe` verb are gone. The scan and its passes stay; agent-browser now holds the page, which also closed the two gaps listed below (0018).

## Context

Nine detector rules need a rendered page because they read layout, not source: `line-length`, `text-overflow`, `text-occlusion`, `body-text-viewport-edge`, `first-viewport-column-overflow`, `edge-flush-cards`, `heading-rhythm`, `blinking-cursor`, and `script-error`. Several more rules gain accuracy from computed styles. They used to run in the live overlay, the extension, or URL scans over headless Chrome, all removed by 0011 and 0013. Dropping them would leave the layout failures that matter most to a human reader to screenshots alone.

The rule logic already ran natively over a serialized page snapshot (`SnapshotDom`), the route the Chrome extension used on strict-CSP sites: a small script measures the page once (DOM, computed styles, rects, viewport), and the engine evaluates the rules over the JSON, asking the page only for hit tests it cannot answer up front.

A snapshot of a real page is 0.5 to 3 MB (80 to 300 KB gzipped). Returning it through the agent's browser tool would put it into the model's context.

## Decision

`impeccable page-probe` keeps the rules without Impeccable driving a browser:

1. It starts a one-shot receiver on 127.0.0.1 (single-use token, time limit) and prints a one-line loader plus a key.
2. The agent evaluates the loader in the page with its own browser tool. The loader fetches the read-only measurement script (`crates/cli/assets/page-snapshot.js`) and a command loop. The scan runs in the engine and asks the page for one operation at a time: snapshot captures, hit tests, a reveal-on-scroll sweep, script errors, image loads, and canvas pixel reads for text over images. These are the URL engine's former passes with the DevTools connection replaced by the loop.
3. `page-probe --result <key>` prints the findings in `detect`'s format, after project ignores.

Only the findings reach the model. The skill runs the probe next to its screenshots (SKILL.md, Rendered-page detector; critique; new-work's inspection round).

## Consequences

- All 61 rules stay; the rendered-page rules see the page at the agent's viewport.
- Visual contrast analyzes image-backed text first and then other candidates (12 each), so gradient-heavy pages cannot crowd images out.
- Script errors cover errors thrown while the probe runs (including during its reveal sweep) and the error overlays of Vite, Next.js, and webpack dev servers. Errors thrown earlier only show in the browser tool's console.
- Not ported: the screenshot pixel fallback for candidates the canvas pass cannot decide, such as cross-origin images without CORS headers. Those candidates stay unreported.
- Chrome's Local Network Access blocks a public HTTPS page from calling 127.0.0.1 unless the user allows it; local dev pages and local files work. A page whose CSP forbids `eval` or `connect-src` to 127.0.0.1 needs `--inline` or cannot be probed.
- The receiver listens only on 127.0.0.1 and only for its token, for at most the time limit.
