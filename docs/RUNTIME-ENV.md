# Runtime environment the binary reads (context crate)

The JS scripts learned two things from their own file location that a single
binary cannot: which harness built them (`lib/provider.mjs`) and where the
skill's `reference/` and `SKILL.md` live (`../reference`, `../SKILL.md`).
`crates/context/src/provider.rs` resolves both at run time.

| Variable | Meaning | Default |
|---|---|---|
| `IMPECCINO_SKILL_DIR` | The skill directory (holds `SKILL.md`, `reference/`, `scripts/`). Used for the native platform references `context` inlines and `pin`'s `command-metadata.json`. | Walk up from the executable's directory until a directory containing `reference/ios.md` is found (the binary ships at `<skill>/scripts/bin/<os>-<arch>/`). None if nothing matches: native refs are then skipped silently, like a missing file in the JS. |
| `IMPECCINO_PROVIDER_ID` | The provider id (`claude-code`, `codex`, `cursor`, ...). Selects the hook manifest paths `context` and `doctor` inspect and the `$`/`/` command prefix (`$` only for `codex`). | Derived from the skill dir's harness folder (`<root>/.codex/skills/impeccino` -> `codex`); otherwise `source`, which is what the JS reads in a source checkout. |
| `IMPECCINO_SELF` | How to spell this binary in printed commands where the JS printed `node <scripts>/<script>.mjs` (`context`'s MANUAL_DETECTOR_REQUIRED, SURFACE_CONTEXT_AVAILABLE, and MONOREPO_TARGET_REQUIRED directives, `doctor`'s fix lines). Printed as `<self> <verb>`. | The executable path (`doctor` falls back to plain `impeccino`). |

The launcher (`skill/scripts/impeccino`) exports `IMPECCINO_SKILL_DIR`
(its parent directory) and `IMPECCINO_SELF` (`$0`) unless they are already
set, so directives name the launcher instead of the platform binary.

Everything else is the engine's own environment (`IMPECCINO_CONTEXT_DIR`,
`IMPECCINO_STALENESS_CACHE`, `IMPECCINO_NO_STALENESS_CHECK`,
`IMPECCINO_CONCEPT_SEED`, ...), read unchanged from the JS contract. The
update check, the image generation probes, and the concept catalog service
are gone ([ADR 0004](adr/0004-no-update-check.md),
[ADR 0012](adr/0012-no-image-comps.md),
[ADR 0019](adr/0019-concept-seed-is-local.md)), and so are the variables they
read.

Oracle note: run `tests/oracle/run.mjs` with
`IMPECCINO_SKILL_DIR=<repo>/skill` (the corpus expects the native
references inlined) and an `IMPECCINO_BIN` path outside `$HOME` (the
harness normalizes `$HOME` before it normalizes the bin path, so a binary
under the home dir renders as `<HOME>/...` instead of `<IMPECCINO>`).
