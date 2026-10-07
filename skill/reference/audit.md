# Audit

Review technical quality in the requested scope without editing implementation.
Read PRODUCT.md, DESIGN.md, the surface brief, and relevant ADRs. Respect
recorded style decisions; report observed accessibility or functional defects
even when a recorded decision conflicts. Select suitable audit knowledge
available to the host and establish the actual platform and support policy.
Native platforms use audit.native.md and skip the HTML/CSS detector.

## Diagnostic Scan

Use the project’s checks, relevant source, rendered pages, and real input to
assess the affected dimensions. Reuse existing hook evidence; do not duplicate
source scans. Keep local source paths separate from rendered URL targets.

### 1. Accessibility

Inspect semantics, names, keyboard/focus, actual contrast, text scaling, and
motion preferences relevant to the changed path. State manual or assistive
coverage separately from automated results.

### 2. Performance

Identify the task and environment, use actual measurements, and distinguish
observed cost from a hypothesis. Source heuristics alone prove no runtime result.

### 3. Theming

Check affected tokens, themes, states, and their rendered readability. Declared
value drift is contextual advice, not proof that an intentional addition is wrong.

### 4. Responsive Design

Inspect relevant sizes, continuous resizing, long content, zoom, and input modes.
**Broken touch interaction** requires gesture evidence. Exercise the gesture
when a browser tool can synthesize touch; include interruption and scroll across
custom controls. Report the input method and browser engine, and what stayed
untested. A screenshot or viewport emulation alone does not prove touch behavior.

### 5. Implementation Integrity

Check truthful content, working primary paths, relevant states, and usable
fallbacks. Separate measured defects from pattern and threshold/risk advisories;
an unjustified generic cluster needs its owning decision examined.

## Generate Report

### Audit Health

Name each assessed dimension as a measured defect, contextual risk, acceptable
within the inspected scope, or unverified. State the method and limits rather
than a numerical health score. A clean scan is not a comprehensive verdict.

### Implementation Integrity

Explain any unsupported claim, missing capability, broken path, or conflict
with recorded decisions. Distinguish observed consequences from assumptions.

### Findings and Actions

Prioritize by user consequence. Each finding names target/location, evidence,
impact, owning decision, and a concrete correction. Include useful strengths and
systemic patterns without treating every familiar style as a defect.

### Coverage

List the paths, viewports/device classes, states, input methods, and tools
actually checked, plus unavailable evidence. Report source versus rendered or
native evidence separately. Screenshots do not verify gestures or complete
accessibility coverage.

## Recommended Actions

Suggest the command that fits each actual finding, such as `/impeccino polish`,
`/impeccino harden`, `/impeccino optimize`, `/impeccino adapt`, or
`/impeccino extract`. The Commands table contains the complete task menu.
An audit alone does not authorize implementation or a documentation rewrite.
Return the report in chat, under the host’s workflow and question policy.
