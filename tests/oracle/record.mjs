#!/usr/bin/env node
/**
 * Record goldens.
 *   node tests/oracle/record.mjs --bin [prefix]     # from the engine binary ($IMPECCINO_BIN,
 *                                                   # or --bin=/path/to/impeccino)
 *   node tests/oracle/record.mjs [prefix]           # from the JS scripts (historical; the
 *                                                   # scripts left the tree with the launcher swap)
 * A prefix limits recording to ids starting with it (e.g. detect-);
 * --ids=a,b records exactly those ids.
 *
 * The committed goldens are the behavior contract. Re-record only cases you
 * added or intentionally changed, review each resulting diff by hand, and
 * leave the frozen function-level vectors untouched.
 */
import { allCases, assertRecordableCases, runCase, writeGolden } from './lib.mjs';

const argv = process.argv.slice(2);
const binFlag = argv.find(a => a === '--bin' || a.startsWith('--bin='));
const impl = binFlag ? 'bin' : 'js';
const bin = binFlag?.includes('=') ? binFlag.split('=').slice(1).join('=') : process.env.IMPECCINO_BIN;
if (impl === 'bin' && !bin) {
  process.stderr.write('record.mjs --bin needs IMPECCINO_BIN or --bin=/path/to/impeccino\n');
  process.exit(2);
}
if (impl === 'bin') process.env.IMPECCINO_BIN = bin;
if (impl === 'js') {
  const { existsSync } = await import('node:fs');
  const { REPO_ROOT } = await import('./lib.mjs');
  if (!existsSync(new URL('../../skill/scripts/context.mjs', import.meta.url))) {
    process.stderr.write('record.mjs: the JS scripts are no longer in the tree; use --bin to record from the engine binary.\n');
    process.exit(1);
  }
  void REPO_ROOT;
}
const prefix = argv.find(a => !a.startsWith('--')) || '';
const exact = argv.find(a => a.startsWith('--ids='))?.slice(6).split(',').filter(Boolean);
const cases = (await allCases()).filter(c => (exact ? exact.includes(c.id) : c.id.startsWith(prefix)));
try {
  assertRecordableCases(cases);
} catch (error) {
  process.stderr.write(`record.mjs: ${error.message}\n`);
  process.exit(2);
}
let n = 0;
for (const c of cases) {
  const res = runCase(c, { impl, bin });
  writeGolden(c.id, res);
  n++;
  const head = res.steps ? `${res.steps.length} steps, exits ${res.steps.map(s => s.exit).join('/')}` : `exit ${res.exit}, ${res.stdout.length}b out`;
  process.stdout.write(`recorded ${c.id} (${head}, ${Object.keys(res.files).length} files)\n`);
}
process.stdout.write(`\n${n} goldens written (${impl})\n`);
