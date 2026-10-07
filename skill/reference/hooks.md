# /impeccino hooks

Manage the **design detector hook** for the current project.

The hook runs the impeccino design detector on direct file edits to design-relevant files (`.tsx`, `.jsx`, `.html`, `.vue`, `.svelte`, `.astro`, `.css`, `.scss`, `.sass`, `.less`, `.ts`, `.js`) and the server templates `.blade.php`, `.twig`, `.html.erb`, `.erb`, `.hbs`, and `.handlebars`, which it reads with the HTML analyzer. Claude Code, Codex, and GitHub Copilot use a post-tool-use hook and push a short system reminder into the agent's context after the edit; findings get a correction prompt, pending issues get a re-nudge, and clean UI-ish files get a short ack unless `IMPECCINO_HOOK_QUIET=1` is set. Plain `.ts` and `.js` files are still scanned, but stay quiet unless the detector finds something. Cursor uses `preToolUse` to deny proposed writes with primary defects and stays silent when it allows a write. When manually configured, Grok Build runs the same PostToolUse scan to mark touched files, then surfaces findings on Stop `additionalContext`. Do not expect a Grok per-edit reminder: Grok discards that stdout.

The detector rules run in two tiers. The per-edit hook surfaces the immediate tier, including measured defects and contextual signals such as gradient text, glow shadows, and design-system drift. Production reports mark pattern, declared-value drift, and threshold/risk findings advisory; they alone cannot deny a write. Everything else (copy cadence, palette and typography taste, layout rhythm) is deferred to a deep pass on the `Stop` hook event, which runs the full rule set over every UI file touched in the session and surfaces the remaining findings once, deduplicated against what the per-edit pass already reported. A session with nothing left to report stops silently. Codex Stop has only a blocking result, so it omits advisories; those remain visible in post-edit reports and explicit scans. The Stop deep pass is wired for Claude Code, Codex, and Grok Build, which dispatch a native `Stop` hook event. Cursor does not get one (its stop hook is not consistently dispatched; the pre-write gate covers it), and GitHub Copilot's stop-style events do not feed context back to the model, so they keep the full detector per edit. Grok also fires an observe-only Stop with `reason: "shutdown"` after `end_turn`; skip that one, scan only `end_turn`.

Every hook is a mechanical pass. The reflexes no scanner catches live in [craft-floor.md](craft-floor.md), which the skill loads before it edits UI, so they apply whether or not a hook is wired. A session with no automatic hook gets one `MANUAL_DETECTOR_REQUIRED` directive from `impeccino context` asking for a single detector run at the end.

## No config file

