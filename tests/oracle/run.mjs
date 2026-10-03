#!/usr/bin/env node
/**
 * Replay the corpus against an implementation and diff against goldens.
 *   IMPECCINO_BIN=/path/to/impeccino node tests/oracle/run.mjs [prefix]
 *   node tests/oracle/run.mjs --js [prefix]     # self-check: JS vs its own goldens
 * Exit 1 on any difference or missing golden.
 */
import { allCases, runCase, readGolden, diffResults, caseRunsHere, expectedForPlatform } from './lib.mjs';

const argv = process.argv.slice(2);
const impl = argv.includes('--js') ? 'js' : 'bin';
const prefix = argv.find(a => !a.startsWith('--')) || '';
const cases = (await allCases()).filter(c => c.id.startsWith(prefix));
let pass = 0, fail = 0, missing = 0, skipped = 0;
for (const c of cases) {
  if (!caseRunsHere(c)) {
    skipped++;
    process.stdout.write(`-- ${c.id}: skipped on ${process.platform} — ${c.platformSkipReason}\n`);
    continue;
  }
  const recorded = readGolden(c.id);
  if (!recorded) { missing++; process.stdout.write(`?? ${c.id}: no golden (run record.mjs)\n`); continue; }
  const golden = expectedForPlatform(c, recorded);
  const actual = runCase(c, { impl });
  const diffs = diffResults(golden, actual);
  if (!diffs.length) { pass++; continue; }
  fail++;
  process.stdout.write(`XX ${c.id}\n${diffs.map(d => '   ' + d.replace(/\n/g, '\n   ')).join('\n')}\n`);
}
process.stdout.write(`\n${pass} pass, ${fail} fail, ${missing} missing goldens${skipped ? `, ${skipped} skipped on ${process.platform}` : ''} (${impl})\n`);
process.exit(fail || missing ? 1 : 0);
