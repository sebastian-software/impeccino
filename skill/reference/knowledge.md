# Design knowledge

Before judging or changing a surface, look for know-how that is already
available instead of starting from recall alone. Projects and users often
install skills, references, and guidelines for design work, and these carry
decisions and checks that general knowledge misses.

## Look before you start

Once the command and its target are clear, name the topics the work touches,
then search for matching sources:

- **Where:** skills the host lists for this session, skills installed in the
  project or for the user, and resources that the project's own instructions or
  docs point to, such as a design-system or style guide.
- **What:** a source whose name or description covers a topic you are about to
  judge. Read its own index or router and open only the parts that apply; do
  not load a whole library.
- **Order:** project sources speak for this codebase and come before user-wide
  ones. Recorded project decisions outrank both: the brief, PRODUCT.md,
  DESIGN.md with its waivers, the surface brief, and project ADRs.

Search once per session and reuse what you found for later commands.

## Topics to search for

Let the work suggest the topic. Typical pairs:

| When you check or change | Look for knowledge on |
| --- | --- |
| Overall quality, generic or templated output | design review, UI anti-patterns |
| A new surface or a new direction | design planning, visual direction |
| Type hierarchy, measure, fonts | typography |
| Color, contrast, themes | color systems, accessibility |
| Grids, spacing, responsive behavior | layout, responsive design |
| Transitions, feedback, effects | motion, interaction |
| Labels, errors, empty states, persuasive copy | interface copy, copywriting |
| Forms, states, localization | forms, error and loading states, internationalization |
| Semantics, focus, keyboard use | accessibility |
| Speed, loading, animation cost | web performance |
| Tokens, components, a design system | design systems, CSS architecture |

The list is a prompt, not a limit: a printed profile calls for print design, a
data grid for tables. Native platforms keep their platform references and add
sources only for concerns they share, such as copy and states.

## Use and report

A knowledge source informs judgment. It never overturns a recorded decision as
a style correction, and an observed functional or accessibility defect is still
reported, as the playbooks require. Pass the files you found to a shipped role
together with its other inputs.

Name the sources you used in the report's coverage section or the handoff.
When nothing qualifies, continue with general knowledge and say so; a missing
source never blocks the work.
