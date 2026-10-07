#!/usr/bin/env node

/**
 * Repository checks for Impeccino.
 *
 * There is no build: skill/ is the universal skill and installs as-is
 * (docs/adr/0001, docs/adr/0003). This script only validates what tests do
 * not: count claims in user-facing files, skill frontmatter limits, and the
 * prose gates from docs/STYLE.md.
 */

import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { readSourceFiles } from './lib/utils.js';

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/**
 * Count commands (from SKILL.md's router table) and detection rules (from the
 * rule registry) and flag stale count claims in user-facing files.
 */
function checkCounts(rootDir, skills) {
  // Count active commands. After the v3.0 consolidation, commands are sub-commands
  // of /impeccino. Count them from the command router table in SKILL.md.
  const impeccinoSkill = skills.find(s => s.name === 'impeccino');
  if (!impeccinoSkill) {
    console.error('❌ skill/SKILL.md must define the impeccino skill.');
    return 1;
  }
  const routerMatches = impeccinoSkill.body.match(/^\| `[^`]+` \|/gm);
  const commandCount = routerMatches ? routerMatches.length : 0;

  // Count detection rules from the registry (crates/foundation/src/registry.rs).
  // Without it, the detection-count check is skipped rather than guessed.
  const { count: detectionCount, reason: detectionReason } = readDetectionRuleCount(rootDir);

  // Validate counts in key files
  const filesToCheck = [
    'README.md',
    'README.md.src',
    'AGENTS.md',
  ];

  let errors = 0;
  for (const relPath of filesToCheck) {
    const absPath = path.join(rootDir, relPath);
    if (!fs.existsSync(absPath)) continue;
    const content = fs.readFileSync(absPath, 'utf-8');

    // Check for stale command counts (look for "N commands" or "N skills" patterns)
    // Historical changelog counts do not describe the current skill.
    const currentContent = content.replace(/<ul class="changelog-items">[\s\S]*?<\/ul>/g, '');
    const countPattern = /\b(\d+)\s+(design\s+)?(commands|sub-commands|skills|steering commands)/gi;
    for (const match of currentContent.matchAll(countPattern)) {
      const num = parseInt(match[1]);
      // Allow 1 (for "1 skill") and the correct count
      if (num !== commandCount && num !== 1) {
        console.error(`  ❌ ${relPath}: found "${match[0]}" but active command count is ${commandCount}`);
        errors++;
      }
    }

    // Check for stale detection counts.
    // "detector" as an infix ("60 deterministic detector rules") and
    // qualified "issues" both evaded the old pattern, which is how five
    // stale counts shipped while the validator reported clean.
    const detectPattern = /\b(\d+)\s+(deterministic\s+)?(detector\s+)?(checks|patterns|rules|detections|issues)\b/gi;
    for (const match of detectionCount == null ? [] : currentContent.matchAll(detectPattern)) {
      const num = parseInt(match[1]);
      if (match[4] === 'issues' && !match[2]) continue; // plain "issues" is prose, not a count claim
      if (num !== detectionCount && num > 10) { // ignore small numbers like "3 patterns"
        console.error(`  ❌ ${relPath}: found "${match[0]}" but detection count is ${detectionCount}`);
        errors++;
      }
    }
  }

  if (errors > 0) {
    console.error(`\n❌ ${errors} stale count reference(s) found. Update them to match source of truth.`);
  }

  console.log(`✓ Counts: ${commandCount} commands, ${detectionCount == null ? `detection rules unchecked: ${detectionReason}` : `${detectionCount} detection rules`}`);
  return errors;
}

const RULE_REGISTRY = path.join('crates', 'foundation', 'src', 'registry.rs');

/**
 * The number of distinct built-in rule ids, read from the `ANTIPATTERNS` table
 * in the registry source (the part before its test module), or
 * `{ count: null, reason }` when the table cannot be found.
 */
function readDetectionRuleCount(rootDir) {
  const file = path.join(rootDir, RULE_REGISTRY);
  if (!fs.existsSync(file)) return { count: null, reason: `${RULE_REGISTRY} is missing` };
  const source = fs.readFileSync(file, 'utf-8');
  const start = source.indexOf('pub static ANTIPATTERNS');
  const end = source.indexOf('\n];', start);
  if (start === -1 || end === -1) return { count: null, reason: `no ANTIPATTERNS table in ${RULE_REGISTRY}` };
  const ids = [...source.slice(start, end).matchAll(/^\s+id: "([^"]+)",$/gm)].map(m => m[1]);
  return ids.length ? { count: new Set(ids).size } : { count: null, reason: `ANTIPATTERNS in ${RULE_REGISTRY} has no ids` };
}

function validateSkillFrontmatter(skills) {
  let errors = 0;

  for (const skill of skills) {
    if (skill.description && skill.description.length > 1024) {
      console.error(`❌ ${skill.filePath}: invalid description: exceeds maximum length of 1024 characters (${skill.description.length})`);
      errors++;
    }
  }

  return errors;
}

function scanProseFiles(rootDir, target, phraseRules) {
  const emDashPatterns = [/—/g, /&mdash;/gi, /&#8212;/gi, /&#x2014;/gi];
  let errors = 0;

  const checkLine = (line, rel, lineNum) => {
    for (const re of emDashPatterns) {
      if (re.test(line)) {
        console.error(`  ❌ ${rel}:${lineNum}: em dash → ${line.trim().slice(0, 120)}`);
        console.error(`        Use commas, colons, semicolons, periods, or parentheses.`);
        errors++;
        re.lastIndex = 0;
        break;
      }
      re.lastIndex = 0;
    }
    if (/ -- /.test(line)) {
      console.error(`  ❌ ${rel}:${lineNum}: \` -- \` em-dash substitute → ${line.trim().slice(0, 120)}`);
      console.error(`        Worse than the em dash. Pick real punctuation.`);
      errors++;
    }
    for (const rule of phraseRules) {
      if (rule.re.test(line)) {
        const matched = line.match(rule.re)?.[0] ?? '';
        console.error(`  ❌ ${rel}:${lineNum}: "${matched}" → ${line.trim().slice(0, 120)}`);
        console.error(`        ${rule.rationale}`);
        errors++;
      }
    }
  };

  const scan = (absPath, rel) => {
    if (fs.statSync(absPath).isDirectory()) {
      for (const entry of fs.readdirSync(absPath)) {
        scan(path.join(absPath, entry), path.join(rel, entry));
      }
      return;
    }
    if (!['.md', '.json'].includes(path.extname(absPath))) return;
    fs.readFileSync(absPath, 'utf-8').split('\n')
      .forEach((line, index) => checkLine(line, rel, index + 1));
  };
  const full = path.join(rootDir, target);
  if (fs.existsSync(full)) scan(full, target);
  return errors;
}

