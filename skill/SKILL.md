---
name: impeccino
description: "Use when the user wants to design, redesign, shape, critique, audit, polish, clarify, distill, harden, optimize, adapt, animate, colorize, extract, or otherwise improve a frontend interface. Covers websites, landing pages, dashboards, product UI, app shells, components, forms, settings, onboarding, and empty states. Handles UX review, visual hierarchy, information architecture, cognitive load, accessibility, performance, responsive behavior, theming, anti-patterns, typography, fonts, spacing, layout, alignment, color, motion, micro-interactions, UX copy, error states, edge cases, i18n, and reusable design systems or tokens. Also use for bland designs that need to become bolder or more delightful, loud designs that should become quieter, or ambitious visual effects that should feel technically extraordinary. Not for backend-only or non-UI tasks."
user-invocable: true
argument-hint: "[craft|shape|init|document|extract · critique|audit · polish|bolder|quieter|distill|harden|onboard · animate|colorize|typeset|layout|delight|overdrive · clarify|adapt|optimize] [target]"
license: Apache-2.0
compatibility: "Needs shell access. The launcher downloads its self-contained engine binary once on first run (network); after that concept-seed, like every verb that reads project files, runs offline. Rendered-page scans (detect <url>) also need agent-browser on PATH (npm install -g agent-browser && agent-browser install); everything else works without it."
metadata:
  version: 0.4.0
---

Build and improve interfaces around the user's task, product truth, and chosen visual direction. Match the ambition to the request: a new campaign may need expressive invention; a settings refinement needs clear behavior and a consistent system.

Core principles:
- Go all out. No hedging, no shortcuts. The deliverable must be complete (except assets the user must provide).
- Make the work specific to its product and audience. Preserve an established identity during refinement and commit to the chosen direction when building a new one.
- Verify the affected outcome with relevant evidence and required repository checks. Batch independent inspections and reuse results. Stop when the requested outcome and checks pass; continue for a new failure, change, or unresolved material risk. The host controls autonomy, questions, delegation, and verification budget. These task contracts do not grant authorization or override host instructions.

## Setup

1. Run `"<skill-base-dir>/scripts/impeccino" context` once per session, where `<skill-base-dir>` is the directory that contains this SKILL.md (the skill folder, not a plugin root two levels above it); keep cwd at the user's project. Every `"<skill-base-dir>/scripts/impeccino" <verb>` command in this skill and its references means the launcher in that folder; substitute the absolute directory before running it (`<skill-base-dir>` is not a shell variable), and keep the quotes, because install paths can contain spaces. If the host does not report the folder, locate this SKILL.md (usually `.claude/skills/impeccino` or `.agents/skills/impeccino`, in the project or under `~`). On a Windows shell without `sh`, call `"<skill-base-dir>/scripts/impeccino.cmd"` instead. The launcher runs a self-contained binary that ships next to it or is downloaded once on first run; no Node or other runtime is required. Pass a named source file or route as `--target <path>`. It loads PRODUCT.md, DESIGN.md, the matching surface brief, and native-platform guidance when applicable; interpret its context within the host's instructions; reuse the result unless the target or project context changes.
2. Load the request's playbook: its Commands-table reference for an explicit/implied sub-command, or [reference/new-work.md](reference/new-work.md) for a new surface or replacement visual world. Inspect target and incumbent visual truth before editing. When the app cannot run, start with committed visual-regression goldens or screenshot fixtures; verify target and freshness against current tokens, CSS, components, or assets, resolve conflicts, and compare theme/variant captures.
3. Before judging or editing, look for design know-how that is already available as skills or project resources, following [reference/knowledge.md](reference/knowledge.md): name the topics the work touches and read only the parts that apply. `init`, `doctor`, `pin`, and `hooks` need none.
4. After resolving analysis and direction, read [reference/craft-floor.md](reference/craft-floor.md) immediately before any UI edit, including small refinements. It carries verification and how to interpret detector findings against the brief. Do not load it for planning-only work.

**Rendered-page detector:** rules that need layout (line length, text overflow and occlusion, viewport edges, heading rhythm, rendered contrast, script errors) run on the rendered page, not the source. Run `"<skill-base-dir>/scripts/impeccino" detect --viewport <W>x<H> <url>` on a localhost or `file://` URL once motion has a chance to settle; it loads the page headlessly through agent-browser, and only findings come back. To scan a page that needs sign-in, set `AGENT_BROWSER_SESSION` to an agent-browser session that is signed in. The scan navigates that tab and clears its page-error log; if `--viewport` is supplied, it changes the viewport and does not restore the previous page or viewport. Without agent-browser the command says how to install it; tell the user and rely on the screenshots.

