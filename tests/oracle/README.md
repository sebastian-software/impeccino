# Oracle: behavior goldens for the engine

The oracle records observable engine behavior: stdout, stderr, exit status, and
selected workspace files. The goldens are the contract every runnable case
must match byte for byte.

- `record.mjs --bin` (with `$IMPECCINO_BIN` or `--bin=/path`) records from an
  engine binary. Record only new cases or intentional behavior changes, and
  review every resulting golden by hand.
- `run.mjs` replays cases against `$IMPECCINO_BIN` and reports differences.
- `tests/oracle.test.mjs` invokes the runner under the default test suite and
  skips when no binary is available. Set `IMPECCINO_BIN`, run
  `pnpm run fetch:engine`, or build with
  `cargo build --release -p impeccino`.
- `cases/*.mjs` define the corpus. `workspaces/` contains project fixtures
  copied to a temporary directory for each case, so writes stay isolated.
- A multi-step case shares one staged workspace; a step may include
  `setup(ws)` to prepare state before that invocation.

## Corpus files

- `cases/detect.mjs`: source scans, flags, stdin, project configuration,
  CLI help/version, and the retired `ignores` response.
- `cases/context.mjs`: context, doctor, pin, surface briefs, palette, signals,
  concept seeds, and retired critique-storage behavior.
- `cases/hooks.mjs`: hook, pre-edit hook, and hook-admin behavior, including
  state carried across invocations.
- `cases/skills.mjs`: retired installer and namespace commands.

## Normalizations

Path placeholders and ISO timestamps are normalized globally. Other masks are
limited to values that vary by machine or run:

- `IMAGE_TOOLS: <IMAGE_TOOLS_PROBE>`: the context command reports which image
  converters are installed on the recording host.
- `"<finding-id>": <EPOCH>`: the staleness cache stores epoch values by
  finding id.
- `projects/<PROJECT>/`: hook state uses a per-project cache directory based
  on the project path. Snapshot labels keep multiple project roots distinct
  and reject normalized path collisions. Windows expectations retain the
  native cache location and Claude exec form.
- Self-referential launcher commands normalize to `<IMPECCINO>` or
  `<HOOK_ADMIN_CMD>`.
- A case may define `normalize: [[regexSource, flags, replacement], ...]`
  for values that vary only in its own output.

Random palette/seed inputs are fixed per case. Real image generation,
interactive flows that open a browser or bind a port, and
unhandled-exception stack traces with source line numbers are outside this
corpus.

`vectors/` contains the frozen function-level data consumed by Rust parity
tests; see `vectors/README.md`. It is not an oracle recorder.