Impeccino keeps no config file. Hooks are a per-project opt-in: `on` installs entries only in supported harness manifests inside the current project, and `off` removes those entries. Running either action from a nested directory uses the repository's manifest set. A globally installed Claude skill can opt one project in by writing its absolute launcher path to that project's `.claude/settings.local.json`; other global skill locations do not identify a harness for project hooks. Impeccino never edits user-level hook settings. Its session cache lives in the user cache (`$XDG_CACHE_HOME/impeccino/projects/`, else `~/.cache/impeccino/projects/`; `%LOCALAPPDATA%\impeccino\projects\` on Windows), never in the project. Three environment variables are the only switches: `IMPECCINO_HOOK_DISABLED=1` turns an installed hook off for one shell, `IMPECCINO_HOOK_QUIET=1` silences the clean and pending acks, and `IMPECCINO_HOOK_LOG=<path>` appends one NDJSON line per invocation.

What used to be detector config is read from the project's own files, by the hook and by `impeccino detect` alike:

- **A rule the project has decided against** is waived in DESIGN.md with `<!-- impeccino-disable <rule-id>: <reason> -->`, best placed next to the Named Rule or Do/Don't that justifies it. It turns the rule off for every file that DESIGN.md governs. Same syntax as the in-file waiver; a bare `impeccino-disable` with no rule id is not honored at project scope.
- **A deliberate value** belongs in DESIGN.md as a token. A font declared under DESIGN.md’s `typography` never counts as an overused font, and the design-system rules accept every declared color, radius, and size.
- **Files that are not the project's own source** stay out through git: whatever `.gitignore` (or `.git/info/exclude`) ignores, and whatever `.gitattributes` marks `linguist-generated` or `linguist-vendored`, is skipped inside a git repository.
- **One spot** is waived where it lives: `impeccino-disable <rule>` (whole file) or `impeccino-disable-line` / `impeccino-disable-next-line` (one line), in any comment syntax, with an optional reason after `:` or `--`.

`impeccino detect --no-config` scans raw: no DESIGN.md tokens, waivers, or declared fonts, and no in-file waivers. Git's ignore rules still apply.

`hooks on` installs manifests for Claude Code (`.claude/settings.local.json` in the project, which is gitignored so the hook stays machine-local; a hook moved into shared `settings.json` is honored in place too), Codex (`.codex/hooks.json`), Cursor (`.cursor/hooks.json`), and GitHub Copilot (`.github/hooks/impeccino.json`, a team-shared committed file read by the CLI and cloud agent), when it finds that project's skill launcher. Grok Build can run the hook from a manually configured `.grok/hooks/impeccino.json` manifest with `/hooks-trust` or `--trust`; `hooks on`, `hooks off`, `hooks reset`, and `hooks status` do not write, manage, or report that manual manifest.

On **Cursor**, `preToolUse` checks proposed Write/Edit/Shell write content and denies only when the production detector reports a primary defect. The denial message is visible to the agent as the tool error, so the agent can reconsider before the write lands.

## Routing

The first argument is the action. Defaults to `status`.

| Action | What it does |
|---|---|
| `status` | Print where the hook is installed, the env override, the DESIGN.md waivers and declared fonts in effect, and the session cache path. |
| `on` | Install or repair hook entries for recognized project-local skill launchers. Fails with an installation instruction when no supported launcher is found. A malformed whole-settings file is preserved and reported as an error. |
| `off` | Remove the hook entries from the local manifests `on` writes. A hook in Claude Code's team-shared `settings.json` is named, never edited. |
| `reset` | `off`, plus delete the hook's session cache for this project. |

The retired `ignore-rule`, `ignore-file`, and `ignore-value` actions only return migration guidance to the DESIGN.md waiver and token rules above; they do not write config or record a waiver.

## Flow

1. Resolve the action from the user's argument. If no action was given, default to `status`.
2. Invoke the admin script and pass the user's output through verbatim:

   ```bash
   "<skill-base-dir>/scripts/impeccino" hooks <action>
   ```

3. If `<action>` is `off` and the command succeeded, follow up with a one-line note. When the output names a remaining team-shared Claude hook, say the project-local entries were removed but that shared hook remains; otherwise say: "Done. New edits will not trigger the design hook in this project until you run `/impeccino hooks on`."
4. If `<action>` is `on` and the command succeeded, follow up with: "Done. The design hook is enabled for this project and will run at the harness's next applicable edit event or proposed write." If it returned a nonzero exit status, pass the error through and do not imply that hooks were enabled.
5. If `<action>` is `status`, just print the script output. Do not add commentary unless the user asked a follow-up question.

## Triage findings

The hook does not repair application code. Interpret each finding against its
measurement, the brief, rendered consequences, and recorded decisions. Repair
supported defects within the task's scope. Advisory matches alone do not prove
poor design; preserve justified choices and address unjustified pattern clusters.

Keep existing waivers. Record an evidenced exception where it belongs, within
the host's authorization and task scope. An in-file exception uses the comment
syntax above; a durable system decision uses a DESIGN.md waiver or actual token.
Generated and vendored files use the project's ignore rules. Do not invent
approval, declare a token to hide a defect, or waive a finding simply to get a
blocked write through. Ask through the host's structured question tool only
when a material decision remains unresolved; reuse existing answers.

A file kept in git can be excluded as generated or vendored through
`.gitattributes` when that classification is supported:

```gitattributes
src/generated/** linguist-generated
third_party/** linguist-vendored
```

Example in-file waivers, with the evidence named:

```css
.ball { animation: bounce 1s; } /* impeccino-disable-line bounce-easing: agent, a literal bouncing ball where the bounce is the subject */
```

```html
<!-- impeccino-disable overused-font: user confirmed, an exported brand document set in the client's Inter -->
```

## Constraints

- Do not edit the launcher or the binary behind `impeccino hook` and `impeccino hook-before-edit` from this flow. Those are skill plumbing.
- Cursor can block a proposed write when the detector finds a real issue. Claude Code, Codex, and GitHub Copilot do not block the edit; they emit a post-edit reminder instead. Removing the hook stops both blocking and reminders.
- The hook is bundled with the Impeccino skill and installed through project-local manifests: `.claude/settings.local.json`, `.codex/hooks.json`, `.cursor/hooks.json`, and `.github/hooks/impeccino.json`. On Codex, the user must approve the hook via `/hooks` the first time. On Cursor, confirm hooks are enabled under Settings -> Hooks. On GitHub Copilot, the CLI loads `.github/hooks/impeccino.json` once it is committed to the repository's default branch, and the cloud agent reads it from the repo directly.

## Failure modes

- If the user asks to "disable the hook", lead with `/impeccino hooks off`, which removes the local hook entries for this project. `IMPECCINO_HOOK_DISABLED=1` also works as an override that follows the shell, and it is the only way to silence a hook a teammate committed to a shared manifest without editing that file.
- If DESIGN.md names a rule id the detector does not have, the waiver silences nothing; `/impeccino doctor` reports it.
