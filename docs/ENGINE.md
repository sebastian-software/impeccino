# The engine: the Rust runtime behind every skill verb

Every command the skill text runs is `<skill-base-dir>/scripts/impeccino <verb>`. The
launcher next to the skill (`skill/scripts/impeccino`, `impeccino.cmd`)
finds or downloads one static binary per platform and execs it. That binary
is built from this repo's Cargo workspace. There is no Node at runtime, and
nothing in the engine runs in a browser
([ADR 0011](adr/0011-no-own-browser-stack.md)).

This page is the map for anyone building or changing the runtime. The
observable behavior of every verb is pinned byte-for-byte by the oracle
corpus in `tests/oracle/`, which is the engine's behavioral contract
([ADR 0015](adr/0015-history-lives-in-git.md)).

Everything is in this repo, Apache-2.0, and builds offline from source. No
part of the engine is fetched at build time, and the engine is one native
binary: there is no WebAssembly build, no in-page bundle, and no browser
extension ([ADR 0013](adr/0013-no-wasm-or-browser-extension.md)).

## Layout

```
Cargo.toml              the workspace (crates/*), release profile
rust-toolchain.toml     the toolchain channel (stable)
skill/scripts/VERSION   which engine release the launcher downloads
crates/
  cli          the `impeccino` binary: verb router, exit codes, the
               "was removed" answer for retired verbs
  common       Io handle (stdout/stderr/stdin/env/cwd), path + process helpers
  context      context, doctor, staleness, signals, concept-seed, pin,
               palette, surface-brief, critique-storage
  hook         the design hook (hook, hook-before-edit, hooks / hook-admin)
  detect       `impeccino detect` and `ignores`: file walk, config, ignores,
               output, the text/regex engine
  html         the static HTML engine: parser, cascade, static document model,
               rule adapters
  foundation   JS-semantics helpers, color, findings, the rule registry,
               inline ignores, CSS measures, and the plain-data types every
               check takes in and hands back
  core         the rule logic: every `check_*` / `scan_*` and its heuristics
```

The verbs: `context`, `doctor`, `pin`, `surface-brief`, `critique-storage`,
`palette`, `signals` (alias `context-signals`), `concept-seed`, `detect`
(files and directories only), `ignores`, `hook`, `hook-before-edit`, and
`hooks` (alias `hook-admin`). The browser and comp verbs (`live*`,
`detect-csp`, `serve-question`, `component-review`, `generate-image`,
`comp-spec`, `comp-diff`, `font-match`, `build-phase`, `capture-server`,
`embed-prompt`) print a "was removed" message and exit 1, so an older skill
copy that calls one gets a clear answer
([ADR 0011](adr/0011-no-own-browser-stack.md),
[ADR 0012](adr/0012-no-image-comps.md)). `detect` refuses URLs with a
pointer to the harness's browser tool.

`crates/core` re-exports the foundation modules under its own paths, so every
consumer names one crate: `impeccino_core::js`, `impeccino_core::color`,
`impeccino_core::checks::rules::check_colors`. The split between the two
crates is about what a check is written against, not about who may see it.

The detector ships 61 built-in rules, listed in
`crates/foundation/src/registry.rs`; `bun run check` reads the count from
there. Most run on source files, through the text engine (`crates/detect`)
or the static HTML engine (`crates/html`). The rules that need layout run in
`crates/core/src/browser` over a page snapshot (`SnapshotDom` in
`crates/foundation/src/browser`): `impeccino detect <url>` drives
agent-browser (`crates/cli/src/page_scan`), installs the measurement script
`crates/cli/assets/page-snapshot.js` in the page, answers the rules' hit
tests with the live page over agent-browser's DevTools endpoint, and falls
back to screenshot pixels for contrast it cannot decide (docs/adr/0016).

Build and test:

```bash
cargo build --release -p impeccino      # target/release/impeccino
cargo test --workspace
IMPECCINO_BIN=target/release/impeccino node tests/oracle/run.mjs   # the behavior gate
```