/**
 * Scan user-facing copy for AI-prose anti-patterns:
 *   - em dashes (— or &mdash;)
 *   - double-hyphen substitutes (` -- `)
 *   - denylisted phrases that read as AI tells in marketing copy
 *
 * The denylist is the editorial brief in docs/STYLE.md, enforced. Each rule has a
 * rationale that prints with the failure so the next author understands why.
 *
 * Scope: README.md. Not skill/, where
 * LLM-facing reference instructions can use technical phrasings the marketing
 * copy can't.
 *
 * Returns the number of occurrences found. Build fails if > 0.
 */
function validateProse(rootDir) {
  // Phrase rules: { re, rationale }. Add to docs/STYLE.md when adding here.
  const phraseRules = [
    { re: /\bload-bearing\b/i, rationale: 'AI tell. Stolen-engineer diction; almost always vague. Name what the thing actually does.' },
    { re: /\bhighest-leverage\b/i, rationale: 'AI tell. Vague claim of impact. Say what specifically pays off.' },
    { re: /\bbiggest unlock\b/i, rationale: 'AI tell. Marketing-speak. Describe the actual change.' },
    { re: /\breflex defaults?\b/i, rationale: 'Internal jargon leaking into user-facing copy. Say "instincts" or "first guesses".' },
    { re: /\bcollapses? into monoculture\b/i, rationale: 'Internal eval-speak. Describe what actually went wrong.' },
    { re: /\bdata-driven\b/i, rationale: 'Empty marketing adjective. Cite the data instead.' },
    { re: /\bseamless(?:ly)?\b/i, rationale: 'Hollow positive. Say what specifically works without friction.' },
    { re: /\brobust(?:ness)?\b/i, rationale: 'Hollow positive. Cite the failure mode it handles.' },
    { re: /\bdelves?\b|\bdelved\b|\bdelving\b/i, rationale: 'Top AI tell. Use "explore", "look at", or just delete.' },
    { re: /\belevate(?:s|d)?\b/i, rationale: 'Marketing verb. Use the specific verb (improve, raise, sharpen).' },
    { re: /\bempower(?:s|ed|ing)?\b/i, rationale: 'Marketing verb. Use "let you" or "make possible".' },
    { re: /\bunderscore(?:s|d)?\b/i, rationale: 'AI tell. Use "show" or "make clear".' },
    { re: /\bpivotal\b/i, rationale: 'Hollow positive. Use "central", "key", or describe the role.' },
    { re: /\bin today's\b/i, rationale: 'Throat-clearing opener. Cut the clause; start at the point.' },
    { re: /\bgone are the days\b/i, rationale: 'Throat-clearing. Make the point directly.' },
    { re: /\bwhether you're\b/i, rationale: 'Audience-pandering. Pick one reader; write to them.' },
    { re: /\blet's dive in\b/i, rationale: 'Throat-clearing. Just start.' },
    { re: /\bin summary\b|\bin conclusion\b/i, rationale: 'Summarizing closer. End on the strongest sentence; trust the reader.' },
    { re: /\bmoreover\b|\bfurthermore\b/i, rationale: 'Transition crutch on a metronome. Drop, or use "also".' },
    { re: /\btapestry\b/i, rationale: 'AI scenery noun. Cut.' },
  ];
  const errors = scanProseFiles(rootDir, 'README.md', phraseRules)
    + scanProseFiles(rootDir, 'README.md.src', phraseRules);

  if (errors === 0) {
    console.log(`✓ Prose validator: no AI tells in user-facing copy`);
  } else {
    console.error(`\n❌ ${errors} prose issue(s) in user-facing copy. See docs/STYLE.md for the rules.`);
  }
  return errors;
}

/**
 * Narrow prose check for Markdown skill guidance and pinned command metadata.
 *
 * The full validateProse rules don't fit LLM-facing reference instructions:
 * the hardening repetition and triadic checklists those files use exist on
 * purpose, and the structural-prose rules in docs/STYLE.md require human judgment.
 * This validator only enforces the mechanical wins: em dashes (which are
 * pure punctuation laziness regardless of audience) and the small handful
 * of denylisted phrases that have no technical reading. Em-dash creep is the
 * only thing likely to come back at scale once humans stop watching.
 *
 * Returns the number of occurrences found. Build fails if > 0.
 */
function validateSkillProse(rootDir) {
  // Tighter than validateProse: only the rules that have no technical reading.
  // Skipping `data-driven` here would be a mistake (it slipped through twice
  // in live.md before this pass); but `seamless`, `robust`, etc. have
  // legitimate technical uses elsewhere we may want to allow.
  const phraseRules = [
    { re: /\bnpx\s+impeccino\b/i, rationale: 'This points at the retired Node CLI. Use the quoted launcher from the installed skill.' },
    { re: /\bload-bearing\b/i, rationale: 'AI tell. Name what the thing actually does.' },
    { re: /\bhighest-leverage\b/i, rationale: 'AI tell. Say what specifically pays off.' },
    { re: /\bbiggest unlock\b/i, rationale: 'Marketing-speak. Describe the actual change.' },
    { re: /\breflex defaults?\b/i, rationale: 'Internal jargon. Say "instincts" or "first guesses".' },
    { re: /\bcollapses? into monoculture\b/i, rationale: 'Eval-speak. Describe what actually went wrong.' },
    { re: /\bdata-driven\b/i, rationale: 'Empty marketing adjective. Cite the data instead.' },
    { re: /\bdelves?\b|\bdelved\b|\bdelving\b/i, rationale: 'Top AI tell. Use "explore" or "look at".' },
    { re: /\btapestry\b/i, rationale: 'AI scenery noun. Cut.' },
    { re: /\bin today's\b/i, rationale: 'Throat-clearing opener. Start at the point.' },
    { re: /\bgone are the days\b/i, rationale: 'Throat-clearing. Make the point directly.' },
    { re: /\blet's dive in\b/i, rationale: 'Throat-clearing. Just start.' },
    { re: /\bin summary\b|\bin conclusion\b/i, rationale: 'Summarizing closer. End on the strongest sentence.' },
  ];
  const errors = scanProseFiles(rootDir, 'skill', phraseRules);

  if (errors === 0) {
    console.log(`✓ Skill prose validator: skill/ is clean`);
  } else {
    console.error(`\n❌ ${errors} prose issue(s) in skill/. See docs/STYLE.md.`);
  }
  return errors;
}

const { skills } = readSourceFiles(ROOT_DIR);
const errors =
  validateSkillFrontmatter(skills) +
  checkCounts(ROOT_DIR, skills) +
  validateProse(ROOT_DIR) +
  validateSkillProse(ROOT_DIR);

if (errors > 0) process.exit(1);
console.log('\n✨ Checks passed.');
