# Command guidance

## Workflow questions

Give advice without executing commands; the menu below is only for bare invocations. Consult relevant command references as needed for prerequisites and scope. If the user also requests execution, follow that request.

## No-argument routing: the context-aware menu

Read this when the user invokes `/impeccino` with no argument. They are asking "what should I do?" Make the menu context-aware instead of static.

Setup has already run `impeccino context`. If that reported `NO_PRODUCT_MD`, the project has no captured context yet: lead the menu with `/impeccino init` as the top recommendation (one line on why) and still show the rest below; don't silently jump into init. Otherwise run `"<skill-base-dir>/scripts/impeccino" signals` once and read its JSON, then lead with the **2-3 highest-value next commands**, each with a one-line reason pulled from the signals, followed by the full menu (the Commands table in SKILL.md, grouped by category). **Never auto-run a command; the recommendation is a suggestion the user confirms.**

Reason over the signals; there is no score to obey:

- `setup.hasDesign` false while `setup.hasCode` true → `document` (capture the visual system).
- A set-up project (`setup.hasProduct`) with a real surface → offering `/impeccino critique <surface>` is a strong default for an evaluation. Critiques are not archived, so nothing in the signals says whether one ran before; if this conversation already holds one, `polish` acts on its Priority Issues.
- `git.changedFiles` pointing at one surface → scope `audit` or `polish` to those files specifically, naming them.
- **The bundled `impeccino detect` is web-only.** If `setup.platform` is `ios`, `android`, or `adaptive`, don't lead with it; the HTML rule engine doesn't apply to native app code.
- Otherwise group by intent (build new / improve what's there / evaluate), tailored to the current surface and `setup.platform`.

**If `scan.targets` is non-empty and `setup.platform` is not `ios`/`android`/`adaptive`, run `"<skill-base-dir>/scripts/impeccino" detect --json <scan.targets joined by spaces>` once** (the bundled detector over local files: no network, no npx; it reads HTML/CSS, so skip it for native projects). `scan.via` tells you what they are: `git-changes` (the markup/style files in your dirty tree, the most relevant set), `source-dir` (e.g. `src`, `app`), `html`, or `root`. Fold the hits into your picks: many quality / contrast hits → `audit` or `polish`; a specific slop family → the matching command (gradient text or eyebrows → `quieter` / `typeset`, flat or gray palette → `colorize`, and so on). It's a real, current signal that beats guessing. If detect errors or the tree is large and slow, skip it and recommend the user run `audit` themselves; never block the suggestion on it.

Keep it to 2-3 pointed picks with the exact command to type. The menu stays the fallback; the recommendation is the lede.
