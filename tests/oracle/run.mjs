#!/usr/bin/env node
/**
 * Replay the corpus against the engine binary and diff against goldens.
 *   IMPECCINO_BIN=/path/to/impeccino node tests/oracle/run.mjs [prefix]
 * Exit 1 on any difference or missing golden.
 */
import { allCases, runCase, readGolden, diffResults, caseRunsHere, expectedForPlatform } from './lib.mjs';

const argv = process.argv.slice(2);
if (argv.includes('--js')) {
  process.stderr.write('run.mjs replays only against the engine binary; set IMPECCINO_BIN and omit --js.\n');
  process.exit(2);
}
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
  const actual = runCase(c);
  const diffs = diffResults(golden, actual);
  if (!diffs.length) { pass++; continue; }
  fail++;
  process.stdout.write(`XX ${c.id}\n${diffs.map(d => '   ' + d.replace(/\n/g, '\n   ')).join('\n')}\n`);
}
process.stdout.write(`\n${pass} pass, ${fail} fail, ${missing} missing goldens${skipped ? `, ${skipped} skipped on ${process.platform}` : ''} (engine binary)\n`);
process.exit(fail || missing ? 1 : 0);
