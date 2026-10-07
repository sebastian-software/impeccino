# Design hooks

Enable automatic edit feedback with `/impeccino hooks on`; use `status` to inspect
it and `off` to remove the project-local entries. The full action and waiver
contract lives in [the skill reference](../skill/reference/hooks.md).

## Project setup

The design hook runs the Impeccino detector on direct UI file edits and surfaces findings back into the agent flow. It is a per-project opt-in ([ADR 0007](adr/0007-hooks-are-a-project-opt-in.md)): run `/impeccino hooks on` in a project with its skill launcher installed, and the engine writes that project's harness manifest; `/impeccino hooks off` and `reset` undo it. Running the action from a nested directory still uses the repository's manifest set. Codex, Cursor, and Copilot hook commands resolve launchers from the Git root, so those targets require a Git checkout. Codex also refuses a project subpath containing `%`, which Windows command parsing expands as an environment variable; move that workspace under a path without `%`. A global Claude skill can write its absolute launcher path into the current project's gitignored `.claude/settings.local.json`; other global skill locations do not activate hooks or change user-level hook settings. Without the hook, the skill asks for one manual detector run when a change is finished.

Hook surfaces the engine manages:

- Claude Code: `.claude/settings.local.json` (gitignored, machine-local). A hook moved into the shared `settings.json` is honored in place.
- Codex: `.codex/hooks.json`, with a `commandWindows` sibling that calls `impeccino.cmd` for cmd.exe. Approve it in `/hooks`.
- Cursor: `.cursor/hooks.json`, which checks proposed writes before they land.
- GitHub Copilot: `.github/hooks/impeccino.json`, a team-shared file the Copilot CLI reads once it is committed to the default branch.

The hook also understands Grok Build's events, and `context` recognizes a Grok manifest at `.grok/hooks/impeccino.json`; `hooks on` does not write that one. Gemini CLI has no hook manifest; the skill asks for a manual detector run there.

Every hook command goes through the skill's launcher. `hooks on` checks that the detected skill includes its launchers; it returns an actionable error when no supported project launcher exists or a whole Claude settings file is malformed. Unrelated hook entries and settings are preserved. The hook is on where its entries are installed; there is nothing else to configure. `IMPECCINO_HOOK_DISABLED=1` turns an installed hook off for one shell, and `IMPECCINO_HOOK_QUIET=1` silences clean and pending per-edit acknowledgments; Stop findings remain visible.

## Execution and troubleshooting

In Claude Code, command hooks run independently of model-tool approval, so the first edit or Stop event can download and cache the engine even if the session denies the model's launcher command. Review hooks before unattended runs; to disable all Claude Code hooks for a run, pass `--settings '{"disableAllHooks": true}'`.

For debugging, set `IMPECCINO_HOOK_LOG` to a path to write one NDJSON line per hook invocation. Leave it unset for normal use.

Each Stop scans up to 20 touched files. Large touched sets and project order rotate across Stop invocations so later files and projects also receive a deep pass. The Stop pass suppresses confirmed pre-existing findings when a verified before-edit baseline is available (currently Claude Edit/Write results for text scans). Other findings are marked new or attribution unknown; unknown is not evidence that your session caused the problem. Explicit checks requested through the skill still report findings independently of session attribution.

## Explicit quality checks

Use `/impeccino audit <target>` to have the agent review the target. For web projects, the audit checks source files and rendered pages; native iOS and Android audits use platform-specific guidance without browser scans. Web workflows and opt-in hooks use the internal engine's 61 deterministic detector rules. Rendered web checks need [agent-browser](https://github.com/vercel-labs/agent-browser) installed (`npm install -g agent-browser && agent-browser install`); source checks do not.

The production reporting policy distinguishes measured defects from contextual
pattern, declared-value drift, and threshold/risk advice. Advisories alone do not
deny Cursor writes or produce finding exit status 2. Codex Stop suppresses
advisory-only context because that event supports only a blocking result;
post-edit reports and explicit scans retain the advice. Runtime policy changes
reach installed skills through the pinned engine release; see [ENGINE.md](ENGINE.md#production-finding-policy).

The agent interprets findings alongside suitable design knowledge and visual inspection. A clean detector run is evidence, not proof of visual or accessibility quality. Ask the agent to record justified waivers and inspect the experience across relevant viewports.

