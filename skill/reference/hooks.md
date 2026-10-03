# /impeccino hooks

Manage the **design detector hook** for the current project.

The hook runs the impeccino design detector on direct file edits to design-relevant files (`.tsx`, `.jsx`, `.html`, `.vue`, `.svelte`, `.astro`, `.css`, `.scss`, `.sass`, `.less`, `.ts`, `.js`) and the server templates `.blade.php`, `.twig`, `.html.erb`, `.erb`, `.hbs`, and `.handlebars`, which it reads with the HTML analyzer. Claude Code, Codex, and GitHub Copilot use a post-tool-use hook and push a short system reminder into the agent's context after the edit; findings get a correction prompt, pending issues get a re-nudge, and clean UI-ish files get a short ack unless `IMPECCINO_HOOK_QUIET=1` is set. Plain `.ts` and `.js` files are still scanned, but stay quiet unless the detector finds something. Cursor uses `preToolUse` to block bad proposed writes before they land and stays silent when it allows a clean write. Grok Build fires the same PostToolUse scan to mark touched files, then surfaces findings on Stop `additionalContext`. Do not expect a Grok per-edit reminder: Grok discards that stdout.

The detector rules run in two tiers. The per-edit hook surfaces only the immediate tier: mechanical, unambiguous problems worth interrupting an edit for, such as broken images, overflowing or clipped content, contrast and legibility failures, gradient text, glow shadows, and design-system drift. Everything else (copy cadence, palette and typography taste, layout rhythm) is deferred to a deep pass on the `Stop` hook event, which runs the full rule set over every UI file touched in the session and surfaces the remaining findings once, deduplicated against what the per-edit pass already reported. A session with nothing left to report stops silently. The Stop deep pass is wired for Claude Code, Codex, and Grok Build, which dispatch a native `Stop` hook event. Cursor does not get one (its stop hook is not consistently dispatched; the pre-write gate covers it), and GitHub Copilot's stop-style events do not feed context back to the model, so they keep the full detector per edit. Grok also fires an observe-only Stop with `reason: "shutdown"` after `end_turn`; skip that one, scan only `end_turn`.

Every hook is a mechanical pass. The reflexes no scanner catches live in [craft-floor.md](craft-floor.md), which the skill loads before it edits UI, so they apply whether or not a hook is wired. A session with no automatic hook gets one `MANUAL_DETECTOR_REQUIRED` directive from `impeccino context` asking for a single detector run at the end.

## No config file

