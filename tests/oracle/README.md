# Oracle: behavior goldens for every `impeccino` verb

`lib.mjs` runs each case (verb + args + staged workspace + stdin) against an
implementation and captures stdout, stderr, exit code, and named files, with
machine-specific paths and timestamps normalized.

- The goldens are the behavior contract the engine binary is held to. Record
  them from the current source binary and review every changed golden by hand.
- `record.mjs --bin` (with `$IMPECCINO_BIN` or `--bin=/path`) writes goldens
  from the binary, for new cases or a delta a review accepted. Plain
  `record.mjs` still targets the JS scripts and only works on a checkout that
  has them (history before the swap).
- `run.mjs` replays the corpus against `$IMPECCINO_BIN` (or `--js` for a
  self-check on a pre-swap checkout) and diffs. Every runnable case must match
  its golden byte for byte; a behavior change requires a reviewed golden edit.
- `tests/oracle.test.mjs` runs `run.mjs` under `pnpm run test` and skips when
  no binary is found (`IMPECCINO_BIN` or `skill/scripts/bin/<os>-<arch>/`,
  filled by `pnpm run fetch:engine`).
- `cases/*.mjs` define the corpus (default export: array or async function
  returning an array). `workspaces/` holds project fixtures that are copied to
  a temp dir per run, so cases can write freely.

Adding a case: append to the matching `cases/*.mjs`, run
`node tests/oracle/record.mjs --bin <prefix>`, review the golden by hand
(the binary is now the recorder, so a bug in it would be frozen too), commit
the golden.

Verb names are the binary's subcommands. `cli-help` and `cli-version` map to
`impeccino --help` / `--version`. `lib.mjs` still carries the `JS_VERBS`
table that maps each verb to the script it was recorded from.

`vectors/` holds the function-level vectors recorded from the JS engine's pure
functions; see `vectors/README.md`.

## Corpus files

- `cases/detect.mjs`: `detect`, `cli-help`, `cli-version`, and the removed `ignores`.
- `cases/hooks.mjs`: `hook`, `hook-before-edit`, `hook-admin`.
- `cases/context.mjs`: `context`, `doctor`, `pin`, `surface-brief`, the
  removed `critique-storage`, `palette`, `embed-prompt`, `context-signals`
  (id prefix `signals-`), `detect-csp` (`csp-`), `concept-seed` (`seed-`),
  `generate-image` (`genimg-`), `serve-question` (`question-`). Only offline
  paths: fake image generation and serve-question modes that never open a
  browser or listen.
  Workspaces are `workspaces/ctx-*`; the header comment in the case file
  describes each one. Machine-specific env (`OPENAI_API_KEY`, context
  overrides, `CI`) is pinned per case so the recording host does not
  leak into goldens.

## Normalizations

Beyond paths and ISO timestamps, `normalize()` masks these run- or
machine-dependent fragments. Each is targeted at one script's output:

- `IMAGE_TOOLS: <IMAGE_TOOLS_PROBE>`: `context` probes `which cwebp sips
  magick ffmpeg`; the set found describes the machine, not the script.
- `"<finding-id>": <EPOCH>`: the staleness notice cache
  (`<user cache>/impeccino/staleness-check.json`) keys epoch stamps by finding id.
- `projects/<PROJECT>/`: the hook keeps its session state in a per-project
  directory of the user cache, named after the project's absolute path plus an
  8-hex digest (docs/adr/0020). The mask applies to snapshot file names too.
  The harness points the user cache at the isolated home: it unsets
  `XDG_CACHE_HOME` and `LOCALAPPDATA`, so state lands under
  `.oracle-home/.cache/impeccino/` on POSIX and
  `.oracle-home/AppData/Local/impeccino/` on Windows. Windows expectations
  retain that native location for snapshots and displayed cache paths.
- `<IMPECCINO> <verb>` / `<HOOK_ADMIN_CMD>`: self-referential command lines.

Not covered on purpose: `palette` with no `--id` / `--from` / env seed (random),
`concept-seed` with no `--from` / env seed (random), `generate-image` real mode,
`serve-question --start` / blocking mode (opens a browser and binds a port),
and unhandled-exception paths whose stack traces carry Node line numbers.

## Live-mode cases (`cases/live-*.mjs`, workspaces `live-*`)

Helpers live in `live-helpers.mjs` (staged journals, buffers, wrapped source
files with the fake-agent variant block, a `.git` FILE pointing at a non-repo
gitdir so roots resolution sees a git boundary while `git check-ignore` exits
128 everywhere and the ignore block lands in the snapshotable
`.gitfake/info/exclude`). Svelte component preview cases symlink this repo's
`node_modules/svelte` into the staged app, exactly like the unit tests.

Harness additions made for live:

- `steps[]` entries may carry their own `setup(ws)` (run right before that
  step) and `daemon: true` with `readyFile` / `readyTimeoutMs`: the verb is
  spawned detached, the harness waits for the ready file, later steps run
  against it, and teardown SIGTERMs (then SIGKILLs) it. Its stdout/stderr land
  in the golden as `daemon: [{stdout, stderr}]`.
- `normalize: [[regexSource, flags, replacement], ...]` on a case applies extra
  masks to that case only. Live uses it for the dynamic helper port
  (`localhost:<PORT>`, `"port": <PORT>`), lease and phase stamps (`<EPOCH>`),
  and float durations (`<N>`).
- Global masks added: `"pid": <PID>` / `(pid <PID>)` and UUID tokens `<UUID>`.
- `snapshotFiles` walks `node_modules/.impeccino-live` (the Svelte preview
  tree) and nothing else under `node_modules`.

Deliberately not covered here (rely on `tests/live-e2e`): the browser
handshake and `/live.js` bundle, SSE, generate/accept round-trips through a
real browser, `variant_mount_failed` republish, manual-edit chat routing and
the codex/claude subprocess providers, Svelte revision-dir publishing, and
`live.mjs`'s dev-server-dependent flows. Lock-file names hash the absolute
source path, so lock cases do not snapshot `.impeccino/live/locks/`.
`live-poll-*-connection-refused` assumes nothing listens on 127.0.0.1:65531.
