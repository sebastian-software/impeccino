# Runtime and test environment

The launcher passes its environment to the native engine. This reference lists
environment names that the engine, launchers, hooks, and repository tooling
read explicitly. Test controls and optional provider keys are separate from
settings used by people running the skill.

## Launcher and engine

Source: `skill/scripts/impeccino`, `skill/scripts/impeccino.cmd`, `crates/common/src`, and `crates/context/src`.

| Variable | Reader and effect | Default or fallback |
|---|---|---|
| `IMPECCINO_BIN` | Launcher override for an engine executable. | Unset: use a matching sibling or cached engine, then download the pinned release. This explicit developer override is not version-checked by the launcher. |
| `IMPECCINO_HOME` | Launcher cache root for downloaded engines. | `$HOME/.impeccino` on POSIX; `%USERPROFILE%\.impeccino` on Windows. |
| `IMPECCINO_DOWNLOAD_BASE` | Launcher and release tooling base URL for engine assets. | `https://github.com/sebastian-software/impeccino/releases/download`. |
| `IMPECCINO_SKILL_DIR` | Launcher-provided skill directory used to locate `SKILL.md`, references, metadata, and the default concept catalog. | Launcher sets the directory containing `SKILL.md`; a direct binary invocation walks up from its executable to find `reference/ios.md`. |
| `IMPECCINO_SELF` | Launcher-provided command path printed in engine instructions. | Launcher sets its own path; a direct binary invocation uses the executable path, then `impeccino`. |
| `IMPECCINO_LAUNCHER_PROBE` | Internal guard used while checking whether a candidate binary has the pinned engine version. Do not set it for normal commands. | Unset. |
| `IMPECCINO_COOLDOWN_FILE` | Internal Windows launcher handoff to PowerShell while it reads or writes the download-failure cooldown marker. Do not set it yourself. | Set only for the duration of that launcher helper call. |
| `HOME`, `USERPROFILE` | Home-directory fallbacks for the cache, staleness notice file, OpenCode pins, and `~/` hook paths. | POSIX prefers `HOME`; Windows prefers `USERPROFILE`. |
| `IMPECCINO_PROVIDER_ID` | Overrides provider identity used to select harness-specific hook paths and command sigils. | Derived from the skill's harness directory; `source` when it cannot be inferred. |
| `IMPECCINO_CONTEXT_DIR` | Fallback directory for `PRODUCT.md` or `DESIGN.md` when normal project and repository locations contain neither file. Relative paths resolve from the working directory. | Unset or blank: no extra context directory. |
| `IMPECCINO_CRITIQUE_META` | Optional JSON object merged into critique-write metadata. The writer replaces target fingerprint and identity fields with freshly resolved values. | Unset or invalid JSON: no caller metadata. |
| `IMPECCINO_STALENESS_CACHE` | Overrides the file used to throttle repeat staleness notices. | `~/.impeccino/staleness-check.json`. |
| `IMPECCINO_NO_STALENESS_CHECK` | Any non-empty value disables the staleness check. | Unset or empty: enabled unless project config sets `stalenessCheck` to `false`. |
| `OPENCODE_CONFIG_DIR` | OpenCode user config directory used by `pin` to find command shortcuts. | Then `XDG_CONFIG_HOME/opencode`; otherwise `~/.config/opencode`. |
| `XDG_CONFIG_HOME` | Base for OpenCode's user config directory when `OPENCODE_CONFIG_DIR` is unset. | `~/.config`. |
| `IMPECCINO_PALETTE_SEED` | Fallback seed for deterministic `palette` selection; an explicit `--from` argument takes precedence. | Unset or blank: choose randomly. |
| `IMPECCINO_API_TIMEOUT` | Time budget, in milliseconds, for the concept catalog API request. | `4000`. |
| `IMPECCINO_API_URL` | Concept catalog API base URL. | `https://impeccable.style/api`. |
| `IMPECCINO_CARD_BASE` | Base URL for concept card images. | `https://impeccable.style/worlds/cards`. |
| `IMPECCINO_CATALOG_DIR` | Directory containing local concept catalog data. | The installed skill's `scripts/` directory, or the current directory if the skill location is unknown. |
| `IMPECCINO_COMPOSITIONS` | Set to `1` to include composition suggestions in concept-seed output. | Unset: omit compositions. |
| `IMPECCINO_CONCEPT_SEED` | Fallback key for deterministic concept-seed selection when `--from` is absent. | Unset or blank: generate a random key. |
| `IMPECCINO_NO_TELEMETRY`, `DO_NOT_TRACK` | Any non-empty value suppresses the anonymous chosen-concept ping. | Unset or empty: the ping may run after a qualifying choice. |

