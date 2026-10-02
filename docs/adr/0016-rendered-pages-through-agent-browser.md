# 0016: Rendered pages are measured through agent-browser

**Status:** Accepted · **Date:** 2026-10-02

## Context

Nine detector rules need a rendered page because they read layout, not source: `line-length`, `text-overflow`, `text-occlusion`, `body-text-viewport-edge`, `first-viewport-column-overflow`, `edge-flush-cards`, `heading-rhythm`, `blinking-cursor`, and `script-error`. More rules gain accuracy from computed styles, and the visual contrast pass needs the page for text over images. They used to run in the live overlay, the browser extension, or URL scans over Impeccable's own Chrome connection, all removed by 0011 and 0013.

The rule logic already runs natively over a serialized page snapshot (`SnapshotDom`): a small script measures the page once (DOM, computed styles, rects, viewport), and the engine evaluates the rules over the JSON, asking the page only for what it cannot answer up front (hit tests, pixels). A snapshot of a real page is 0.5 to 3 MB, so it must not pass through the model's context.

[agent-browser](https://github.com/vercel-labs/agent-browser) is a headless browser CLI that agents already use for screenshots, scrolling, and evaluation, from any harness with a shell.

## Decision

`impeccable detect <url>` scans the rendered page through agent-browser (`crates/cli/src/page_scan`, the `UrlEngine` slot of `detect`):

- agent-browser opens the URL in a session of its own (closed afterwards), or in `AGENT_BROWSER_SESSION`'s when that is set, for example a session that is already signed in.
- The measurement script (`crates/cli/assets/page-snapshot.js`) and a small set of page operations are installed with `eval`. The scan asks the page one operation at a time over the DevTools endpoint agent-browser exposes (`get cdp-url`), with `agent-browser eval` as the fallback; every agent-browser command costs a fixed ~170 ms round trip through its daemon, and a scan asks a few hundred questions.
- The scan runs the former URL engine's passes: the browser rules with live hit tests, the content-hidden-at-rest reveal sweep, script errors (agent-browser's error log from the page's first script on, plus dev-server error overlays), the visual contrast pass (image-backed text first, then other candidates, 12 each), and the screenshot pixel fallback for candidates the analyses cannot decide (filters, blend modes, opacity stacks, cross-origin images). With the rendered pixel as foreground, the pixel pass measures glyph cores, since anti-aliased edges would flag dark text.

Considered and dropped: a loader the harness's own browser tool evaluates, calling back to a one-shot receiver on 127.0.0.1. It worked in any harness with JavaScript evaluation, but needed a detached receiver, a token, CORS and Private Network headers, and a two-step command choreography; it could not take screenshots or see errors from before it ran; and Chrome's Local Network Access blocked public HTTPS pages.

## Consequences

- One command: `detect --viewport 390x844 http://localhost:3000/`. Project ignores, `--json`, exit codes, and multi-URL runs work as for files; multi-URL runs share one session.
- All 61 rules stay, at the viewport the scan is given.
- agent-browser (and the Chromium it installs) is a dependency for rendered scans only. SKILL.md's `compatibility` names it, `context` says at session start when it is missing, and `detect <url>` says how to install it. Source scans are unaffected.
- A typical page takes 5 to 11 seconds, about 4 of them for the browser start and the network-idle wait.
- The scan sees the URL in its own headless page, not the agent's in-app browser tab.

## Revisit when

agent-browser stops exposing a DevTools endpoint, or its command round trip gets fast enough that `eval` alone suffices.
