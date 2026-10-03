# Runtime and test environment

The launcher passes its environment to the native engine. This reference lists names read by the engine, launchers, hooks, and repository tooling. Test controls and optional provider keys are separate from settings used by people running the skill.

## Launcher and engine

Source: `skill/scripts/impeccino`, `skill/scripts/impeccino.cmd`, `crates/common/src`, and `crates/context/src`.

| Variable | Reader and effect | Default or fallback |
|---|---|---|
| `IMPECCINO_BIN` | Explicit developer override for the engine executable. | Unset: check the pinned sibling and user-cache binary, then download the pinned release. The explicit override must be usable but may have another version. |
| `IMPECCINO_HOME` | Overrides the cache root used for downloaded engines. | `$XDG_CACHE_HOME/impeccino` when absolute, else `$HOME/.cache/impeccino`; on Windows `%LOCALAPPDATA%\impeccino`, else `%USERPROFILE%\AppData\Local\impeccino`. |
| `IMPECCINO_DOWNLOAD_BASE` | Base URL for launcher downloads and release tooling. | GitHub Releases for this repository. |
| `IMPECCINO_SKILL_DIR` | Skill directory for `SKILL.md`, references, and command metadata. | Launcher sets it; direct binary calls search upward from the executable for `reference/ios.md`. |
| `IMPECCINO_SELF` | Launcher path printed in engine instructions and detector tips. | Launcher sets it; direct binary calls use the executable path, then `impeccino`. |
| `IMPECCINO_LAUNCHER_PROBE` | Internal guard while an automatic launcher candidate is checked. Do not set for normal commands. | Unset. |
| `IMPECCINO_COOLDOWN_FILE` | Internal handoff to the Windows launcher helper for its download-failure cooldown marker. Do not set it yourself. | Set only during that helper call. |
| `HOME`, `USERPROFILE`, `LOCALAPPDATA`, `XDG_CACHE_HOME` | Home and cache roots used by the launcher, hook state, staleness throttle, and `~/` path expansion. | Operating-system values; relative `XDG_CACHE_HOME` is ignored. |
| `IMPECCINO_PROVIDER_ID` | Overrides harness identity used for hook paths and command sigils. | Derived from the installed skill path, otherwise `source`. |
| `IMPECCINO_CONTEXT_DIR` | Fallback for PRODUCT.md or DESIGN.md when normal project and repository locations contain neither. Relative paths use the working directory. | Unset or blank: no fallback directory. |
| `IMPECCINO_STALENESS_CACHE` | Overrides the file used to throttle repeat staleness notices. | `<user cache>/staleness-check.json`. |
| `IMPECCINO_NO_STALENESS_CHECK` | Any non-empty value disables the boot staleness check. | Unset or empty: enabled. |
| `OPENCODE_CONFIG_DIR` | OpenCode user config directory used by `pin` to find shortcuts. | Then `XDG_CONFIG_HOME/opencode`; otherwise `~/.config/opencode`. |
| `XDG_CONFIG_HOME` | Base for OpenCode's user config directory when `OPENCODE_CONFIG_DIR` is unset. | `~/.config`. |
| `IMPECCINO_PALETTE_SEED` | Fallback seed for deterministic `palette` selection; an explicit `--from` argument takes precedence. | Unset or blank: choose randomly. |
| `IMPECCINO_CONCEPT_SEED` | Fallback key for local deterministic `concept-seed` selection when `--from` is absent. | Unset or blank: generate a random key. |

## Hooks and rendered-page scans

Source: `crates/hook/src`, `crates/context/src/context_cli.rs`, and `crates/cli/src/page_scan`.

