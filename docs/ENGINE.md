# The engine: the Rust runtime behind every skill verb

The public interface is the skill and its agent workflows. The Rust engine, its CLI, and its crates are internal implementation details, not separately supported user APIs ([ADR 0005](adr/0005-no-marketplace-packages.md)).

Every command the skill text runs is `"<skill-base-dir>/scripts/impeccino" <verb>`. The
launcher next to the skill (`skill/scripts/impeccino`, `impeccino.cmd`)
finds or downloads one static binary per platform and execs it. That binary
is built from this repo's Cargo workspace. There is no Node at runtime, and
nothing in the engine runs in a browser
([ADR 0011](adr/0011-no-own-browser-stack.md)).

This page is the map for anyone building or changing the runtime. The
observable behavior of every verb is pinned byte-for-byte by the oracle
corpus in `tests/oracle/`, which is the internal integration contract between the engine, skill, launcher, and hooks
([ADR 0015](adr/0015-history-lives-in-git.md)).

Everything is in this repo, Apache-2.0, and builds offline from source. No
part of the engine is fetched at build time, and the engine is one native
binary: there is no WebAssembly build, no in-page bundle, and no browser
extension ([ADR 0013](adr/0013-no-wasm-or-browser-extension.md)).

## Internal interface policy

Keep engine verbs and flags when the skill, launchers, hooks, or repository tooling need them. Their arguments, output streams, exit codes, and side effects must remain reliable for those callers. Public skill commands and internal engine verbs are different surfaces: `/impeccino audit` is a user workflow; `detect` is an implementation step in that workflow.

Changes to the internal interface require updating its callers and regression coverage together. A pinned engine and skill can be released in sequence; installed older skill copies still need their pinned engine. Marking this interface internal does not itself remove existing verbs or bypass that release discipline. There is no separate standalone CLI product or downstream Rust API stability commitment.

## Current internal callers

The skill references call `context`, `doctor`, `pin`, `signals`, `concept-seed`, `surface-brief`, `detect`, and `hooks`. Installed hook manifests call `hook` and `hook-before-edit`; the launchers use the hidden `engine-probe` handshake to validate the pinned engine. `palette` remains an internal engine verb; no direct caller appears in the current skill text. `critique-storage` and `ignores` now return their documented removal messages because critiques live in chat and projects have no config file (ADR 0020).

The aliases `context-signals`, `hook-admin`, root help, version flags, and implicit detector target syntax remain internal engine behavior. Check their callers, repository tests, and engine-produced commands before removing any of them.

## Internal detector invocation

These calls are for engine development, tests, and debugging skill or hook integration. They are not a separate user workflow. Agents invoke the installed skill's quoted launcher; contributors can set `IMPECCINO_BIN` to a local build:

```bash
"<skill-base-dir>/scripts/impeccino" detect src/          # scan a directory
"<skill-base-dir>/scripts/impeccino" detect --json .      # inspect structured output
"<skill-base-dir>/scripts/impeccino" detect --no-config src/ # scan without design decisions
```

