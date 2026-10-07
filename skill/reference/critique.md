# Critique

Review the requested UX and visual decisions without editing implementation.

Resolve the target and read recorded authority: PRODUCT.md (Brand Commitments,
Product Principles), DESIGN.md (Named Rules, Do's and Don'ts, waivers), the
surface brief, and project ADRs (`docs/adr/`, `doc/adr/`, `adr/`) when relevant.
Use PRODUCT.md’s `## Users` to ground audience and tasks; fictional personas
and numerical heuristic scores are not evidence. A style warning that contradicts
a recorded decision is dropped as a proposed correction. Still report observed
accessibility or functional defects and explain any conflicting decision.

Inspect the experience before detector output when possible to reduce anchoring.
Use independent assessments only when the host permits and the task benefits;
report when the review is local rather than independent. Select suitable review
knowledge available to the host.

## Mechanical scan (web only)

Native platforms skip the web-only detector; use relevant platform captures.
Scan local source paths with `detect --json <local source paths>` and rendered
web URLs with `detect --viewport <W>x<H> <url>` when that evidence is relevant.
A source file is not a rendered-page target. On web targets, rerun the scan;
on native targets, recheck the device captures when confirming a correction.
Validate screenshots and exercise interactions; report unavailable coverage.
Stop servers started solely for this review unless asked to keep them.

## Report

Return actionable findings in chat; nothing is archived. Name target, audience,
what works, prioritized findings, evidence and user consequence, concrete
recommendations, and coverage limits. Separate measured defects from advisory
patterns; an unjustified cluster needs its owning decision corrected. Do not
claim overall accessibility or design quality from a clean scan.

Ask a targeted question only when an unresolved finding needs a material choice.
When the user rejects a finding as deliberate, reuse that decision. Record it
only within the requested documentation scope: product or brand intent goes
into PRODUCT.md; a visual rule goes into DESIGN.md, with an existing or justified
`impeccino-disable <rule-id>` waiver when appropriate. Do not invent approval.
Suggested follow-ups can include polish, audit, clarify, adapt, harden, optimize,
`/impeccino extract`, or a focused visual command; critique alone does not authorize edits.

Apply [craft-floor.md](craft-floor.md) before implementation. Use the resolved
target and existing answers; ask only about material gaps through the host's
structured question tool when available. These contracts follow
the host’s authorization, questions, delegation, and verification budget.
