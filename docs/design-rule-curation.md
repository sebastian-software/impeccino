# Design rule curation

On 6 October 2026, 84 deduplicated design-review points were accepted for the
Skill Library. The acceptance included their contextual exceptions. 82 points
had related material in at least two source projects; R21 and R24 were accepted
individually. Each project counted once, including Impeccino. These counts
describe source overlap, not independent votes or proof of universal validity.

The maintained knowledge owner is `effective-web` in the Skill Library.
Impeccino keeps task entry points, context, measurements, and project-file
contracts. Setup step 3 asks the agent to look for design
knowledge by topic through `skill/reference/knowledge.md`, project sources
before user-wide ones; it names no particular library. The Library is still not a mandatory runtime
dependency or an exclusive source; without a qualifying source, the work
continues on general knowledge and the report says so.
This follows [issue 74](https://github.com/sebastian-software/impeccino/issues/74).

## Content dispositions

The Library already covered many points. Existing functional and specialist
guidance remains at its owner; the additions extend contextual pattern review
and close the identified gaps. They do not add 84 detector rules.

| Review IDs | Subject | Maintained knowledge owner |
| --- | --- | --- |
| R01–R07 | Card chrome, nesting, radii, depth, component fit, and tile weight | Layout pattern review; layout foundations; design-system rules |
| R08–R17 | Font choice, hierarchy, display size, labels, emphasis, measure, readability, numbers, and case | Visual pattern review; typography system and detail; line height and measure |
| R18–R29 | Page structure, alignment, columns, alternation, width, grouping, heading proximity, first viewport, close, numbering, pricing, and FAQ | Layout pattern review; layout foundations; design planning core |
| R30–R36 | Palette, themes, light effects, glass, textures, colour roles, and text colour | Visual pattern review; colour system and accessibility |
| R37–R44 | Relevant imagery, illustration, material quality, technical imagery, visible media, artifacts, icons, and product views | Visual pattern review; SVG graphics; design-system rules |
| R45–R54 | Entrances, visible defaults, hover, real activity, timing, scroll, marquees, cursor behaviour, animation cost, and reduced motion | Motion pattern review; motion interaction; animation runtime performance; scroll feedback |
| R55–R62 | Specific copy, cadence, action names, supported claims, real urgency, marked examples, useful help, and repeated information | Copy pattern review; interface copy |
| R63–R77 | Working flows, relevant states, forms, feedback, useful data, contrast, semantics, overflow, mobile, system consistency, interruption, browser behaviour, delivery, control semantics, and locale | UI quality gates; focused component, form, state, table, accessibility, responsive, performance, and i18n references |
| R78–R84 | Brief authority, task fit, preservation, house-style awareness, evidenced findings, rendered verification, and translated references | UI anti-patterns; design review; redesign preservation; design planning core |

R21 keeps image/text alternation when it improves the reading path, including
short, clearly grouped comparisons. R24 binds headings spatially to the content
they introduce; dense rows still need an identifiable relation. Both rules are
at the layout owner rather than repeated in every command.

The runtime modules are [UI anti-patterns](https://github.com/sebastian-software/skills.sebastian-software.com/blob/main/skills/effective-web/references/ui-antipatterns.md),
[layout](https://github.com/sebastian-software/skills.sebastian-software.com/blob/main/skills/effective-web/references/ui-pattern-layout.md),
[visual](https://github.com/sebastian-software/skills.sebastian-software.com/blob/main/skills/effective-web/references/ui-pattern-visual.md),
[motion](https://github.com/sebastian-software/skills.sebastian-software.com/blob/main/skills/effective-web/references/ui-pattern-motion.md),
and [copy](https://github.com/sebastian-software/skills.sebastian-software.com/blob/main/skills/effective-web/references/ui-pattern-copy.md).
The links identify their maintained destinations, integrated in Skill Library
PR 279 and refined in PR 280.

## Conflicts resolved in Impeccino

The previous craft floor, new-work calibration, finish reviewer, and documenter
contained categorical style instructions that contradicted the accepted
exceptions. Those instructions no longer override functional labels, deliberate
component relationships, accepted fonts and palettes, readable grey, suitable
vector media, or purposeful image hover. Distill preserves meaningful colour
and type roles; loading feedback is selected for the actual operation.

Observed accessibility and functional defects remain reportable, including
when a recorded visual decision conflicts with the repair. Style warnings need
context and evidence. That initial knowledge change (PR 75) left the detector's matchers, severity
metadata, hook policy, and exit statuses unchanged. The production reporting
boundary below records the remaining engine scope in issue 74.

Native-platform material remains pending curation. The task-contract migration
below records the later disposition of command-specific web teaching.

## Source record

These are related sources considered in the review, not runtime dependencies or
imported skill packages. The shipped guidance is a curated synthesis; source
blacklists, absolute bans, fixed values, and contradictory recommendations were
not adopted automatically. Only DeleteSlop's publicly visible material was
considered. The Impeccable design catalog is outside this rule curation.

- [rwcod: Anti-Slop Patterns](https://github.com/rwcod/anti-ai-slop-ui/blob/main/references/anti_slop_patterns.md)
- [Anti-Slop Atlas](https://github.com/phcodesage/anti-slop-atlas#the-slop-tells)
- [ch040602: Anti-AI Slop](https://github.com/ch040602/anti-ai-slop/blob/main/SKILL.md)
- [Anthropic: frontend-design](https://github.com/anthropics/skills/blob/main/skills/frontend-design/SKILL.md)
- [Unslop UI](https://github.com/claudiusararu/unslop-ui-skill/blob/main/TELLS.md)
- [muris11: antislop-ui](https://github.com/muris11/anti-ai-slop/blob/main/skills/antislop-ui/SKILL.md)
- [Krirox: anti-ai-slop-skills](https://github.com/Krirox/anti-ai-slop-skills/blob/main/SKILL.md)
- [Taste Skill v1](https://github.com/Leonxlnx/taste-skill/blob/main/skills/taste-skill-v1/SKILL.md)
- [agshinrajabov: no-slop-design](https://github.com/agshinrajabov/no-slop-design/blob/main/SKILL.md)
- [DeleteSlop: No Slop Design](https://www.deleteslop.com/skills/no-slop-design)
- [Impeccino before this change](https://github.com/sebastian-software/impeccino/tree/66f902d735dc8f9bee19a42f1f99c38b5aaff47a/skill)

The Library baseline was commit `2c021db985bbdf44248c33a413e7ce1b37cafa18`.
Existing local changes to its design-system rules, deep layout appendix, and
design-review route were preserved.

## Task-contract migration

The remaining source was reviewed at Impeccino commit
`5e1cccc6e32e062d6e0729f9870ee6eac0b2aeb6`. The command references below keep
purpose, inputs, scope, evidence, and completion contracts. Their general web
teaching is maintained in the existing `effective-web` references named here;
the host can choose another suitable source. This disposition does not import
the old prose or make those references a runtime dependency.

| Impeccino source | Knowledge disposition | Impeccino contract retained |
| --- | --- | --- |
| `critique.md`, `polish.md` | `change-scoped-interface-review.md`, `cognitive-ux.md`, `ui-quality-gates.md`, and the four pattern modules | Target and recorded decisions; independent review when available; chat findings with evidence; no invented numerical score |
| `adapt.md`, `layout.md` | `responsive-design.md`, `css-layout-responsive.md`, `layout-foundations.md`, `layout-spacing.md`, `print-web-layout.md` | Preserve the primary task across target contexts; verify real touch gestures and report missing input coverage |
| `typeset.md`, `colorize.md` | `typography-system.md`, `typography-detail.md`, `colour-system.md`, `colour-accessibility.md` | Preserve incumbent tokens and roles; verify actual readability, states, themes, and fallback |
| `animate.md`, `delight.md`, `overdrive.md` | `motion-interaction.md`, `animation-runtime-performance.md`, `ui-pattern-motion.md`, `interface-copy.md` | Identify the requested interaction; reduced-motion alternatives, interruption, usable fallback, performance evidence |
| `bolder.md`, `quieter.md`, `distill.md`, `operate.md`, `new-work.md` | `design-planning-core.md`, `redesign-preservation.md`, `ui-pattern-layout.md`, `ui-pattern-visual.md` | Refinement scope versus identity replacement; per-surface mode; preserve content, function, and established identity |
| `clarify.md`, `onboard.md` | `interface-copy.md`, `cognitive-ux.md`, `forms-and-state.md`, `loading-states.md`, `design-planning.md` | Real first-use tasks, recovery and relevant states; preserve factual claims and reachable capabilities |
| `harden.md`, `audit.md` | `ui-quality-gates.md`, `html-accessibility.md`, `accessibility-testing.md`, `forms-and-state.md`, `i18n-ux.md`, `i18n-rtl.md` | Evidence-based findings, supported ranges, interrupted gestures, no screenshot-only interaction verdict |
| `optimize.md` | `browser-performance.md`, `react-performance-priorities.md`, `animation-runtime-performance.md` | Comparable before/after evidence; actual bottleneck; preserve behavior and report measurement limits |
| `extract.md`, `document.md` | `design-system-rules.md`, `component-api-design.md` | Extraction boundaries, consumers, actual reused tokens; DESIGN.md format, detector metadata, and waiver preservation remain local integration contracts |
| `init.md`, `shape.md` | `design-planning.md`, `design-planning-core.md` | Useful discovery for material gaps; supported product facts, explicit assumptions, smallest useful brief |

The fixed interview rounds, compulsory confirmation, candidate tournaments,
standing-exit ceremony, six-block brief requirement, unconditional reviewer
handoffs, and fixed inspection ceilings are retired workflow prescriptions.
They are not migrated as design knowledge. Existing answers and authorization
remain effective. Host instructions control questions, delegation, autonomy,
and verification budget. Fictional persona panels and heuristic score bands
are retired as unevidenced assessment machinery; audience and task review
remain useful. Framework recipes and universal numerical style thresholds are
not copied into task contracts; their appropriate use belongs to the maintained
specialist knowledge.

The hook reference retains manifests, tiers, cache and waiver contracts while
removing its former blanket approval gate; the host owns the write boundary.

Existing PRODUCT.md headings and schema stamp, DESIGN.md tokens and waivers,
DESIGN.json schema, SURFACES.md markers, and legacy direction blocks remain
readable. A roll key is evidence only when a roll actually ran. Development
briefs stay out of browser-delivered artifacts.

### Creative source candidates and native material

`concept-seed` keeps its local hash, supported scopes, 5–7 candidate count,
three-index surface deal, re-rolls, and register behavior. Its lead excludes
the first two entries; this is a selection bias, not a quality measure. The
helper cannot see the candidate list. Repeatability requires retaining that
ordered list and the key, count, scope, and flags. Using the helper is optional.

The local palette catalog's color tuples, IDs, weighting, mood examples, and
strategies remain source candidates. Its former mandatory color-space,
background, dosage, and contrast recipes are retired; actual contrast and
semantic roles need evaluation in the implemented surface. The remote
Impeccable catalog was removed in ADR 0019 and is unavailable for this curation;
no catalog content is imported or treated as reviewed knowledge.

`ios.md`, `android.md`, `adapt.native.md`, and the native diagnostic material in
`audit.native.md` remain available and explicitly marked as pending curation.
They are retained candidates, not a completed native knowledge migration.

### Production reporting boundary

Raw matchers and frozen function-level oracle vectors retain their legacy
contract. Production CLI and hook reports apply a shared interpretation policy:
`broken-image`, `script-error`, `low-contrast`, `text-occlusion`, `text-overflow`,
and `first-viewport-column-overflow` remain primary. The other built-in pattern,
declared-value drift, and threshold/structural-risk findings are advisory. This
phase does not claim deeper specification conformance or complete accessibility
coverage. External rule packs retain ownership of their own finding policy.

Advisories alone exit successfully and never deny a Cursor write. Hooks retain
contextual advice; Codex Stop suppresses it because that event offers only a
blocking result. `--no-advisory` hides contextual findings, while operational
failures and primary diagnostics keep their existing exit semantics. Waivers,
inline ignores, project boundaries, and native scan exclusions remain effective.

Issue #86 subsequently consolidates detector metadata into DESIGN.md. Legacy
DESIGN.json remains a read-compatible input until explicit, verified migration;
[ADR 0020](adr/0020-project-state-is-top-level-files.md) owns the current artifact
layout.
