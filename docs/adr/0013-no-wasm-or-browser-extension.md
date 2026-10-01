# 0013: No WebAssembly build and no browser extension

**Status:** Accepted · **Date:** 2026-10-02

## Context

The rule engine was compiled to WebAssembly (`crates/wasm`, `crates/bundle`, `crates/xtask`, `browser-bundle/`) for two consumers: the Chrome/Firefox extension and the live-mode overlay. The extension was never used by the owner of this branch, and the overlay left with live mode (0011).

## Decision

Remove the WebAssembly build, the in-page bundle, and the browser extension, together with the DOM layers in `crates/core` and `crates/foundation` that only they used and the DOM hooks of the `RulePack` trait.

## Consequences

- The engine is one native binary; `cargo build` needs no wasm toolchain and no tracked generated bundle.
- Downstream rule packs keep the text and static HTML hooks.
- This supersedes the note in 0005 that the extension was unaffected.
