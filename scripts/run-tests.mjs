#!/usr/bin/env node
import { spawn } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_SUITES, OPT_IN_SUITES, SUITES, expandSuites } from './test-suites.mjs';
import { createGroupShutdown, trackChildExit } from './lib/process-group.mjs';

const REPO_ROOT = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const VITEST = path.join(REPO_ROOT, 'node_modules', 'vitest', 'vitest.mjs');

// Global wall-clock backstop for any one command. Even with per-test timeouts
// and client-side network deadlines in place, a wedged tool or an orphaned
// grandchild can keep a runner alive forever; this cap guarantees the sweep
// terminates. Per-suite `wallClockMs` overrides it; the env var overrides both.
const DEFAULT_WALL_CLOCK_MS = Number(process.env.IMPECCINO_TEST_WALL_CLOCK_MS) || 1_200_000;

const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  printHelp();
  process.exit(0);
}

if (args.includes('--list')) {
  printSuites();
  process.exit(0);
}


/**
 * Suite commands run in their own process group, which buys two things: the
 * wall-clock cap can SIGKILL a wedged tree whole (Vitest, its worker
 * processes, and any grandchildren or browsers they left open),
 * and a Ctrl-C can end that same tree deterministically instead of orphaning
 * it. Nothing in this file may use spawnSync: a blocked event loop cannot run
 * the signal handlers that make either guarantee, and cannot reap the child it
 * is waiting on either.
 */
const shutdown = createGroupShutdown();

for (const sig of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  process.on(sig, () => { void shutdown.onSignal(exitCodeForSignal(sig)); });
}
// Nothing can be awaited here, so this is the one path that does not wait.
process.on('exit', () => shutdown.onExit());

/** The shell convention for "killed by signal N": 130 SIGINT, 143 SIGTERM, 129 SIGHUP. */
function exitCodeForSignal(sig) {
  return 128 + (os.constants.signals[sig] ?? 0);
}

const requestedSuites = args.filter((arg) => !arg.startsWith('-'));
let suites;
try {
  suites = expandSuites(requestedSuites);
} catch (err) {
  console.error(err.message);
  process.exit(1);
}

await main();

async function main() {
  for (const suiteName of suites) {
    const suite = SUITES[suiteName];
    console.log(`\n## test:${suiteName}`);
    console.log(suite.description);
    for (const command of suite.commands) {
      await runCommand(command, suiteName);
    }
  }
}

async function runCommand(command, suiteName) {
  const env = { ...process.env, ...(command.env || {}) };
  const wallClockMs = command.wallClockMs ?? DEFAULT_WALL_CLOCK_MS;

  if (command.runner !== 'vitest') throw new Error(`Unsupported test runner "${command.runner}"`);
  // One Vitest invocation per command; it runs the files in parallel worker
  // processes (vitest.config.mjs: pool forks).
  const vitestArgs = [VITEST, 'run', '--config', path.join(REPO_ROOT, 'vitest.config.mjs')];
  if (command.timeoutMs) vitestArgs.push(`--testTimeout=${command.timeoutMs}`, `--hookTimeout=${command.timeoutMs}`);
  vitestArgs.push(...command.files);
  await runProcess(process.execPath, vitestArgs, { env, wallClockMs });
}

function runProcess(cmd, args, { env, wallClockMs }) {
  console.log(`$ ${formatCommand(cmd, args)}`);
  return new Promise((resolve) => {
    const child = spawn(cmd, args, {
      // Own process group: the wall-clock cap and the shutdown handler can
      // then take down the runner, every test file it forked, and anything
      // those forked, in one signal.
      detached: true,
      // stdin is deliberately not inherited. A detached child is a background
      // process group on the terminal, and a background read of the tty stops
      // the process with SIGTTIN. No suite reads the runner's stdin.
      stdio: ['ignore', 'inherit', 'inherit'],
      env,
    });
    // Registered before the handlers below, so `hasExited` is already set by
    // the time they run and a shutdown mid-exit does not signal a dead pid.
    const running = shutdown.track(trackChildExit(child));

    let timedOut = false;
    const timer = wallClockMs
      ? setTimeout(() => {
          timedOut = true;
          console.error(
            `\n[run-tests] wall-clock cap of ${wallClockMs}ms exceeded for "${formatCommand(cmd, args)}"; ` +
            'killing the process group (SIGKILL).',
          );
          // A test blocked in a synchronous spawnSync cannot be reached by
          // Vitest's test timeout, so this is the guaranteed end of the tree.
          // No graceful phase: the cap has already been generous.
          try { process.kill(-running.child.pid, 'SIGKILL'); }
          catch { try { running.child.kill('SIGKILL'); } catch { /* already gone */ } }
        }, wallClockMs)
      : null;

    child.on('error', (err) => {
      if (timer) clearTimeout(timer);
      shutdown.release();
      console.error(err.message);
      process.exit(1);
    });
    child.on('exit', (code, signal) => {
      if (timer) clearTimeout(timer);
      shutdown.release();
      if (shutdown.shuttingDown) return;
      if (timedOut) process.exit(1);
      if (signal) {
        console.error(`[run-tests] "${formatCommand(cmd, args)}" killed by signal ${signal}`);
        process.exit(1);
      }
      if (code !== 0) {
        process.exit(code || 1);
      }
      resolve();
    });
  });
}

function formatCommand(cmd, args) {
  const shown = cmd === process.execPath && args[0] === VITEST ? ['vitest', ...args.slice(1)] : [cmd === process.execPath ? 'node' : cmd, ...args];
  return shown.join(' ');
}

function printHelp() {
  console.log(`Usage: node scripts/run-tests.mjs [suite...]

Aliases:
  default     ${DEFAULT_SUITES.join(', ')}
  all-local   ${DEFAULT_SUITES.join(', ')}
  all         ${[...DEFAULT_SUITES, ...OPT_IN_SUITES].join(', ')}

Run with --list to see suite contents.`);
}

function printSuites() {
  for (const [name, suite] of Object.entries(SUITES)) {
    const marker = suite.optIn ? ' (opt-in)' : '';
    console.log(`\n${name}${marker}`);
    console.log(`  ${suite.description}`);
    for (const command of suite.commands) {
      console.log(`  ${command.runner}:`);
      for (const file of command.files) console.log(`    ${file}`);
    }
  }
}
