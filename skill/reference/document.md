Generate a `DESIGN.md` file at the project root that captures the current visual design system, so AI agents generating new screens stay on-brand.

DESIGN.md follows the [official DESIGN.md format spec](https://raw.githubusercontent.com/google-labs-code/design.md/main/docs/spec.md): optional YAML frontmatter carrying machine-readable design tokens, followed by up to eight markdown sections in a fixed order. **Tokens are normative; prose provides context for how to apply them.** Sections may be omitted when not relevant, but those present stay in the specified order. Use the canonical headings below so the file remains portable across DESIGN.md-aware tools.

## The frontmatter: token schema

The YAML frontmatter is the machine-readable layer. It's what Stitch's linter validates and what later agents and tools read. Keep it tight; every entry should correspond to a token the project actually uses.

```yaml
---
name: <project title>
description: <one-line tagline>
colors:
  primary: "#b8422e"
  neutral-bg: "#faf7f2"
  # ...one entry per extracted color; key = descriptive slug
typography:
  display:
    fontFamily: "Cormorant Garamond, Georgia, serif"
    fontSize: "clamp(2.5rem, 7vw, 4.5rem)"
    fontWeight: 300
    lineHeight: 1
    letterSpacing: "normal"
  body:
    # ...
rounded:
  sm: "4px"
  md: "8px"
spacing:
  sm: "8px"
  md: "16px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.neutral-bg}"
    rounded: "{rounded.sm}"
    padding: "16px 48px"
  button-primary-hover:
    backgroundColor: "{colors.primary-deep}"
---
```

Rules that matter:

- **Token refs** use `{path.to.token}` (e.g. `{colors.primary}`, `{rounded.md}`). Components may reference primitives; primitives may not reference each other.
- **Colors accept any valid CSS color string.** Hex is the recommended default for portability, but preserve an incumbent `rgb()`, `hsl()`, `oklch()`, wide-gamut, or mixed-color value when it is the project's normative source. Never split the source of truth without explicit reason.
- **Component sub-tokens** are limited to 8 props: `backgroundColor`, `textColor`, `typography`, `rounded`, `padding`, `size`, `height`, `width`. Shadows, motion, focus rings, backdrop-filter: none of those fit. Describe them in the body; actual shadow values can also use the detector metadata block below.
- **Scale keys are open-ended.** Use whatever names the project already uses (`oxblood-deep`, `surface-container-low`). Don't rename to Material defaults.
- **Variants are naming convention, not schema.** `button-primary` / `button-primary-hover` / `button-primary-active` as sibling keys.

## The markdown body: eight sections (canonical order)

1. `## Overview`
2. `## Colors`
3. `## Typography`
4. `## Layout`
5. `## Elevation & Depth`
6. `## Shapes`
7. `## Components`
8. `## Do's and Don'ts`

Omit irrelevant sections rather than filling them with invented rules. Put responsive layout in Layout, depth in Elevation & Depth, radius and form language in Shapes, and per-component behavior in Components. Unknown sections are preserved by the format, but new visual guidance should use the canonical structure whenever it fits.

## Scope and evidence

Record the actual reusable visual system from the resolved project or app.
Read existing DESIGN.md and any legacy DESIGN.json before updating. Preserve
legacy metadata through the migration below; do not create a new sidecar.
A document request authorizes recording within its scope; it does not authorize replacing an
incumbent identity or unrelated decisions. Ask only when a material boundary
or fact remains unclear. Use the design-system knowledge found through
[knowledge.md](knowledge.md) rather than inventing a metaphor or mandatory naming workshop.

## Scan mode

Inspect the project's token source, stylesheets, theme configuration, reusable
components, and relevant rendered evidence. Reuse actual names, formats, roles,
states, layout behavior, and component APIs. Document durable reused decisions;
one-off values are not automatically system rules. Do not remap an incumbent
system to a generic role taxonomy or fabricate missing components and tokens.

Stage frontmatter from observed primitives. Component variants use the eight
supported properties above; prose describes behavior that the token format
cannot express. Named Rules are optional when an actual invariant needs one.
Keep source values authoritative instead of restating them differently in prose.
A style advisory does not erase a justified system choice. Broken behavior,
unsupported claims, and accessibility defects are findings, not new system rules.

### Preservation

**Preserve the waivers.** When rewriting DESIGN.md, carry every such comment
over verbatim and keep it next to the Named Rule or Do/Don't that justifies it.
This includes `<!-- impeccino-disable <rule-id>: <reason> -->` comments, which
are detector metadata only. Preserve declared fonts, colors, radii, and sizes
outside an authorized removal or replacement. If surrounding prose changes,
retain the waiver and explain its existing decision without inventing approval.
Do not add a waiver or token simply to hide a measured defect.

Write only supported facts using the canonical headings above, omitting
irrelevant sections. Keep surface composition and mode in that surface's
SURFACES.md section, not as global DESIGN.md rules. Apply [new-work.md](new-work.md)
when an explicitly requested identity replacement needs direction.

### Keep detector metadata in DESIGN.md

The engine reads these optional fields in a marked JSON block inside DESIGN.md: `extensions.colorMeta.<token>.canonical`, `extensions.colorMeta.<token>.tonalRamp`, `extensions.roundedMeta.<token>` (a string/number or its `canonical`, `value`, `values`, `aliases`, and `role` metadata), and `extensions.shadows[].value`. Do not generate component snippets, narrative, motion, breakpoints, display names, or other fields the engine does not read.

Include the block only when actual tokens need these extra details. Put it at the end of DESIGN.md, outside any example fence. The marker must be on its own line immediately before the JSON fence. There may be only one marked block. Keep the frontmatter authoritative for its token primitives; metadata preserves source color values or tonal steps, radius details, and actual shadow values.

Example block (copy its contents without the outer Markdown example fence):

````markdown
<!-- impeccino:design-metadata -->
```json
{
  "schemaVersion": 2,
  "extensions": {
    "colorMeta": {
      "primary": {
        "canonical": "oklch(60% 0.25 350)",
        "tonalRamp": ["oklch(40% 0.2 350)", "oklch(60% 0.25 350)"]
      }
    },
    "roundedMeta": {
      "pill": { "canonical": "999px", "role": "pill" }
    },
    "shadows": [
      { "value": "0 4px 24px rgba(0,0,0,0.12)" }
    ]
  }
}
```
````

For a legacy DESIGN.json next to the resolved DESIGN.md, `doctor --fix` moves the entire JSON object into this block and removes the sidecar only after verifying the replacement. It preserves Markdown, waivers, and unknown metadata fields. Use that mechanical migration within the authorized write boundary before updating the record. If both records contain different metadata or either is malformed, retain both and resolve the reported conflict; never merge by guessing. The engine reads legacy JSON only when DESIGN.md has no marked block. A malformed block never triggers fallback.

A metadata-only refresh updates this block while preserving the rest of DESIGN.md. For new metadata, use `schemaVersion: 2` and an `extensions` object. Do not add an empty block when no extra metadata is needed. Preserve migrated fields that the current engine does not interpret unless their removal is authorized.

## Seed mode

`/impeccino document --seed` records a provisional direction before implementation.
The post-build documenter uses scan mode after implementation; it does not run
this standalone seed path. Resolve product facts with [init.md](init.md) when
needed and reuse the chosen direction from [new-work.md](new-work.md). Ask only
about material gaps. Seed mode does not replace coherent incumbent code.

Use the canonical section order. Mark unresolved implementation facts as
provisional, omit nonexistent components, and keep the first surface's
composition in its surface brief. Write minimal frontmatter with `name` and
`description` only when real tokens do not yet exist. Omit the detector metadata
block until actual detector metadata is available. Keep a recognizable seed marker:

```markdown
<!-- SEED: provisional direction before implementation; re-run /impeccino document once there's code to capture the actual tokens and components. -->
```

Existing seed markers and records remain valid. Rerun scan mode after code
exists to record actual tokens and any needed metadata.

## Completion

Check values against their actual sources, preserve scope and waivers, and
confirm canonical section order and metadata fields. Return paths written, the
system facts captured, and unresolved or unverified decisions. If the record
already matches, leave it unchanged and report what was checked. A metadata-only
refresh preserves all other DESIGN.md content. Unrelated drift is reported
rather than repaired.