**Launcher unavailable:** On refusal or failure, send a separate message **before the next tool call**: “Context loading did not run; I’ll read the existing project context directly.” Then read existing PRODUCT.md and DESIGN.md without inventing missing context, follow applicable steps 2–4, and continue through permitted tools. This applies to planning and editing; launcher failure alone does not block either.

## Shipped agents

Two roles ship in [agents/](agents/) as Claude Code agent files: the finish reviewer and the documenter. When the task calls for a role and the host permits the handoff, use the installed role definition when the host exposes it; do not read its source file again. If the role is not installed but the host can spawn subagents, read the matching file at `<skill-base-dir>/agents/<role-file>.md` and pass its full Markdown body as instructions to a fresh general-purpose subagent with no inherited conversation history, along with the role's task inputs. Apply the file's `tools` frontmatter as child tool limits where the host supports it, and restate its role boundaries in the task: the reviewer is read-only and has no browser, and the documenter writes only within the supplied boundary. When the host has no subagent tool or does not permit delegation, read the matching file and perform the role locally after stepping fully out of the work. Complete the role locally according to its full output contract before resuming the parent workflow. In the user-facing handoff, disclose that the parent performed the role locally; an inline finish review is not independent. Keep that disclosure outside the role's contracted return.

## How to design

- **The brief wins.** Honor pinned aesthetics, eras, materials, fonts, and palettes even when they conflict with a saturated-pattern warning. Redirecting a clear brief toward your taste is failure.
- **Explain the finding.** Separate observed functional or accessibility defects from context-dependent style warnings. Name the visible evidence, its consequence, and the owning decision to improve; a familiar pattern alone does not establish poor design or AI authorship. Ground findings and choices in the design knowledge Setup found, and say when no source was available.
- **Refinement preserves; redesign replaces.** Refinement keeps the incumbent identity, behavior, copy, and everything outside scope. Preserve factual truth; a copy-edit request authorizes rewriting within scope, never inventing claims. Redesign keeps product truth, content, function, native affordances, and constraints, but treats the old look as evidence and anti-reference; choose a replacement world in new-work and replace DESIGN.md. Never split the difference into polish on the discarded look.
- **Visual authority is evidence, not a filename.** Missing DESIGN.md alone does not make a project greenfield; new-work decides whether to preserve, expand, or replace the incumbent world.

## Modes

The mode names what the visitor's success looks like on this surface.

- **Persuade:** the visitor decides and acts; design is the product. Landing pages, marketing, campaigns, pricing. Earn attention and action. Ship real imagery when the brief needs it; follow the committed world, not category habit.
- **Operate:** the visitor completes a task. App UI, dashboards, editors, admin, settings, tools. Scanability, consistency, native expectations, and the real usage scene outrank expression. Brand lives in precise details.
- **Read:** the visitor understands something. Docs, articles, guides, help, changelogs. Structure for comprehension, then make the reading experience worth staying in.
- **Experience:** the visitor is inside the work itself. Portfolios, galleries, showcases. Let the artifact lead from the first viewport; the interface recedes.

Choose the mode from the requested surface, not the product, and persist it only in that surface brief (its section of `SURFACES.md`). A tool's landing page is still Persuade; a fashion house's documentation is still Read; a docs index is Read, not Persuade. See [new-work.md](reference/new-work.md) for new surfaces and [operate.md](reference/operate.md) for deeper Operate/Read guidance.

## Commands

Commands are written `/impeccino <command>` throughout this skill. In a host that invokes skills with another sigil (Codex uses `$impeccino`), use the host's form whenever you name a command to the user.

