import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const workflow = Bun.YAML.parse(readFileSync(new URL('../.github/workflows/release-engine.yml', import.meta.url), 'utf8'));
const action = (job, name) => job.steps.find(step => step.uses?.startsWith(`${name}@`));

describe('engine release workflow', () => {
  test('runs on engine tags with a read-only token; only publishing may write and attest', () => {
    expect(workflow.on).toEqual({ push: { tags: ['engine-v*'] } });
    expect(workflow.permissions).toEqual({ contents: 'read' });
    expect(workflow.jobs.publish.permissions).toEqual({ contents: 'write', 'id-token': 'write', attestations: 'write' });
    expect(workflow.jobs.build.permissions?.['id-token']).toBeUndefined();
  });

  test('attests every binary before a draft release is published', () => {
    const steps = workflow.jobs.publish.steps;
    const attest = action(workflow.jobs.publish, 'actions/attest-build-provenance');
    expect(attest.with['subject-path']).toBe('out/impeccino-*');
    const publish = steps.find(step => step.name === 'Publish the GitHub Release');
    expect(steps.indexOf(attest)).toBeLessThan(steps.indexOf(publish));
    expect(publish.run).toContain('--draft');
    expect(publish.run).toContain('--draft=false');
    expect(publish.run).not.toContain('gh release upload');
  });

  test('publishes every built binary', () => {
    expect(workflow.jobs.publish.needs).toBe('build');
    expect(action(workflow.jobs.build, 'actions/upload-artifact').with.name).toBe('impeccino-${{ matrix.short }}');
    expect(action(workflow.jobs.publish, 'actions/download-artifact').with.pattern).toBe('impeccino-*');
    expect(workflow.jobs.publish.steps.find(step => step.name === 'Lay out release assets').run).not.toContain('sha256sum');
  });

  test('artifact downloads stay on the same-run runtime-token path', () => {
    const download = action(workflow.jobs.publish, 'actions/download-artifact');
    // Supplying github-token opts into the public API path, which requires
    // separate Actions permissions and can read other workflow runs.
    for (const input of ['github-token', 'repository', 'run-id']) {
      expect(download.with[input]).toBeUndefined();
    }
  });

  test('every third-party action is pinned to a commit', () => {
    for (const job of Object.values(workflow.jobs)) {
      for (const step of job.steps) {
        if (step.uses) expect(step.uses).toMatch(/@[a-f0-9]{40}$/);
      }
    }
  });
});
