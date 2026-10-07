# New visual work

Build a new surface or replace a visual identity within the requested scope.
PRODUCT.md owns product facts; DESIGN.md owns durable visual decisions; each
marked section of SURFACES.md owns one surface's mode and strategy. Select
suitable design knowledge available to the host. No external skill is required.

## 1. Decide what is already true

Read the resolved context, representative code, tokens, components, and assets.
A missing DESIGN.md does not erase an established identity. A local extension
inherits that identity and its surrounding behavior. A redesign preserves
product truth, content, function, native affordances, and technical constraints
while replacing the requested visual system. An incomplete brand preserves its
confirmed assets. Where no incumbent visual authority exists, invent within the brief.

Use [init.md](init.md) for missing durable product context and [document.md](document.md)
for an incumbent system that needs recording. Reuse available evidence and
answers; a narrow task can proceed without first creating either file.

## 2. Resolve material gaps

Establish the audience, primary task or action, real content, relevant states,
constraints, and untouched areas. Ask through the host's structured question
tool only when a missing answer materially changes the work and cannot be
inferred. A precise brief does not require confirmation or an interview.
The host decides whether to ask, proceed with stated assumptions, or delegate.

## 3. Choose the amount of invention

Refinement preserves the incumbent world; a new surface may need a composition;
an explicit redesign may need a replacement identity. Use suitable knowledge
for the actual job, including Operate and Read guidance in [operate.md](operate.md).
Honor specified directions and existing answers. Offer alternatives when they
help resolve a real choice; there is no fixed candidate count or choice ritual.

The optional local helper accepts an ordered list of five to seven candidates:

`"<skill-base-dir>/scripts/impeccino" concept-seed --scope <direction|surface> --mode <mode> --candidate-count <count>`

It returns a suggested index, or three dealt indices for surface scope. It
cannot read the list or assess design quality, and its lead excludes the first
two entries. Keep the same ordered list, count, key, scope, and flags to reproduce
a roll with `--from <key>`. `--reroll <n>` changes selection; direction re-rolls
also accept `--register safer|bolder`. Safer prints no assignment. These flags
do not authorize implementation, demand new candidates, or override the brief.

## 4. Commit to the requested result

Keep product facts supported and demonstration data labeled synthetic. Build
with the project's conventions and the selected direction, preserving behavior,
semantics, accessibility, responsiveness, and performance. A seed or catalog
sample supplies neither facts nor permission to replace an incumbent system.

## 5. Record the decision

When persistence is useful and within scope, keep the development-only contract
under `### Direction contract` in the relevant surface brief, that surface's
section of the project's `SURFACES.md`. Record scope, visitor mode, decisions,
constraints, and unresolved facts without duplicating global tokens or product
truth. Existing THESIS, OWN-WORLD, STORY, FIRST VIEWPORT, FORM, and FINISH blocks
remain readable; no six-block template is required for new work. Record the
printed seed key and ordered candidate list only when a concept roll ran.

`"<skill-base-dir>/scripts/impeccino" surface-brief read <primary-target>`

`"<skill-base-dir>/scripts/impeccino" surface-brief write <primary-target> <body-file> [related-target ...]`

The body file contains section prose only, with subheadings at `###` or deeper.
The helper writes the heading and marker, replaces exactly this surface's
section of `SURFACES.md`, and leaves other sections untouched. After writing,
read the brief once more to verify scope and retained decisions. Shape returns
a brief without persisting or implementing unless separately requested.

Never copy the direction contract into implementation source or any browser-delivered artifact:
HTML or framework comments, hidden DOM, `<template>` content, `data-*` attributes,
serialized props or state, React Server Component payloads, client bundles,
metadata or JSON-LD, or accessibility-only text. It belongs in development context.

## 6. Verify and finish

Inspect the changed task or reading path at relevant sizes, states, and input
methods. For web layouts, distinguish source scans from rendered URL evidence:
use `detect --json <changed files>` when hooks have not already supplied that
source evidence, and `detect --viewport <W>x<H> <url>` for layout-dependent
checks. Native platforms skip this HTML/CSS detector; use platform captures and
input tools. Validate captures before drawing conclusions and report gaps.

When a finish review is part of the requested workflow, use the shipped reviewer
under SKILL.md's host-permitted role routing. Supply the original request,
existing answers, artifact and context paths, any surface brief, detector results,
and valid captures with their exact paths and required viewport set. Review is
read-only; a local review is not independent. Repair supported material findings
within scope, verify the repairs, and report the actual verdict and its limits.

When recording a new world or requested system change, use the documenter or
[document.md](document.md) after implementation. Its write boundary comes from
the task's existing authorization, not from this reference. New system records
include token-bearing DESIGN.md, with any needed detector metadata in its marked
block; ordinary extensions preserve
the incumbent record and report unrelated drift. Stop when the requested outcome
and required checks pass, under the host's workflow and budget.
