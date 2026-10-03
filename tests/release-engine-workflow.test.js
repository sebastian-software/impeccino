import { describe, expect, test } from 'vitest';
import { parse } from 'yaml';
import { readFileSync } from 'node:fs';
import { RELEASE_TARGET_LICENSE_CRATES } from '../scripts/generate-engine-notices.mjs';

const workflow = parse(readFileSync(new URL('../.github/workflows/release-engine.yml', import.meta.url), 'utf8'));
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

  test('generates plain-text notices from the locked target union before binary attestation', () => {
    const steps = workflow.jobs.publish.steps;
    const checkout = action(workflow.jobs.publish, 'actions/checkout');
    const fetch = steps.find(step => step.name === 'Fetch locked release dependencies');
    const install = steps.find(step => step.name === 'Install pinned cargo-about');
    const notices = steps.find(step => step.name === 'Generate engine third-party notices');
    const attest = action(workflow.jobs.publish, 'actions/attest-build-provenance');
    const publish = steps.find(step => step.name === 'Publish the GitHub Release');

    expect(checkout.with['persist-credentials']).toBe(false);
    expect(steps.indexOf(checkout)).toBeLessThan(steps.indexOf(fetch));
    expect(steps.indexOf(fetch)).toBeLessThan(steps.indexOf(install));
    expect(steps.indexOf(install)).toBeLessThan(steps.indexOf(notices));
    expect(steps.indexOf(notices)).toBeLessThan(steps.indexOf(attest));
    expect(notices.run).toBe('node scripts/generate-engine-notices.mjs out/THIRD-PARTY-NOTICES.txt');
    expect(attest.with['subject-path']).toBe('out/impeccino-*');
    expect(publish.run).toContain('THIRD-PARTY-NOTICES.txt');
    expect(publish.run).toContain('out/*');
  });

  test('publishes every built binary', () => {
    expect(workflow.jobs.publish.needs).toEqual(['build', 'smoke-linux-arm64']);
    expect(action(workflow.jobs.build, 'actions/upload-artifact').with.name).toBe('impeccino-${{ matrix.short }}');
    expect(action(workflow.jobs.publish, 'actions/download-artifact').with.pattern).toBe('impeccino-*');
    expect(workflow.jobs.publish.steps.find(step => step.name === 'Lay out release assets').run).not.toContain('sha256sum');
    expect(workflow.jobs.build.strategy.matrix.include.map(target => target.short)).toEqual([
      'darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'windows-x64',
    ]);
    const build = workflow.jobs.build.steps.find(step => step.name === 'Build');
    expect(build.run).toContain('build --locked --release -p impeccino --target ${{ matrix.target }}');
  });

  test('license generator covers every published platform and retains transitive packages', () => {
    const config = readFileSync(new URL('../about.toml', import.meta.url), 'utf8');
    const targetBlock = config.match(/^targets = \[([\s\S]*?)\]/m)?.[1] ?? '';
    expect(targetBlock.match(/"[^"]+"/g)).toEqual([
      '"aarch64-apple-darwin"',
      '"x86_64-apple-darwin"',
      '"x86_64-unknown-linux-musl"',
      '"aarch64-unknown-linux-musl"',
      '"x86_64-pc-windows-msvc"',
    ]);
    expect(config).toContain('ignore-transitive-dependencies = false');
    expect(config).toContain('private = { ignore = true }');
    expect(config).toContain('[ring]\naccepted = ["OpenSSL"]');

    const template = readFileSync(new URL('../about.hbs', import.meta.url), 'utf8');
    expect(template).toContain('License: {{{name}}}');
    expect(template).toContain('{{{text}}}');
    const generator = readFileSync(new URL('../scripts/generate-engine-notices.mjs', import.meta.url), 'utf8');
    expect(generator).toContain("'--frozen'");
    expect(generator).toContain("'--fail'");
    const lock = readFileSync(new URL('../Cargo.lock', import.meta.url), 'utf8');
    expect(RELEASE_TARGET_LICENSE_CRATES).toContain('windows-link');
    expect(RELEASE_TARGET_LICENSE_CRATES).toContain('libc');
    for (const crate of RELEASE_TARGET_LICENSE_CRATES) {
      expect(lock).toContain(`name = "${crate}"\n`);
    }
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

  test('validates both engine version sources against the pushed tag', () => {
    const check = workflow.jobs.build.steps.find(step => step.name === 'Check the tag matches both engine versions');
    expect(check.shell).toBe('bash');
    expect(check.run).toContain('node scripts/release.mjs engine --check-tag "$GITHUB_REF_NAME"');
  });

  test('pins cross and runs the exact arm64 artifact before publishing', () => {
    const cross = workflow.jobs.build.steps.find(step => step.name === 'Install pinned cross');
    expect(cross.run).toBe('cargo install cross --locked --version 0.2.5');

    const smoke = workflow.jobs['smoke-linux-arm64'];
    expect(smoke.needs).toBe('build');
    expect(smoke['runs-on']).toBe('ubuntu-24.04-arm');
    const download = action(smoke, 'actions/download-artifact');
    expect(download.with.name).toBe('impeccino-linux-arm64');
    const runStep = smoke.steps.find(step => step.name === 'Run the exact linux-arm64 release artifact');
    expect(runStep.run).toContain('chmod +x artifact/impeccino');
    expect(runStep.run).toContain('artifact/impeccino --version');
    expect(runStep.run).toContain('artifact/impeccino engine-probe');
    expect(smoke.steps.indexOf(download)).toBeLessThan(smoke.steps.indexOf(runStep));
    expect(workflow.jobs.publish.needs).toEqual(['build', 'smoke-linux-arm64']);
  });
});
