# 0016: Rendered-page rules run through the harness's browser

**Status:** Accepted · **Date:** 2026-10-02

## Context

Nine detector rules need a rendered page because they read layout, not source: `line-length`, `text-overflow`, `text-occlusion`, `body-text-viewport-edge`, `first-viewport-column-overflow`, `edge-flush-cards`, `heading-rhythm`, `blinking-cursor`, and `script-error`. Several more rules gain accuracy from computed styles. They used to run in the live overlay, the extension, or URL scans over headless Chrome, all removed by 0011 and 0013. Dropping them would leave the layout failures that matter most to a human reader to screenshots alone.

The rule logic already ran natively over a serialized page snapshot (`SnapshotDom`), the route the Chrome extension used on strict-CSP sites: a small script measures the page once (DOM, computed styles, rects, viewport), and the engine evaluates the rules over the JSON, asking the page only for hit tests it cannot answer up front.

A snapshot of a real page is 0.5 to 3 MB (80 to 300 KB gzipped). Returning it through the agent's browser tool would put it into the model's context.

## Decision

`impeccable page-probe` keeps the rules without Impeccable driving a browser:

1. It starts a one-shot receiver on 127.0.0.1 (single-use token, time limit) and prints a one-line loader plus a key.
2. The agent evaluates the loader in the page with its own browser tool. The loader fetches the read-only measurement script (`crates/cli/assets/page-snapshot.js`), posts the snapshot to the receiver, and answers the hit-test rounds the rules ask for.
3. `page-probe --result <key>` prints the findings in `detect`'s format, after project ignores.

Only the findings reach the model. The skill runs the probe next to its screenshots (SKILL.md, Rendered-page detector; critique; new-work's inspection round).

## Consequences

- All 61 rules stay; the rendered-page rules see the page at the agent's viewport.
- Not ported yet: the image and canvas pixel pass of visual contrast, script errors (the probe arrives after the page loaded), and the reveal-on-scroll sweep for content hidden at rest.
- Chrome's Local Network Access blocks a public HTTPS page from calling 127.0.0.1 unless the user allows it; local dev pages and local files work. A page whose CSP forbids `eval` or `connect-src` to 127.0.0.1 needs `--inline` or cannot be probed.
- The receiver listens only on 127.0.0.1 and only for its token, for at most the time limit.
