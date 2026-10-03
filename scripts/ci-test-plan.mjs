#!/usr/bin/env node
import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { DEFAULT_SUITES, matchesSuiteTriggers } from './test-suites.mjs';

const eventName = process.env.GITHUB_EVENT_NAME || '';
const localNoChanges = !eventName && !process.env.CI_CHANGED_FILES;
const changedFiles = localNoChanges ? [] : getChangedFiles();
const forceDeterministic = localNoChanges || eventName === 'push' || eventName === 'workflow_dispatch';
const forceSkillBehavior = eventName === 'workflow_dispatch'
  && process.env.GITHUB_EVENT_INPUTS_SKILL_BEHAVIOR === 'true';
// The Rust workspace (the engine) builds and tests when its own inputs move.
// tests/oracle is included: the goldens are the engine's behavior gate and
// the oracle job replays them against a source build.
const RUST_PATTERNS = [
  /^crates\//,
  /^Cargo\.(toml|lock)$/,
  /^about\.(toml|hbs)$/,
  /^rust-toolchain\.toml$/,
  /^LICENSE$/,
  /^NOTICE\.md$/,
  /^skill\/(LICENSE|NOTICE\.md)$/,
  /^scripts\/generate-engine-notices\.mjs$/,
  // The engine embeds this skill command metadata at compile time.
  /^skill\/scripts\/command-metadata\.json$/,
  /^tests\/oracle\//,
  /^\.github\/workflows\/ci\.yml$/,
];
const rustChanged = changedFiles.some((file) => RUST_PATTERNS.some((re) => re.test(file)));

const plan = {
  core: true,
  oracle: forceDeterministic || matchesSuiteTriggers('oracle', changedFiles),
  rust: forceDeterministic || rustChanged,
  skill_behavior: forceSkillBehavior || matchesSuiteTriggers('skill-behavior', changedFiles),
};

writeGithubOutputs(plan);
printSummary(plan, changedFiles);

function getChangedFiles() {
  if (process.env.CI_CHANGED_FILES) {
    return process.env.CI_CHANGED_FILES
      .split(/\r?\n/)
      .map((file) => file.trim())
      .filter(Boolean);
  }

  const event = process.env.GITHUB_EVENT_NAME || '';
  const sha = process.env.GITHUB_SHA || 'HEAD';

  if (event === 'pull_request' && process.env.GITHUB_BASE_REF) {
    const base = `origin/${process.env.GITHUB_BASE_REF}`;
    return gitDiffNames(`${base}...${sha}`) || gitDiffNames(`${base}...HEAD`) || allChanged();
  }

  const before = process.env.GITHUB_EVENT_BEFORE;
  if (before && !/^0+$/.test(before)) {
    return gitDiffNames(`${before}..${sha}`) || allChanged();
  }

  return allChanged();
}

function allChanged() {
  return git(['ls-files']).split(/\r?\n/).filter(Boolean);
}

function gitDiffNames(range) {
  try {
    return git(['diff', '--name-only', range]).split(/\r?\n/).filter(Boolean);
  } catch {
    return null;
  }
}

function git(args) {
  return execFileSync('git', args, { encoding: 'utf-8' });
}

function writeGithubOutputs(outputs) {
  const outputPath = process.env.GITHUB_OUTPUT;
  if (!outputPath) return;
  const lines = [];
  for (const [key, value] of Object.entries(outputs)) {
    lines.push(`${key}=${value ? 'true' : 'false'}`);
  }
  fs.appendFileSync(outputPath, lines.join('\n') + '\n');
}

function printSummary(outputs, files) {
  const deterministic = DEFAULT_SUITES.map((name) => `${name}=${outputs[name]}`).join(' ');
  console.log(`Event: ${eventName || 'local'}`);
  console.log(`Changed files: ${files.length}`);
  console.log(`Deterministic suites: ${deterministic} rust=${outputs.rust}`);
  console.log(`skill_behavior=${outputs.skill_behavior}`);
}
