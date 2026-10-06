import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

describe('skill reference authoring contracts', () => {
  it('keeps direction contracts in development-only surface briefs', () => {
    const newWork = readFileSync(join(ROOT, 'skill/reference/new-work.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const recordDecision = newWork.match(/## 5\. Record the decision\n([\s\S]*?)\n## 6\./)?.[1] ?? '';

    assert.match(recordDecision, /development-only contract/);
    assert.match(recordDecision, /under `### Direction contract` in the relevant surface brief, that surface's section of the project's `SURFACES.md`/);
    assert.match(recordDecision, /replaces exactly this surface's section of `SURFACES.md`/);
    assert.match(recordDecision, /read the brief once more/i);
    assert.match(recordDecision, /all six contract blocks/);
    assert.match(recordDecision, /printed seed key when a concept roll ran, or the permitted no-roll reason when it did not/);

    for (const block of ['THESIS', 'OWN-WORLD', 'STORY', 'FIRST VIEWPORT', 'FORM', 'FINISH']) {
      assert.match(recordDecision, new RegExp(`${block}:`));
    }

    for (const browserArtifact of [
      /HTML or framework comments/,
      /hidden DOM/,
      /<template>/,
      /`data-\*` attributes/,
      /serialized props or state/,
      /React Server Component payloads/,
      /client bundles/,
      /metadata or JSON-LD/,
      /accessibility-only text/,
    ]) {
      assert.match(recordDecision, browserArtifact);
    }

    assert.match(recordDecision, /Never copy the direction contract into implementation source or any browser-delivered artifact/);
    assert.doesNotMatch(newWork, /contract in the artifact's opening comment/);
    assert.doesNotMatch(newWork, /survives the production build/);
    assert.doesNotMatch(newWork, /grep the built output/);
    assert.doesNotMatch(newWork, /emitted markup/);
    assert.doesNotMatch(newWork, /first child of the document's body/);
  });

  it('keeps reduced-motion guidance on the animation build path', () => {
    const animate = readFileSync(join(ROOT, 'skill/reference/animate.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const accessibility = animate.match(/## Accessibility and control\n([\s\S]*?)\n## Verify/)?.[1] ?? '';
    const verify = animate.match(/## Verify\n([\s\S]*?)(?:\n## |$)/)?.[1] ?? '';

    assert.match(accessibility, /prefers-reduced-motion/);
    assert.match(accessibility, /intentional alternative/);
    assert.match(accessibility, /not disabling all motion/);
    assert.match(verify, /reduced[- ]motion/i);
  });

  it('keeps critiques in the chat and honors recorded decisions instead of an archive', () => {
    const read = (name) => readFileSync(join(ROOT, `skill/reference/${name}`), 'utf-8').replace(/\r\n?/g, '\n');
    const critique = read('critique.md');
    const polish = read('polish.md');

    for (const text of [critique, polish]) {
      assert.doesNotMatch(text, /critique-storage|\.impeccino\//);
    }
    assert.match(critique, /nothing is archived/);
    assert.match(critique, /PRODUCT\.md \(Brand Commitments, Product Principles\), DESIGN\.md \(Named Rules, Do's and Don'ts/);
    assert.match(critique, /`docs\/adr\/`, `doc\/adr\/`, `adr\/`/);
    assert.match(critique, /A style warning that contradicts a recorded decision is dropped/);
    assert.match(critique, /Still report observed accessibility or functional defects/);
    assert.match(critique, /When the user rejects a finding as deliberate/);
    assert.match(critique, /product or brand intent goes into PRODUCT\.md/);
    assert.match(critique, /a visual rule goes into DESIGN\.md/);
    assert.match(critique, /impeccino-disable <rule-id>/);
    assert.match(polish, /Critiques are not archived between runs/);
    assert.match(polish, /Perform an independent pass either way/);
    assert.match(polish, /is not drift; leave it/);
    for (const audit of ['audit.md', 'audit.native.md']) {
      assert.match(read(audit), /A finding that contradicts a recorded decision is dropped/, audit);
    }
  });

  it('keeps DESIGN.md waivers through a rewrite', () => {
    const document = readFileSync(join(ROOT, 'skill/reference/document.md'), 'utf-8').replace(/\r\n?/g, '\n');
    assert.match(document, /\*\*Preserve the waivers\.\*\*/);
    assert.match(document, /carry every such comment over verbatim and keep it next to the Named Rule or Do\/Don't that justifies it/);
    assert.match(document, /detector metadata only/);
    assert.match(document, /extensions\.colorMeta\.<token>\.canonical/);
    assert.match(document, /extensions\.colorMeta\.<token>\.tonalRamp/);
    assert.match(document, /extensions\.roundedMeta\.<token>/);
    assert.match(document, /extensions\.shadows\[\]\.value/);
    assert.match(document, /do not generate component snippets, narrative, motion, breakpoints/i);
    assert.doesNotMatch(document, /shadow DOM|5-10 components/i);
  });

  it('keeps touch-gesture verification in the adapt, audit, and harden references', () => {
    const adapt = readFileSync(join(ROOT, 'skill/reference/adapt.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const audit = readFileSync(join(ROOT, 'skill/reference/audit.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const harden = readFileSync(join(ROOT, 'skill/reference/harden.md'), 'utf-8').replace(/\r\n?/g, '\n');
    const verifyAdaptations = adapt.match(/## Verify Adaptations\n([\s\S]*?)\n## /)?.[1] ?? '';
    const responsive = audit.match(/### 4\. Responsive Design\n([\s\S]*?)\n### 5\./)?.[1] ?? '';
    const edgeCases = harden.match(/### Edge Cases & Boundary Conditions\n([\s\S]*?)\n### /)?.[1] ?? '';
    const verifyHardening = harden.match(/## Verify Hardening\n([\s\S]*?)(?:\n## |$)/)?.[1] ?? '';

    assert.match(verifyAdaptations, /\*\*Primary gesture\*\*/);
    assert.match(verifyAdaptations, /produced the evidence/);
    assert.match(verifyAdaptations, /verify layout, never a gesture/);
    assert.match(verifyAdaptations, /reported gap, not a blocker/);
    assert.match(verifyAdaptations, /\*\*Scroll across it\*\*[\s\S]*without activating it/);
    assert.match(responsive, /\*\*Broken touch interaction\*\*/);
    assert.match(responsive, /what stayed untested/);
    assert.match(responsive, /Exercise the gesture when a browser tool can synthesize touch/);
    assert.match(edgeCases, /\*\*Interrupted gestures\*\*[\s\S]*works without a reload/);
    assert.match(edgeCases, /clear the dragging state and release capture/);
    assert.match(verifyHardening, /\*\*Interrupted gestures\*\*/);
  });
});