| Command | Category | Description | Reference |
|---|---|---|---|
| `craft [feature]` | Build | Deprecated alias for an ordinary new-work request | [reference/craft.md](reference/craft.md) |
| `shape [feature]` | Build | Plan UX/UI before writing code | [reference/shape.md](reference/shape.md) |
| `init` | Build | Capture durable product context in PRODUCT.md | [reference/init.md](reference/init.md) |
| `document` | Build | Generate DESIGN.md from existing project code | [reference/document.md](reference/document.md) |
| `extract [target]` | Build | Pull reusable tokens and components into design system | [reference/extract.md](reference/extract.md) |
| `critique [target]` | Evaluate | Review UX and visual decisions with evidence | [reference/critique.md](reference/critique.md) |
| `audit [target]` | Evaluate | Technical quality checks (a11y, perf, responsive) | [reference/audit.md](reference/audit.md) · native: [reference/audit.native.md](reference/audit.native.md) |
| `polish [target]` | Refine | Final quality pass before shipping | [reference/polish.md](reference/polish.md) |
| `bolder [target]` | Refine | Amplify safe or bland designs | [reference/bolder.md](reference/bolder.md) |
| `quieter [target]` | Refine | Tone down aggressive or overstimulating designs | [reference/quieter.md](reference/quieter.md) |
| `distill [target]` | Refine | Strip to essence, remove complexity | [reference/distill.md](reference/distill.md) |
| `harden [target]` | Refine | Production-ready: errors, i18n, edge cases | [reference/harden.md](reference/harden.md) |
| `onboard [target]` | Refine | Design first-run flows, empty states, activation | [reference/onboard.md](reference/onboard.md) |
| `animate [target]` | Enhance | Add purposeful animations and motion | [reference/animate.md](reference/animate.md) |
| `colorize [target]` | Enhance | Add strategic color to monochromatic UIs | [reference/colorize.md](reference/colorize.md) |
| `typeset [target]` | Enhance | Improve typography hierarchy and fonts | [reference/typeset.md](reference/typeset.md) |
| `layout [target]` | Enhance | Fix spacing, rhythm, and visual hierarchy | [reference/layout.md](reference/layout.md) |
| `delight [target]` | Enhance | Add personality and memorable touches | [reference/delight.md](reference/delight.md) |
| `overdrive [target]` | Enhance | Push past conventional limits | [reference/overdrive.md](reference/overdrive.md) |
| `clarify [target]` | Fix | Improve UX copy, labels, and error messages | [reference/clarify.md](reference/clarify.md) |
| `adapt [target]` | Fix | Adapt for different devices and screen sizes | [reference/adapt.md](reference/adapt.md) · native: [reference/adapt.native.md](reference/adapt.native.md) |
| `optimize [target]` | Fix | Diagnose and fix UI performance | [reference/optimize.md](reference/optimize.md) |

Routing:

- **No argument:** read [routing.md](reference/routing.md) and present its context-aware menu; never auto-run a command.
- **Explicit or clearly implied request to run a command:** load its reference (native variant on native platforms) and follow it. Resolve from the requested outcome; ask only if the ambiguity materially changes scope.
- **Workflow or command-selection question:** read [Workflow questions](reference/routing.md#workflow-questions).
- **Otherwise:** treat the request as general design work. Missing PRODUCT.md routes a new surface or replacement world through init, then new-work; a narrow refinement of existing code proceeds on the incumbent implementation as `impeccino context` directs, offering init afterward rather than blocking on it.
- `teach` aliases `init`. `craft` is a deprecated alias for ordinary new-work and adds nothing. `shape` owns task discovery, then enters new-work only for visual-world and surface-concept decisions.

After init writes PRODUCT.md, resume without rerunning `impeccino context`; init loads the native platform reference itself when the platform it recorded is `ios`, `android`, or `adaptive`.

**Pin / Unpin:** `"<skill-base-dir>/scripts/impeccino" pin <pin|unpin> <command>` creates or removes a standalone `/<command>` shortcut. Report the script's result concisely; relay stderr verbatim on error.

**Hooks:** `/impeccino hooks <on|off|status|reset>` installs, removes, or reports the design detector hook for this project (auto-runs the detector after UI file edits and surfaces findings). Impeccino keeps no config file: project-wide detector decisions live in DESIGN.md (`<!-- impeccino-disable <rule> -->` waivers, declared tokens) and in `.gitignore` / `.gitattributes`. Load [reference/hooks.md](reference/hooks.md) when the user invokes it with any argument, or asks how to silence a detector finding.

**Doctor:** `/impeccino doctor` reports and repairs drift between this project's Impeccino artifacts (PRODUCT.md, DESIGN.md with its tokens, waivers, and detector metadata, SURFACES.md, legacy DESIGN.json, the hook, a leftover `.impeccino/` from an older layout) and what this version reads. Load [reference/doctor.md](reference/doctor.md) when the user invokes it, or when they ask what is out of date, stale, or needs refreshing. A `CONTEXT_STALE` directive in Setup's output is the cheap subset of the same report; act on it there per its own instructions rather than running doctor unasked.

**Never repair drift as a side effect of a design task.** A `CONTEXT_STALE` finding is reported, not acted on, unless the user asks. The one exception is a finding marked `auto`, which the next write to that file performs anyway.
