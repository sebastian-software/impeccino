import { describe, expect, test } from 'vitest';
import { parse } from 'yaml';
import { readFileSync, readdirSync } from 'node:fs';

const directory = new URL('../.github/workflows/', import.meta.url);
const workflows = Object.fromEntries(readdirSync(directory)
  .filter(name => /\.ya?ml$/.test(name))
  .map(name => [name, parse(readFileSync(new URL(name, directory), 'utf8'))]));

describe('workflow execution boundaries', () => {
  test('installs repository dependencies before helpers that parse skill YAML', () => {
    const yamlHelper = /node scripts\/(?:pin-engine|skill-pin-pr|release)\.mjs|node scripts\/product-release\.mjs prepare-skill/;
    for (const [name, workflow] of Object.entries(workflows)) {
      for (const [jobName, job] of Object.entries(workflow.jobs)) {
        const steps = job.steps || [];
        const helperIndex = steps.findIndex(step => yamlHelper.test(step.run || ''));
        if (helperIndex === -1) continue;
        const context = `${name}: ${jobName}`;
        const installIndex = steps.findIndex(step => /^pnpm install --frozen-lockfile$/m.test(step.run || ''));
        expect(installIndex, context).toBeGreaterThan(-1);
        expect(installIndex, context).toBeLessThan(helperIndex);
        expect(steps.slice(0, installIndex).some(step => step.uses?.startsWith('pnpm/action-setup@')), context).toBe(true);
      }
    }
  });

  test('repository actions are pinned to full commit SHAs', () => {
    for (const [name, workflow] of Object.entries(workflows)) {
      for (const [jobName, job] of Object.entries(workflow.jobs)) {
        for (const step of job.steps || []) {
          if (!step.uses || step.uses.startsWith('./')) continue;
          expect(step.uses, `${name}: ${jobName}`).toMatch(/@[a-f0-9]{40}$/);
        }
      }
    }
  });

  test('CI uses a read-only repository token without job-level escalation', () => {
    const ci = workflows['ci.yml'];
    expect(ci.permissions).toEqual({ contents: 'read' });
    for (const job of Object.values(ci.jobs)) {
      expect(job.permissions).toBeUndefined();
    }
  });

  test('runner-scoped contexts are not used in any job-level environment', () => {
    for (const [name, workflow] of Object.entries(workflows)) {
      for (const [jobName, job] of Object.entries(workflow.jobs)) {
        expect(JSON.stringify(job.env || {}), `${name}: ${jobName}`).not.toMatch(/\$\{\{\s*runner\./);
      }
    }
  });
});