## Hooks and rendered-page scans

Source: `crates/hook/src`, `crates/context/src/context_cli.rs`, and `crates/cli/src/page_scan`.

| Variable | Reader and effect | Default or fallback |
|---|---|---|
| `IMPECCINO_CACHE_ROOT` | Relocates disposable hook cache state beneath a root with a per-project directory. A `~/` prefix expands using the home variables above. | Unset or blank: state is stored under the project's `.impeccino/` directory. |
| `CURSOR_PROJECT_DIR` | Hook event project-directory fallback. GitHub events use `cwd`, then this variable, then the hook process working directory. Cursor events use `cwd`, first `workspace_roots` entry, then this variable, then process working directory. Grok events use `cwd`, `workspaceRoot`, then this variable, then process working directory. | Used only when the event has no usable directory. |
| `IMPECCINO_HOOK_HARNESS` | Forces hook event interpretation for a named harness (`cursor`, `github`, `grok`, `claude`, `codex`, or `gemini`). | Inferred from the event, then `claude`. |
| `IMPECCINO_HOOK_DISABLED` | A truthy value (`1`, `true`, `yes`, or `on`, case-insensitive) disables automatic hook scans. | Unset or false: project hook configuration decides. |
| `IMPECCINO_HOOK_QUIET` | A truthy value suppresses per-edit hook status output. | Unset or false: follow project `hook.quiet` setting (default `false`). |
| `IMPECCINO_HOOK_DEPTH`, `CLAUDE_HOOK_DEPTH` | After trimming, `1`, `true`, `yes`, `on` (case-insensitive), or a string of ASCII digits containing a non-zero digit marks a nested invocation and stops it from running again. Values such as `0`, `false`, and `abc` do not. | Unset, blank, or a string of zeroes: process the event normally. |
| `IMPECCINO_HOOK_LOG` | Overrides the hook audit-log path; `~/` expands from `HOME` or `USERPROFILE`. | The `hook.auditLog` project setting, or no audit log. |
| `IMPECCINO_AGENT_BROWSER` | Selects the `agent-browser` executable used for rendered-page scans and availability checks. | Search `PATH` for `agent-browser`. |
| `PATH`, `PATHEXT` | Search locations and Windows executable extensions used to find `agent-browser`. | Operating-system values. |
| `AGENT_BROWSER_SESSION` | Reuses an existing agent-browser session for URL scans and leaves that session open afterward. | Create a temporary Impeccino session and close it afterward. |
| `SystemRoot` | Provides the root for the Windows launcher’s PowerShell cooldown helper and, when the file exists, `System32\taskkill.exe` while stopping a rendered scan's process tree. | Windows normally provides it. The launcher has no PowerShell fallback; for `taskkill.exe`, an unset root or missing file falls back to `PATH`. |

## Repository tests and release tooling

These controls affect checks and test fixtures, not normal skill runs.

Source: `scripts/`, `tests/`, and Rust test modules under `crates/*/tests`.

