# 0012: No image-generated comps

**Status:** Accepted · **Date:** 2026-10-02

## Context

The comp-first build path generated a full-fidelity mock with an image model, then tried to reproduce it in HTML and CSS: `generate-image`, `comp-spec` (a measured region map), plates cut from the comp, `font-match` against a 1 MB fingerprint index of Google Fonts, `comp-diff` heatmaps, a staged `build-phase` with a native capture service, a provenance scanner (`embed-prompt`), and an asset-producer agent. A `buildPath` setting chose between comp-first and code-first.

In practice the comps looked good and repeatedly did not survive the translation into a real layout. The fidelity machinery kept trying to close a gap that reopened with every build, and it was the heaviest part of the engine.

## Decision

Impeccino does not generate comps. The code-led build, carried by the direction contract (thesis, own world, first viewport, signature interaction), is the only build path. When a brief needs real imagery, the agent uses its harness's image tool directly; Impeccino has no image pipeline of its own.

Removed: the verbs above, `crates/comp` and `crates/comp-verbs`, the font index, the asset-producer agent, region maps and plates, decision comps, the `buildPath` setting and its directives, the image-tool probes in `context`, and the build-completion hook (including the Gemini hook manifest, which existed only for it).

## Consequences

- The finish reviewer judges the build against the direction contract and the screenshots; its dispositions are recapture, fix, and ship.
- Gemini CLI no longer gets a hook manifest; Claude Code, Codex, Cursor, Copilot, and Grok keep theirs.
- `buildPath` in an existing config is tolerated without a finding.