`bun run test` and the oracle find the binary through `IMPECCINO_BIN`, then
`skill/scripts/bin/<os>-<arch>/` (`bun run fetch:engine` downloads the pinned
release there; `IMPECCINO_BIN=target/release/impeccino bun run fetch:engine`
copies a local build), then `target/release/impeccino`, so a plain
`cargo build --release -p impeccino` is enough.

The frozen function-level vectors in `tests/oracle/vectors/calls/` replay
through `impeccino_core::vectors::call` (`cargo test -p impeccino-core`),
which is the union of foundation's dispatch arms and the core's.

## Rule packs

The built-in rules are compiled in and always run. A **rule pack** is how a
crate that depends on this workspace adds rules of its own without forking it:
one process-lifetime value carrying its own registry rows plus the hooks it
has rules for. With no pack installed nothing changes, which the oracle
enforces byte-for-byte.

The traits:

- `impeccino_core::rule_pack::RulePack` (object-safe, `Send + Sync + Debug`)
  with one hook, defaulting to empty: `check_text(content, file_path, ext)`
  for the text engine.
- `impeccino_html::StaticRulePack` with `check_document(doc, file_path)`.
  The `StaticDocument` model belongs to `crates/html`, and `detect` cannot
  name a type from a crate that depends on it, so the static engine's hook is
  a separate trait. A pack that covers HTML implements both.

The DOM hooks for the browser engines left with them
([ADR 0013](adr/0013-no-wasm-or-browser-extension.md)).

Three steps for the downstream crate: declare `static ROWS: &[Antipattern]`
with namespaced ids (`mypack/my-rule`) and return them from `registry()`;
call `impeccino_core::rule_pack::install(&PACK)` once at startup, which is
what makes `get_antipattern` resolve the pack's ids and therefore what gives
its findings a name, description, category, and severity; then pass the pack
to the engine being run.

Where a pack reference travels:

| Engine | Field |
|---|---|
| text | `TextOptions.rule_pack`, `ScanOptions.rule_pack` |
| static HTML | `DetectHtmlOptions.static_rule_pack` and `.rule_pack`; `StaticHtmlEngine.static_rule_pack` for the `Engines` seam |

Where each hook runs, and why there:

- **Text engine** (`detect_text`): after every built-in matcher, style-block
  and CSS-in-JS pass, the design-system scan, the dedupe, and the page
  analyzers, and before inline ignores. Appending last keeps built-in output
  identical, and being inside the waiver step means `impeccino-disable`
  covers a pack's rules the same way it covers built-in ones.
- **Static HTML engine** (`detect_html_source`): after the element rules, the
  design-system merge, the page-level checks and the pattern checks, again
  just before inline ignores. An HTML file gets exactly one pack pass:
  `static_rule_pack` when it is set, otherwise `rule_pack.check_text` over
  the raw HTML source, which is how a text-only pack still covers `.html`
  files. A pack that implements both never reports the same file twice.

The registry keeps `ANTIPATTERNS` as the built-in list and consults the
registered rows after it (`registry::extend`, `registry::all_antipatterns`).
`extend` is idempotent per slice and panics on an id collision, so a pack can
never shadow a built-in rule. Registration is append-only and has no undo:
a pack is a property of the process, not of a run.

The design hook's immediate tier (the rule ids worth fixing at the edit site)
lives in `impeccino_core::registry::IMMEDIATE_TIER_RULES`, which
`impeccino-hook` reads, so there is one list.

## Releases

Releases are tags with notes GitHub generates from the commits
([ADR 0014](adr/0014-releases-are-tags.md)). Two release kinds touch the
runtime, in this order:

1. **Engine** (`engine-v<version>`): `bun run release:engine` verifies
   the version and a clean tree, then tags and pushes;
   `.github/workflows/release-engine.yml` builds the five targets and
   publishes the binaries with `.sha256` sidecars, which the launcher
   downloads and verifies.
2. **Skill** (`skill-v<version>`), which `scripts/check-engine-release.mjs`
   gates on the engine release.

CI runs the workspace build and tests (`rust`, `rust-windows`) and replays the
oracle against a release build from the checkout under test.