| Variable | Effect | Default or fallback |
|---|---|---|
| `IMPECCINO_PUBLIC_REPO` | Checkout root for Rust fixture, vector, and HTML oracle tests. | The current repository checkout. |
| `IMPECCINO_ORACLE_PREFIX` | Limits the JS oracle replay to case IDs with this prefix. | Unset: replay all cases. |
| `IMPECCINO_PAGE_SCAN_SIGTERM_CHILD` | Internal child mode for the page-scan SIGTERM cleanup test. | Used only by that test. |
| `IMPECCINO_PAGE_SCAN_TREE_CHILD_EXE` | Internal executable path for the page-scan process-tree cleanup test. | Used only by that test. |
| `IMPECCINO_PAGE_SCAN_TREE_CHILD_PID_FILE` | Internal PID-file path for the page-scan process-tree cleanup test. | Used only by that test. |
| `IMPECCINO_TEST_ARG_CAPTURE` | File path where a Windows test stub records launcher arguments. | Used only by the launcher contract test. |
| `IMPECCINO_TEST_WALL_CLOCK_MS` | Global wall-clock limit in milliseconds for each test command. | `1200000`, unless a suite sets its own limit. |
| `IMPECCINO_SKIP_ENGINE_CHECK` | Set to `1` to bypass the skill-release engine asset probe when assets exist but the probe is unreachable. | Unset: the release gate checks the pinned engine assets. |
| `IMPECCINO_SKILL_BEHAVIOR_MODELS` | Comma-separated model IDs for the opt-in skill-behavior suite. | `claude-sonnet-5`, `gpt-5.6-terra`, and `gemini-3.7-flash`. |
| `IMPECCINO_SKILL_BEHAVIOR_EFFORT` | OpenAI reasoning effort for the skill-behavior suite. | `high`. |
| `IMPECCINO_SKILL_BEHAVIOR_REQUIRE_PROVIDER` | Set to `1` in the billed CI lane to fail if no provider-backed scenario runs. | Unset: missing provider keys skip those scenarios. |
| `IMPECCINO_SKILL_BEHAVIOR_TRACE_DIR` | Directory for tool-call traces written by the test harness. | Unset: traces remain in memory. |
| `IMPECCINO_SKILL_BEHAVIOR_TURN_TIMEOUT_MS` | Per-turn timeout in milliseconds for provider calls. | `840000`. |
| `IMPECCINO_SKILL_BEHAVIOR_VERBOSE` | Set to `1` to print per-scenario traces. | Unset: concise test output. |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_CLOUD_API_KEY`, `DEEPSEEK_API_KEY` | Optional credentials read by the provider-backed tests from the process environment or repo-root `.env`. | Missing keys cause local provider scenarios to skip. Do not commit key values. |
| `GOOGLE_GENERATIVE_AI_API_KEY` | Provider SDK key set from `GOOGLE_CLOUD_API_KEY` when the SDK-specific name is not already present. | An existing value takes precedence; otherwise copied from the Google test key. |
| `GITHUB_EVENT_NAME`, `CI_CHANGED_FILES` | Inputs used by the CI test planner to select suites and changed-file coverage. | GitHub event metadata, or the local no-changes plan. |
| `GITHUB_EVENT_INPUTS_SKILL_BEHAVIOR` | Enables the billed skill-behavior lane when the manual workflow input is `true`. | Unset or any other value: lane remains opt-in and off. |
| `GITHUB_SHA`, `GITHUB_BASE_REF`, `GITHUB_EVENT_BEFORE` | Commit references used to calculate changed files in CI. | The CI planner falls back to the available event/ref or `HEAD`. |
| `GITHUB_OUTPUT` | GitHub Actions output file used by the CI test planner. | Provided by GitHub Actions. |
| `CARGO_ABOUT_BIN` | Path to the `cargo-about` executable used to generate engine notices. | `cargo-about` on `PATH`. |
| `ComSpec`, `COMSPEC`, `PROCESSOR_ARCHITECTURE`, `TEMP` | Windows shell, architecture, and temporary-directory values used by launcher tests. | Provided by Windows. |
| `TMPDIR` | Temporary-directory base used by POSIX launcher tests. | `/tmp`. |

The opt-in `test:skill-behavior` and `test:skill-workflow` suites can make
provider API calls. Their credentials stay in the local environment or the
gitignored repo-root `.env`; only the variable names are listed here.

The default Vitest suite checks this inventory against environment names read
from source files. The check strips source comments before extracting readers,
so a name mentioned only in a comment does not count as coverage.
