import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const SCRIPT = 'scripts/ci-test-plan.mjs';

describe('ci-test-plan', () => {
  it('requires explicit manual opt-in and preprovisions the full workflow job', () => {
    const workflow = readFileSync('.github/workflows/ci.yml', 'utf8');
    assert.match(workflow, /skill_workflow:\s*description:[^\n]+\s*type: boolean\s*default: false/);
    const job = workflow.split('\n  skill-workflow:')[1];
    assert.doesNotMatch(job.split('\n    steps:')[0], /runner\./, 'runner context is unavailable at job-level env');
    assert.match(job, /if: github.event_name == 'workflow_dispatch' && inputs.skill_workflow/);
    assert.ok(job.indexOf('pnpm run fetch:engine') < job.indexOf('pnpm run test:skill-workflow'));
    assert.ok(job.indexOf('playwright install --with-deps chromium') < job.indexOf('pnpm run test:skill-workflow'));
    const protocol = workflow.split('\n  skill-behavior:')[1].split('\n  skill-workflow:')[0];
    assert.match(protocol, /pnpm run fetch:engine/);
    assert.doesNotMatch(protocol, /IMPECCINO_SKILL_BEHAVIOR_MODELS:/, 'protocol coverage must retain the multi-family defaults');
    assert.match(protocol, /GOOGLE_CLOUD_API_KEY:/);
    assert.match(protocol, /ANTHROPIC_API_KEY:/);
  });
  it('keeps docs-only pull requests on the core suite', () => {
    const outputs = runPlan({
      GITHUB_EVENT_NAME: 'pull_request',
      CI_CHANGED_FILES: 'README.md',
    });

    assert.equal(outputs.core, 'true');
    assert.equal(outputs.oracle, 'false');
    assert.equal(outputs.rust, 'false');
    assert.equal(outputs.skill_behavior, 'false');
  });

  it('routes an engine version bump to the oracle lane', () => {
    const outputs = runPlan({
      GITHUB_EVENT_NAME: 'pull_request',
      CI_CHANGED_FILES: 'skill/scripts/VERSION',
    });

    assert.equal(outputs.oracle, 'true');
  });

  it('routes skill setup changes to the skill behavior lane', () => {
    const outputs = runPlan({
      GITHUB_EVENT_NAME: 'pull_request',
      CI_CHANGED_FILES: 'skill/SKILL.md',
    });

    assert.equal(outputs.skill_behavior, 'true');
  });

  it('forces deterministic suites on push without forcing opt-in suites', () => {
    const outputs = runPlan({
      GITHUB_EVENT_NAME: 'push',
      CI_CHANGED_FILES: 'README.md',
    });

    assert.equal(outputs.core, 'true');
    assert.equal(outputs.oracle, 'true');
    assert.equal(outputs.rust, 'true');
    assert.equal(outputs.skill_behavior, 'false');
  });

  it('enables opt-in suites on manual dispatch', () => {
    const outputs = runPlan({
      GITHUB_EVENT_NAME: 'workflow_dispatch',
      CI_CHANGED_FILES: 'README.md',
    });
    assert.equal(outputs.skill_behavior, 'true');
  });

  it('schedule events run only the deterministic suites', () => {
    const outputs = runPlan({ GITHUB_EVENT_NAME: 'schedule' });
    assert.equal(outputs.core, 'true');
    assert.equal(outputs.oracle, 'true');
    assert.equal(outputs.rust, 'true');
    assert.equal(outputs.skill_behavior, 'false');
  });

});

function runPlan(env) {
  const tmp = mkdtempSync(join(tmpdir(), 'impeccino-ci-plan-'));
  const outputPath = join(tmp, 'github-output');
  try {
    const result = spawnSync(process.execPath, [SCRIPT], {
      cwd: process.cwd(),
      encoding: 'utf-8',
      env: {
        ...process.env,
        GITHUB_OUTPUT: outputPath,
        ...env,
      },
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    return Object.fromEntries(
      readFileSync(outputPath, 'utf-8')
        .trim()
        .split(/\r?\n/)
        .filter(Boolean)
        .map((line) => line.split('=')),
    );
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}