| Variable | Reader and effect | Default or fallback |
|---|---|---|
| `IMPECCINO_CACHE_ROOT` | Relocates disposable hook state beneath a root with a per-project directory. `~/` expands using `HOME` or `USERPROFILE`. | `<user cache>/projects`; if no home is available, a system temporary directory is used. |
| `CURSOR_PROJECT_DIR` | Hook event project-directory fallback. GitHub events use `cwd`, then this variable, then the hook process working directory. Cursor and Grok events have their own event-field precedence before this fallback. | Used only when the event has no usable directory. |
| `IMPECCINO_HOOK_HARNESS` | Forces event interpretation for a named harness. | Inferred from the event, then `claude`. |
| `IMPECCINO_HOOK_DISABLED` | Truthy value (`1`, `true`, `yes`, or `on`, case-insensitive) disables installed hook scans. | Unset or false: the hook runs where its manifest entries are installed. |
| `IMPECCINO_HOOK_QUIET` | Truthy value suppresses clean and pending per-edit hook acknowledgments. | Unset or false: show the normal acknowledgment. |
| `IMPECCINO_HOOK_DEPTH`, `CLAUDE_HOOK_DEPTH` | Nested-invocation guards. Truthy values and nonzero ASCII digit strings stop the hook from running recursively. | Unset, blank, or zero: process the event normally. |
| `IMPECCINO_HOOK_LOG` | Writes one NDJSON record per invocation; `~/` expands from `HOME` or `USERPROFILE`. | Unset: no audit log. |
| `IMPECCINO_AGENT_BROWSER` | Selects the executable for rendered-page scans and availability checks. | Search `PATH` for `agent-browser`. |
| `PATH`, `PATHEXT` | Search locations and Windows executable extensions used to find `agent-browser`. | Operating-system values. |
| `AGENT_BROWSER_SESSION` | Reuses an existing agent-browser session for URL scans and leaves it open afterward. | Create a temporary Impeccino session and close it afterward. |
| `SystemRoot` | Locates the Windows PowerShell cooldown helper and, when present, `System32\taskkill.exe` for stopping a rendered scan's process tree. | Windows normally provides it; taskkill can fall back to `PATH`. |

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
| `IMPECCINO_SKIP_ENGINE_CHECK` | Bypasses the skill-release engine asset probe when assets exist but the probe is unreachable. | Unset: the release gate checks the pinned engine assets. |
| `IMPECCINO_SKILL_BEHAVIOR_MODELS` | Comma-separated model IDs for the opt-in skill-behavior suite. | `claude-sonnet-5`, `gpt-5.6-terra`, and `gemini-3.7-flash`. |
| `IMPECCINO_SKILL_BEHAVIOR_EFFORT` | OpenAI reasoning effort for the skill-behavior suite. | `high`. |
| `IMPECCINO_SKILL_BEHAVIOR_REQUIRE_PROVIDER` | Fails the billed CI lane if no provider scenario runs. | Unset: missing keys skip local scenarios. |
| `IMPECCINO_SKILL_BEHAVIOR_TRACE_DIR` | Directory for provider test traces. | Unset: traces remain in memory. |
| `IMPECCINO_SKILL_BEHAVIOR_TURN_TIMEOUT_MS` | Per-turn timeout for provider calls. | `840000`. |
| `IMPECCINO_SKILL_BEHAVIOR_VERBOSE` | Prints per-scenario traces. | Unset: concise test output. |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_CLOUD_API_KEY`, `GOOGLE_GENERATIVE_AI_API_KEY`, `DEEPSEEK_API_KEY` | Optional credentials for provider-backed tests. | Read from process environment or repo-root `.env`; missing keys skip scenarios. Do not commit key values. |
| `GITHUB_EVENT_NAME`, `CI_CHANGED_FILES` | Inputs to the CI test planner. | GitHub event metadata or the local no-changes plan. |
| `GITHUB_EVENT_INPUTS_SKILL_BEHAVIOR` | Enables the billed skill-behavior lane when the manual workflow input is `true`. | Unset or another value: lane remains off. |
| `GITHUB_SHA`, `GITHUB_BASE_REF`, `GITHUB_EVENT_BEFORE` | Commit references used to calculate changed files in CI. | Available event/ref metadata or `HEAD`. |
| `GITHUB_OUTPUT` | GitHub Actions output file used by the CI planner. | Provided by GitHub Actions. |
| `CARGO_ABOUT_BIN` | Path to the `cargo-about` executable used to generate engine notices. | `cargo-about` on `PATH`. |
| `ComSpec`, `COMSPEC`, `PROCESSOR_ARCHITECTURE`, `TEMP`, `TMPDIR` | Windows shell, architecture, and temporary-directory values used by launcher and process tests. | Operating-system values. |

The opt-in `test:skill-behavior` and `test:skill-workflow` suites can make
provider API calls. Their credentials stay in the local environment or the
gitignored repo-root `.env`; only variable names are listed here.

The default Vitest suite checks this inventory against environment names read
from source files. The check strips source comments before extracting readers,
so a name mentioned only in a comment does not count as coverage.
