# 0018: Rendered pages are measured through agent-browser

**Status:** Accepted · **Date:** 2026-10-02 · Supersedes the receiver in [0016](0016-rendered-page-rules-via-the-harness-browser.md)

## Context

0016 kept the rendered-page rules by letting the agent's own browser tool run a loader that called back to a receiver on 127.0.0.1. That worked in any harness with JavaScript evaluation, but it carried a lot of machinery: a detached receiver process, a token, a state directory, CORS and Private Network headers, a two-step `page-probe` / `page-probe --result` flow the agent had to choreograph. It also left two gaps. Undecidable contrast candidates had no screenshot pixels, and errors thrown before the loader ran never reached the scan. Chrome's Local Network Access blocked public HTTPS pages from calling the receiver.

[agent-browser](https://github.com/vercel-labs/agent-browser) is a headless browser CLI agents already use for screenshots, scrolling, and evaluation. Any harness with a shell can run it.

## Decision

`impeccable detect <url>` scans the rendered page through agent-browser. The former URL engine's slot in `detect` (`UrlEngine`) is filled again, now by `crates/cli/src/page_scan`:

- agent-browser opens the URL in a session of its own (closed afterwards), or in `AGENT_BROWSER_SESSION`'s when that is set (for example, a session that is already signed in).
- The measurement script and the page operations are installed with `eval`. The scan asks the page for one operation at a time over the DevTools endpoint that agent-browser exposes (`get cdp-url`), with `agent-browser eval` as the fallback. Every agent-browser command costs a fixed ~170 ms round trip through its daemon, and a scan asks a few hundred questions.
- Page errors come from agent-browser's error log, which starts with the page's first script.
- Contrast candidates the analyses cannot decide (filters, blend modes, opacity stacks, cross-origin images) go to the screenshot pixel fallback: screenshot with and without the text, compare glyph pixels. With the rendered pixel as foreground, it measures glyph cores instead of anti-aliased edges, which would otherwise flag dark text.

Removed: `page-probe`, its receiver, loader, token, and state directory.

## Consequences

- One command instead of a choreography: `detect --viewport 390x844 http://localhost:3000/`. Project ignores, `--json`, exit codes, and multi-URL runs work as for files; multi-URL runs share one session.
- The two remaining gaps of 0016 are closed. Every former URL-engine pass runs again.
- agent-browser (and the Chromium it installs) becomes a dependency for rendered scans only. Without it, `detect <url>` says how to install it; source scans are unaffected. A skill manager profile can list it next to the skill.
- Public HTTPS pages work; there is no loopback call.
- A typical fixture page takes 5 to 11 seconds, about 4 of them for the browser start and the network-idle wait.
- The scan does not see the agent's in-app browser. It sees the same URL in its own headless page, at the viewport it is given.

## Revisit when

agent-browser stops exposing a DevTools endpoint, or its command round trip drops far enough that `eval` alone is fast.