The detector catches 61 deterministic issues across AI slop (side-tab borders, purple gradients, bounce easing, dark glows) and general design quality (low contrast, cramped padding, tiny text, skipped headings, and more). `detect` reads files, directories, and URLs. For a URL (`http`, `https`, or `file`), it loads the page headlessly through [agent-browser](https://github.com/vercel-labs/agent-browser) and adds the rules that need layout (line length, text overflow and occlusion, viewport edges, heading rhythm), rendered contrast including text over images, and script errors: `impeccino detect --viewport 390x844 http://localhost:3000/`. Set `AGENT_BROWSER_SESSION` to scan in a session that is already signed in. Rendered scans need agent-browser installed (`npm install -g agent-browser && agent-browser install`); source scans do not.

Human-readable findings are diagnostics written to stderr, so redirect them with `2> findings.txt`. Use `--json` for machine-readable results on stdout. Exit `0` means the scan completed without primary findings, exit `2` means it completed with primary findings, and exit `1` means the scan was aborted or at least one requested target could not be scanned; operational failure takes precedence for a partial multi-target scan. A clean detector run is evidence, not proof of visual or accessibility quality: it does not replace inspecting the rendered experience across relevant viewports.

`detect` has no project config file. It applies DESIGN.md waivers and declared design values, skips files excluded by the project's Git ignore rules, and honors in-file waiver comments. `--no-config` skips DESIGN.md decisions and in-file waivers; `--no-inline-ignores` skips only the in-file comments. A project-wide waiver belongs in DESIGN.md; a local waiver belongs in a comment next to the code.

Rendered-page scanning is described above and in [ADR 0016](adr/0016-rendered-pages-through-agent-browser.md).

## Layout

```
Cargo.toml              the workspace (crates/*), release profile
rust-toolchain.toml     the toolchain channel (stable)
skill/scripts/VERSION   which engine release the launcher downloads
crates/
  cli          the `impeccino` binary: verb router, exit codes, the
               "was removed" answer for retired verbs
  common       Io handle, project/workspace paths, scan extensions and
               generated-path scope, path + process helpers
  context      context, doctor, staleness, signals, concept-seed, pin,
               palette, surface-brief (SURFACES.md)
  hook         the design hook (hook, hook-before-edit, hooks / hook-admin)
  detect       `impeccino detect`: file walk (with the shared scan scope and
               the project's Git ignore rules), DESIGN.md decisions, output,
               the text/regex engine
  html         the static HTML engine: parser, cascade, static document model,
               rule adapters
  foundation   JS-semantics helpers, color, findings, the rule registry,
               inline ignores, CSS measures, and the plain-data types every
               check takes in and hands back
  core         the rule logic: every `check_*` / `scan_*` and its heuristics
```

The verbs: `context`, `doctor`, `pin`, `surface-brief`, `palette`, `signals`
(alias `context-signals`), `concept-seed`, `detect` (files, directories, and
URLs through agent-browser), `hook`, `hook-before-edit`, and `hooks` (alias
`hook-admin`). The browser and comp verbs (`live*`, `detect-csp`,
`serve-question`, `component-review`, `generate-image`, `comp-spec`,
`comp-diff`, `font-match`, `build-phase`, `capture-server`, `embed-prompt`),
`critique-storage`, and `ignores` print a "was removed" message and exit 1,
so an older skill copy that calls one gets a clear answer
([ADR 0011](adr/0011-no-own-browser-stack.md),
[ADR 0012](adr/0012-no-image-comps.md),
[ADR 0020](adr/0020-project-state-is-top-level-files.md)).

No verb reads a config file. Project state is `SURFACES.md` and `DESIGN.json`
beside DESIGN.md; `detect` and the hook take their waivers from DESIGN.md
(`crates/detect/src/design_decisions.rs`) and skip what the project's
`.gitignore` and `.gitattributes` exclude (`project_ignores.rs`); the hook's
session cache and the boot's staleness throttle live in the per-user cache
(`impeccino_common::project_files::user_cache_dir`).
Hook file state stays in the cache for its owning project. When a session's
working directory differs from that project, the session-cwd cache also keeps
a bounded `projectRoots` index so Stop can find the project's cache without
searching the full user cache. Stop treats those roots only as lookup hints:
it rechecks project containment and file safety before reading touched files.
Concurrent index updates use a sibling advisory lock around the atomic cache
replacement. Stop's 20-file budget caps files actually passed to the detector
across the project fan-out; it does not cap candidate-index traversal.

`crates/core` re-exports the foundation modules under its own paths, so every
consumer names one crate: `impeccino_core::js`, `impeccino_core::color`,
`impeccino_core::checks::rules::check_colors`. The split between the two
crates separates shared data and helpers from rule logic within the internal
runtime.

Static and rendered DOM adapters feed shared computed-style and text facts
into `core::checks::{quality,rules,text_rules}`. These predicates own
layout-independent quality findings, font usage, cream palettes, heading
order, repeated container text, and tab/status contexts. Adapter parity tests
in `crates/html/src/static_engine.rs` assert identical findings, including
hidden-text exemptions and UTF-16 snippet boundaries. Geometry, live pixels,
and the browser's full CSS implementation remain rendered-page inputs.

The detector ships 61 built-in rules, listed in
`crates/foundation/src/registry.rs`; `pnpm run check` reads the count from
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

`pnpm run test` and the oracle find the binary through `IMPECCINO_BIN`, then
`skill/scripts/bin/<os>-<arch>/` (`pnpm run fetch:engine` downloads the pinned
release there; `IMPECCINO_BIN=target/release/impeccino pnpm run fetch:engine`
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

1. **Engine** (`engine-v<version>`): `pnpm run release:engine` verifies
   the version and a clean tree, then tags and pushes;
   `.github/workflows/release-engine.yml` builds the five targets, attests
   each binary, generates `THIRD-PARTY-NOTICES.txt` from the locked Cargo
   dependency union for all five targets, and publishes them as an immutable
   release.
2. **Pins**: `scripts/pin-engine.mjs` verifies each binary's build
   attestation and writes the digests to `skill/scripts/engine.sha256`, which
   the launchers check downloads against.
3. **Skill** (`skill-v<version>`), gated on the engine release
   (`scripts/check-engine-release.mjs`) and on the pins.

CI runs the workspace build and tests (`rust`, `rust-windows`) and replays the
oracle against a release build from the checkout under test.

## Bounded scan work

Directory detection loads each primary source once, passes it to the text or static HTML engine while it is in memory, and retains only import metadata and findings. Import resolution uses a shared hash set of candidate paths. Linked stylesheets are still resolved relative to the HTML file.

Context's visual evidence probe reads at most 256 KiB from a candidate before applying its existing 64 Ki UTF-16 unit window; doctor, signals and concept-seed skip that probe because they do not report visual evidence. Globstar workspace discovery is capped at 32 directory levels and 4,096 visited directories. Target discovery already paid for at boot remains reusable by diagnostics.

Stop retains its 20-file invocation limit and rotates large file sets and project order between invocations. Cursor's repeated-denial signature is the set of rule IDs in one file, independent of line shifts or duplicate occurrences. Proposal scans create a new directory atomically and never reuse a directory owned by another proposal.

Display snippets end at whole Unicode scalars within their UTF-16 limit. CSS color labels clamp and round channels to bytes; the inherited numeric `color_to_hex` helper remains available for frozen compatibility vectors. Invalid SVG dimensions do not produce a sized-scene finding. `.agents/skills/impeccino` defaults to Codex identity; an explicit provider override still takes precedence.

## Production finding policy

Raw matcher outputs and frozen function vectors preserve their compatibility
contract. CLI reports and hooks apply `apply_reporting_policy` from foundation:
measured broken images, script errors, contrast, occlusion, and overflow remain
primary; built-in pattern, declared-value drift, and threshold/risk signals are
advisory. External packs own their registered finding policy. This classification
does not expand accessibility coverage or establish deeper conformance.

Advisories remain visible in explicit reports and hook context, sort after
primary hook findings within each file, do not deny Cursor writes, and do not produce finding
exit status 2 alone. Codex Stop omits advisory-only context because its schema
supports a blocking decision only. `--no-advisory`, operational failures,
waivers, inline ignores, native exclusions, and target ownership still apply.
See [Design rule curation](design-rule-curation.md) for the primary rule IDs,
knowledge ownership, creative-tool limits, and retained native sources.
