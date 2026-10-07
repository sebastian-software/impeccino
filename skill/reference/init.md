# Init flow

`init` captures durable product truth in PRODUCT.md. It does not invent a visual world and does not write DESIGN.md; [new-work.md](new-work.md) creates or expands one, and [document.md](document.md) records an incumbent one.

## Step 1: Load current state

Use the PRODUCT.md path resolved by `impeccino context`. Update it instead of creating a competing authority. In a child app inheriting root context, confirm shared versus app-specific scope before writing.

- **No PRODUCT.md:** explore, resolve material gaps, and record supported context.
- **PRODUCT.md exists:** update the requested stale or missing product knowledge; do not reopen confirmed fields without a reason.
- **Legacy PRODUCT.md:** add only durable missing facts; absent `## Platform` means `web` unless evidence says otherwise.
- **Only DESIGN.md exists:** leave it untouched and create PRODUCT.md.
- **Redesign/rebrand request:** preserve confirmed product truth unless the user changes it. Visual replacement happens later in new-work, not here.

Preserve existing facts outside the requested change. Do not offer DESIGN.md merely because it is missing. If another request invoked init, record the relevant context and resume it. New visual work continues in new-work; `shape` resumes its planning task.

## Step 2: Explore the project

Before asking, scan enough to avoid making the user repeat known facts: product docs and copy; package/config and app boundaries; features, workflows, routes, and roles; names, logos, legal/proof assets, and brand commitments; platform/accessibility signals; and the dev command and entry point.

Treat repository evidence as a hypothesis, not user approval. Note visual maturity without documenting, extending, or replacing the world.

Form a platform hypothesis: `web`, `ios`, `android`, or `adaptive` (one product that genuinely adapts its design language per OS). Mobile web remains `web`; a native wrapper around a website does not make its design language native.

## Step 3: Resolve product facts

Reuse the explicit brief, existing answers, and repository evidence. Ask only
about material gaps that cannot be inferred, through the host's structured
question tool when available. Useful discovery covers the primary users and
jobs, product mechanism, durable constraints, real proof, and uncertain platform.
A clear brief does not require an answer or approval round before writing.
Follow the host's autonomy policy; distinguish supported facts, hypotheses, and
open decisions. Never interpret a tool's presence as permission to override it.

An existing stack is evidence. Choose routine implementation details within
scope when delegated; ask about framework or deployment only if the answer
would materially change the result. Do not turn implementation preferences
into invented product commitments. Init captures product truth, not palettes,
typography, page concepts, or a compulsory aesthetic interview.

### What belongs here

- users, jobs, workflows, purpose, success, positioning, and operating context;
- capabilities, constraints, terminology, evidence, platform, and accessibility;
- confirmed voice, assets, and brand commitments.

### What does not belong here

- visual worlds, palettes, typography, components, or page concepts;
- visitor mode, narrative, CTA/proof sequence, or other surface strategy;
- invented testimonials, customers, benchmarks, pricing, licensing, or deployment claims;
- a requirement to decide every optional field.

## Step 4: Write PRODUCT.md

Write supported facts, clearly labeled assumptions, and open decisions. Omit irrelevant sections rather than filling them with generic prose.

```markdown
# Product

<!-- impeccino:product-schema 1 -->

## Platform

web

## Stack
[Greenfield only: a specified or delegated stack, with its basis. Omit when undecided or an existing codebase already answers it.]

## Users
[Primary users, their situation, and job. Add other audiences only when confirmed.]

## Product Purpose
[What the product does, why it exists, and what success means.]

## Positioning
[The product mechanism or claim a neighboring product could not truthfully copy.]

## Operating Context
[Workflows, environments, tools, documents, materials, and rituals that are factual parts of using or evaluating the product.]

## Capabilities and Constraints
[Confirmed functionality, technical constraints, terminology, and explicitly undecided product facts.]

## Brand Commitments
[Existing name, voice, assets, personality, identity constraints, and references the user explicitly made binding. Omit when none exist.]

## Evidence on Hand
[Real content, data, demonstrations, testimonials, case studies, press, or assets, with paths where applicable. State absences that future work must not fabricate.]

## Product Principles
[Relevant durable strategic principles supported by the brief or product evidence; no visual recipes.]

## Accessibility & Inclusion
[Known user needs or required standard. Omit when no product-specific requirement was established.]
```

Platform is the bare value `web`, `ios`, `android`, or `adaptive`. Preserve useful legacy headings. New files go at `PROJECT_ROOT/PRODUCT.md`; otherwise update the resolved file. Record it when the task calls for durable product context; planning need not block on a file write.

Copy the `impeccino:product-schema` comment verbatim, including when you update an older file. It records which version of the product record this file follows, so later versions can tell a deliberately short record from one written before a section existed, and never propose an interview the user has already sat through. Update the number only when this reference's template changes it. Sections a later version retires are reported to you at boot as deprecated; remove them only within the requested context-maintenance scope.

When the platform you just recorded is `ios`, `android`, or `adaptive`, load [ios.md](ios.md), [android.md](android.md), or both before any design work. On a project that had no PRODUCT.md, `impeccino context` could not know the platform and so never loaded them; init is the only place that learns the answer.

### Completion

When init is asked to write context, verify the record exists at the resolved
path with supported facts, explicit assumptions, and the schema marker. A
planning-only request can return its facts and gaps without writing. Reuse the
record and resume the original task within its existing authorization.

## Step 5: Wrap up or resume

Summarize captured and deliberately undecided facts. Do not offer DESIGN.md merely because it is missing.

Recommend the next action from the actual project state:

- Empty or early project: ask naturally for the surface to be built, or use `/impeccino shape <surface>` when the user wants a confirmed brief without implementation. New-work will establish a visual world only when the requested work needs one.
- Existing coherent interface without DESIGN.md: `/impeccino document` if the user wants the incumbent system recorded independently of a new build.
- Existing surface needing work: name the most relevant scoped command.

If init was invoked by another request, resume without rerunning `impeccino context`; the native reference above is the one thing that run could not have given you, and new-work owns later visual decisions.