Impeccino keeps no config file. The hook is on wherever its entries sit in the harness settings, so `on` installs them and `off` removes them. Its session cache lives in the user cache (`$XDG_CACHE_HOME/impeccino/projects/`, else `~/.cache/impeccino/projects/`; `%LOCALAPPDATA%\impeccino\projects\` on Windows), never in the project. Three environment variables are the only switches: `IMPECCINO_HOOK_DISABLED=1` turns an installed hook off for one shell, `IMPECCINO_HOOK_QUIET=1` silences the clean and pending acks, and `IMPECCINO_HOOK_LOG=<path>` appends one NDJSON line per invocation.

What used to be detector config is read from the project's own files, by the hook and by `impeccino detect` alike:

- **A rule the project has decided against** is waived in DESIGN.md with `<!-- impeccino-disable <rule-id>: <reason> -->`, best placed next to the Named Rule or Do/Don't that justifies it. It turns the rule off for every file that DESIGN.md governs. Same syntax as the in-file waiver; a bare `impeccino-disable` with no rule id is not honored at project scope.
- **A deliberate value** belongs in DESIGN.md as a token. A font declared under `typography` (or in DESIGN.json) never counts as an overused font, and the design-system rules accept every declared color, radius, and size.
- **Files that are not the project's own source** stay out through git: whatever `.gitignore` (or `.git/info/exclude`) ignores, and whatever `.gitattributes` marks `linguist-generated` or `linguist-vendored`, is skipped inside a git repository.
- **One spot** is waived where it lives: `impeccino-disable <rule>` (whole file) or `impeccino-disable-line` / `impeccino-disable-next-line` (one line), in any comment syntax, with an optional reason after `:` or `--`.

`impeccino detect --no-config` scans raw: no DESIGN.md tokens, waivers, or declared fonts, and no in-file waivers. Git's ignore rules still apply.

Supported harnesses: Claude Code (`.claude/settings.local.json` in the project, which is gitignored so the hook stays machine-local; a hook you move into the shared `settings.json` is honored in place too), Codex (`.codex/hooks.json` in the project), Cursor (`.cursor/hooks.json` in the project), Grok Build (`.grok/hooks/impeccino.json` in the project; requires `/hooks-trust` or `--trust`), and GitHub Copilot (`.github/hooks/impeccino.json` in the project, a team-shared committed file that both the Copilot CLI and the cloud agent read). For the Copilot CLI, repo-level hooks fire once `.github/hooks/impeccino.json` is committed to the repository's default branch.

On **Cursor**, `preToolUse` checks proposed Write/Edit/Shell write content and denies only when the real detector finds an issue. The denial message is visible to the agent as the tool error, so the agent can reconsider before the bad write lands.

## Routing

The first argument is the action. Defaults to `status`.

| Action | What it does |
|---|---|
| `status` | Print where the hook is installed, the env override, the DESIGN.md waivers and declared fonts in effect, and the session cache path. |
| `on` | Install or repair the hook entries in every harness whose skill folder is present. Writes nothing else. |
| `off` | Remove the hook entries from the local manifests `on` writes. A hook in Claude Code's team-shared `settings.json` is named, never edited. |
| `reset` | `off`, plus delete the hook's session cache for this project. |

The old `ignore-rule`, `ignore-file`, and `ignore-value` actions are gone with the config file; they answer with a pointer to the decisions above.

## Flow

1. Resolve the action from the user's argument. If no action was given, default to `status`.
2. Invoke the admin script and pass the user's output through verbatim:

   ```bash
   "<skill-base-dir>/scripts/impeccino" hooks <action>
   ```

3. If `<action>` is `off`, follow up with a one-line note: "Done. New edits will not trigger the design hook in this project until you run `/impeccino hooks on`."
4. If `<action>` is `on`, follow up with: "Done. The design hook will fire after the next Edit/Write on a UI file."
5. If `<action>` is `status`, just print the script output. Do not add commentary unless the user asked a follow-up question.

## Triage findings

The hook never writes anything into the project. Triage each finding into one of three outcomes:

- **Real design problem**: fix it. Never waive a finding to skip a fix or to push a blocked write through.
- **Confident false positive or sanctioned exception in one place**: waive it where it lives with an in-file comment and disclose it in your reply. The bar is evidence you can name: an intentional demo or fixture, documentation of bad design, literal or domain-appropriate motion (a ball that bounces), or a choice the user already confirmed. Put that evidence in the reason as `<who decided: evidence>`; write "user confirmed" only when the user actually did.
- **Unsure**: leave the finding standing and ask the user in one line. Ask once; a one-line question costs less than the hook re-firing on every later edit.

Self-serve stops at the in-file waiver. A project-wide decision (a DESIGN.md waiver, a new DESIGN.md token, a `.gitignore` or `.gitattributes` entry) changes what the whole team's detector reports, so ask the user first and record it in the fitting file:

- The project has decided against a rule everywhere: add `<!-- impeccino-disable <rule-id>: <reason> -->` to DESIGN.md next to the Named Rule or Do/Don't it follows from. If no rule states the decision yet, write one; a waiver with no design rule behind it reads as a shortcut.
- A font, color, radius, or size is deliberate: declare it as a DESIGN.md token. An `overused-font` finding for a declared font never fires again, and the design-system rules accept it.
- A file is generated, vendored, or a fixture: keep it out with `.gitignore` when git should not track it either, or mark it in `.gitattributes`:

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
