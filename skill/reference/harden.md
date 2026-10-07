# Harden

Repair resilience problems in the requested production UI path.

Establish supported inputs, content ranges, states, permissions, locales, and
runtime conditions from the real product. Use suitable forms, accessibility,
i18n, and performance knowledge. Fix the owner of a confirmed failure while
preserving valid behavior, identity, and API contracts. Do not add speculative
features or silently swallow errors.

### Edge Cases & Boundary Conditions

Exercise the task’s relevant empty, loading, error, success, concurrent,
permission, long-text, offline, and recovery states. For custom controls:

**Interrupted gestures**: test cancellation, lost capture, release outside the
control, focus loss, and a second pointer. On cancellation, clear the dragging
state and release capture; the next tap or drag works without a reload.

### Evidence

Use real input or the project’s behavioral test tools. Screenshots establish
layout, not successful touch behavior. Report the tested method and engine,
and any unsupported environment or input coverage.

## Verify Hardening

Reproduce the original failure and confirm valid paths still work. Include
**Interrupted gestures** when a gesture changed. Add a meaningful regression
when the project can drive the affected behavior. Report remaining risk and
verification limits instead of creating a broad unrelated hardening checklist.

Apply [craft-floor.md](craft-floor.md) before implementation. Use the resolved
target and existing answers; ask only about material gaps through the host's
structured question tool when available. These contracts follow
the host’s authorization, questions, delegation, and verification budget.
