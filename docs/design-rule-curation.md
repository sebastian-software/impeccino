# Design rule curation

On 6 October 2026, 84 deduplicated design-review points were accepted for the
Skill Library. The acceptance included their contextual exceptions. 82 points
had related material in at least two source projects; R21 and R24 were accepted
individually. Each project counted once, including Impeccino. These counts
describe source overlap, not independent votes or proof of universal validity.

The maintained knowledge owner is `effective-web` in the Skill Library.
Impeccino keeps task entry points, context, measurements, and project-file
contracts. Its workflows can use suitable knowledge available to the host;
the Library is not a mandatory runtime dependency or an exclusive source.
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
The links identify their maintained destination; additions become available on
the default branch after integration.

## Conflicts resolved in Impeccino

The previous craft floor, new-work calibration, finish reviewer, and documenter
contained categorical style instructions that contradicted the accepted
exceptions. Those instructions no longer override functional labels, deliberate
component relationships, accepted fonts and palettes, readable grey, suitable
vector media, or purposeful image hover. Distill preserves meaningful colour
and type roles; loading feedback is selected for the actual operation.

Observed accessibility and functional defects remain reportable, including
when a recorded visual decision conflicts with the repair. Style warnings need
context and evidence. The detector's matchers, severity metadata, hook policy,
and exit statuses are unchanged by this knowledge change; measured-policy work
belongs to the separate engine scope in issue 74.

Native-platform material and the remaining command-specific teaching guidance
are not fully curated by this change. This record covers the accepted review
points and conflicting instructions, not the whole migration in issue 74.

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
