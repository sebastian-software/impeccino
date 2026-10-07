/**
 * Provider-backed workflow contract tests. Unlike scenarios.test.mjs, these
 * assert the attended turns and writes that make init/redesign/refinement real.
 */
import { describe, it } from 'vitest';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { tool } from 'ai';
import { z } from 'zod';

import {
  prepareWorkspace,
  cleanupWorkspace,
  runTurn as runHarnessTurn,
  fileLoaded,
  summarizeTrace,
  ENGINE_BIN,
  ENGINE_MISSING_MESSAGE,
} from '../skill-behavior/harness.mjs';
import { detectProvider, getModel, hasKey, resolveModelList, PROVIDERS } from '../skill-behavior/providers.mjs';
import { assertNewWorkLifecycle } from '../skill-behavior/assertions.mjs';
import { PRODUCT_MD_SAMPLE, DESIGN_MD_SAMPLE as ORIGINAL_DESIGN, CASE_STUDY_ANSWER } from '../skill-behavior/fixtures.mjs';
import { prepareBrowser } from './browser.mjs';
import { assertCompleted, assertFreshCaptures, assertDocumentationArtifacts } from './assertions.mjs';

const DESIGN_MD_SAMPLE = ORIGINAL_DESIGN.replace(/GT Sectra \(commercial\)/g, 'Georgia (system)').replace(/JetBrains Mono/g, 'monospace').replace(/Inter/g, 'Arial');

async function runTurn(options) {
  // Preflight happens before the first provider call. These are text-only
  // HTML fixtures: no dependencies, font downloads, or browser discovery.
  const browser = await prepareBrowser(options.workspace);
  try {
    const additionalTools = options.additionalTools;
    const result = await runHarnessTurn({
      ...options, maxSteps: 50, timeoutMs: 840000,
      userPrompt: `${options.userPrompt}\nUse system fonts and no external assets for this text-only fixture. The browser_snapshot and view_image tools are ready for visual review.`,
      environment: [browser.environment, options.environment].filter(Boolean).join('\n\n'),
      additionalTools: (trace) => ({ ...browser.tools(trace), ...(additionalTools?.(trace) ?? {}) }),
    });
    assertCompleted(result);
    const contextCalls = result.trace.toolCalls.filter(({ name, input }) => name === 'bash' && /impeccino\s+context\b/.test(input.command));
    assert.equal(contextCalls.length, 1, 'completed workflow must load context exactly once');
    return result;
  } finally {
    await browser.close();
  }
}

const LEGACY_DESIGN = `# Design

## Identity
BORING_BEIGE_CARDS. Quiet beige panels, timid scale, rounded cards everywhere.

## Color
Warm gray background with a muted tan accent.
`;

const EXISTING_PAGE = `<!doctype html>
<html><head><style>
:root { --legacy-beige: #e8e1d5; --legacy-tan: #a78969; }
body { background: var(--legacy-beige); color: #3c3833; font-family: Arial, sans-serif; }
.card { border: 1px solid #cfc5b6; border-radius: 18px; padding: 24px; }
</style></head><body>
<header data-untouched="header"><a href="/">Harbor Desk</a></header>
<main><section id="case-study" class="card"><h1>Harbor Desk</h1><p>Challenge. Approach. Outcome.</p><p>Image placeholder</p></section></main>
<footer data-untouched="footer">Operational since 1987</footer>
</body></html>`;

// Deliberately broken enough that any honest critique lists three or more
// Priority Issues, so the run cannot reach the "fewer than 3" skip branch by
// merit. Low contrast, an icon-tile stack, a kicker over the heading, dead
// hierarchy, and a placeholder CTA.
const FLAWED_PAGE = `<!doctype html>
<html><head><style>
body { background:#f4f4f5; color:#b9b9c0; font-family: Arial, sans-serif; font-size:15px; }
h1, h2, h3, p { font-size:15px; font-weight:400; margin:8px 0; }
.tile { width:48px; height:48px; background:#e6e6ea; border-radius:12px; }
.card { border:1px solid #e6e6ea; border-radius:12px; padding:16px; }
</style></head><body>
<main>
  <p class="kicker">INTRODUCING</p>
  <h1>Harbor Desk</h1>
  <p>A platform that helps teams do more of what matters, faster.</p>
  <section class="card"><div class="tile"></div><h3>Lightning Fast</h3><p>Blazing performance.</p></section>
  <section class="card"><div class="tile"></div><h3>Rock Solid</h3><p>Enterprise grade.</p></section>
  <section class="card"><div class="tile"></div><h3>Fully Secure</h3><p>Bank level security.</p></section>
  <button style="background:#e6e6ea;color:#c9c9d0;border:none;padding:8px 12px">Learn More</button>
</main>
</body></html>`;

/**
 * Flatten assistant output into ordered parts.
 *
 * `generateText` only returns `text` for the FINAL step, which is empty when a
 * turn ends on a tool call. Reading the report out of that field silently tests
 * nothing. Walking responseMessages instead preserves emission order, which is
 * the point: critique's invariant is that report prose precedes the question
 * inside the message, since prose after a structured question is withheld until
 * the user answers.
 */
function assistantParts(responseMessages) {
  const parts = [];
  for (const message of responseMessages) {
    if (message.role !== 'assistant') continue;
    const content = message.content;
    if (typeof content === 'string') {
      parts.push({ kind: 'text', value: content });
      continue;
    }
    for (const part of content ?? []) {
      if (part.type === 'text') parts.push({ kind: 'text', value: part.text ?? '' });
      else if (part.type === 'tool-call') parts.push({ kind: 'tool', value: part.toolName ?? '' });
    }
  }
  return parts;
}

function firstCall(trace, predicate) {
  return trace.toolCalls.findIndex(predicate);
}

function firstMutation(trace, pattern) {
  return firstCall(trace, ({ mutatedPaths = [] }) => mutatedPaths.some((file) => pattern.test(file)));
}

function workflowTraceMessage(trace) {
  return JSON.stringify(summarizeTrace(trace), null, 2);
}

function roleDefinition(workspace, filename) {
  const source = fs.readFileSync(path.join(workspace, '.claude/skills/impeccino/agents', filename), 'utf8');
  const tools = source.match(/^tools:\s*(.+)$/m)?.[1].split(',').map((name) => name.trim());
  const body = source.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '').trim();
  assert.ok(tools?.length, `${filename} must declare role tool limits`);
  return { body, tools };
}

/**
 * Simulates a host that has no installed native roles but can spawn a generic
 * child. It records the parent's handoff and returns scripted role reports;
 * it does not execute a child model or prove that the role completed its work.
 */
function genericRoleSpawnTool(trace, workspace) {
  const roles = [
    {
      id: 'finish-reviewer',
      filename: 'impeccino-finish-reviewer.md',
      result: `disposition: ship\n\npersistence\npass — PRODUCT.md exists.\n\nworld\nTYPE: matches the chosen system.\nMATERIAL: no unsupported material claims.\nGROUND: matches the documented palette.\n\nceiling\nreached\n\nmaterial_fixes\nnone\n\nkeep\nPreserve the approved reading surface.`,
    },
    {
      id: 'documenter',
      filename: 'impeccino-documenter.md',
      result: `No changes: checked PRODUCT.md, DESIGN.md, and index.html.\nPalette: white, near-black, blue links.\nType ramp: system-ui body, bold headings.\nNamed rules: single-column reading layout.\nNamed rules: no decorative containers.\nNamed rules: preserve keyboard affordances.\nDefects or drift not canonized: none found.`,
    },
  ].map((role) => ({ ...role, ...roleDefinition(workspace, role.filename) }));

  return {
    spawn_general_purpose_agent: tool({
      description: 'Start a fresh general-purpose subagent with role instructions, role tool limits, and task inputs. This test host records the request and returns a scripted role report; it does not execute a child model.',
      inputSchema: z.object({
        roleInstructions: z.string().describe('Full Markdown body from the shipped role file.'),
        tools: z.array(z.string()).describe('Tool limits from the role file frontmatter.'),
        task: z.string().describe('The inputs and task for this role.'),
      }),
      execute: async (input) => {
        const role = roles.find(({ body }) => input.roleInstructions.includes(body));
        trace.toolCalls.push({
          name: 'spawn_general_purpose_agent',
          input,
          role: role?.id,
          mutatedPaths: [],
        });
        return role?.result ?? 'Fixture error: include the full shipped role body in the instructions.';
      },
    }),
  };
}

function assertRoleLoadedBeforeGenericSpawn(trace, workspace, { id, filename }) {
  const role = roleDefinition(workspace, filename);
  const path = `.claude/skills/impeccino/agents/${filename}`;
  const read = trace.toolCalls.findIndex((call) => call.loadedFiles?.includes(path));
  const spawn = trace.toolCalls.findIndex((call) => call.name === 'spawn_general_purpose_agent' && call.role === id);
  assert.ok(read >= 0, `must load the shipped ${id} role body before spawning it.\n${workflowTraceMessage(trace)}`);
  assert.ok(spawn > read, `must read ${filename} before the generic spawn.\n${workflowTraceMessage(trace)}`);
  assert.equal(trace.toolCalls[spawn].input.roleInstructions, role.body);
  assert.deepEqual(trace.toolCalls[spawn].input.tools, role.tools);
}

// Full builds are separately opt-in and default to one provider. The existing
// model selection variable can explicitly request a cross-provider sweep.
for (const modelId of process.env.IMPECCINO_SKILL_BEHAVIOR_MODELS ? resolveModelList() : ['claude-sonnet-5']) {
  const provider = detectProvider(modelId);
  const keyPresent = hasKey(provider);

  describe(`skill workflow contract :: ${modelId}`, () => {
    if (!keyPresent) {
      it.skip(`skipped — ${PROVIDERS[provider].envKey} is unset`, () => {});
      return;
    }
    if (!ENGINE_BIN) {
      it.skip(`skipped — ${ENGINE_MISSING_MESSAGE}`, () => {});
      return;
    }
    const model = getModel(modelId);

    it('fresh init asks and writes PRODUCT without inventing a visual system', async () => {
      const workspace = prepareWorkspace({ files: {} });
      try {
        const { trace } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino init for a harbor operations product, then finish setup.',
        });
        const question = firstCall(trace, ({ name }) => name === 'ask_user_question');
        const productWrite = firstMutation(trace, /(^|\/)PRODUCT\.md$/i);
        assert.ok(fileLoaded(trace, 'init.md'), `init.md was not loaded.\n${workflowTraceMessage(trace)}`);
        assert.ok(question >= 0, `structured user was never asked.\n${workflowTraceMessage(trace)}`);
        assert.ok(productWrite > question, `PRODUCT.md must follow a user answer.\n${workflowTraceMessage(trace)}`);
        const product = fs.readFileSync(path.join(workspace, 'PRODUCT.md'), 'utf8');
        assert.doesNotMatch(product, /^## Register\s*$/im);
        assert.match(product, /ferry|dispatch|harbor/i, 'PRODUCT.md should incorporate the simulated user context');
        assert.equal(fs.existsSync(path.join(workspace, 'DESIGN.md')), false, 'init must not create DESIGN.md');
      } finally {
        cleanupWorkspace(workspace);
      }
    });

    it('an initialized build completes both inline role passes when the host has no subagent tool', async () => {
      const workspace = prepareWorkspace({
        files: { 'PRODUCT.md': PRODUCT_MD_SAMPLE, 'DESIGN.md': DESIGN_MD_SAMPLE },
      });
      try {
        for (const filename of ['impeccino-finish-reviewer.md', 'impeccino-documenter.md']) {
          assert.equal(fs.existsSync(path.join(workspace, '.claude/agents', filename)), false, `${filename} must be absent from the native-agent directory`);
        }
        const { trace } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino create a concise evidence-led case-study page. Ask me for its real content first, record the agreed surface brief before code, and leave it at index.html. Complete the shipped finish review and documentation check, locally if the host has no subagent tool.',
          simulatedUser: { answer: () => CASE_STUDY_ANSWER },
          environment: 'This host has no subagent tool and no native Impeccino roles.',
        });
        const question = firstCall(trace, ({ name }) => name === 'ask_user_question');
        assert.ok(fileLoaded(trace, 'new-work.md'), `new-work.md was not loaded.\n${workflowTraceMessage(trace)}`);
        assert.ok(question >= 0, `task concept was never put to the user.\n${workflowTraceMessage(trace)}`);
        assertNewWorkLifecycle(trace, { target: 'index.html', requireDiscovery: true, persistBrief: true });
        assertFreshCaptures(trace, workspace, 'index.html');
        assert.ok(fileLoaded(trace, 'impeccino-finish-reviewer.md'), 'inline fallback must read the shipped finish-reviewer role');
        assert.ok(fileLoaded(trace, 'impeccino-documenter.md'), 'inline fallback must read the shipped documenter role');
        assert.equal(trace.toolCalls.some(({ name }) => name === 'spawn_general_purpose_agent'), false, 'the no-subagent scenario must not claim a spawned reviewer or documenter');
        assert.equal(fs.existsSync(path.join(workspace, 'index.html')), true, 'new-work must still produce the requested artifact');
      } finally {
        cleanupWorkspace(workspace);
      }
    });

    it('an initialized build passes shipped role bodies to a generic subagent when native definitions are absent', async () => {
      const workspace = prepareWorkspace({
        files: { 'PRODUCT.md': PRODUCT_MD_SAMPLE, 'DESIGN.md': DESIGN_MD_SAMPLE },
      });
      try {
        for (const filename of ['impeccino-finish-reviewer.md', 'impeccino-documenter.md']) {
          assert.equal(fs.existsSync(path.join(workspace, '.claude/agents', filename)), false, `${filename} must be absent from the native-agent directory`);
        }
        const { trace } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino create a concise evidence-led case-study page. Ask me for its real content first, record the agreed surface brief before code, and leave it at index.html. Complete the shipped finish review and documentation check, locally if the host has no subagent tool.',
          simulatedUser: { answer: () => CASE_STUDY_ANSWER },
          environment: 'No named Impeccino roles are installed in .claude/agents. The host can spawn a fresh general-purpose subagent with role instructions, tool limits, and task inputs.',
          additionalTools: (trace) => genericRoleSpawnTool(trace, workspace),
        });
        const question = firstCall(trace, ({ name }) => name === 'ask_user_question');
        assert.ok(fileLoaded(trace, 'new-work.md'), `new-work.md was not loaded.\n${workflowTraceMessage(trace)}`);
        assert.ok(question >= 0, `task concept was never put to the user.\n${workflowTraceMessage(trace)}`);
        assertNewWorkLifecycle(trace, { target: 'index.html', requireDiscovery: true, persistBrief: true });
        assertFreshCaptures(trace, workspace, 'index.html');
        assertRoleLoadedBeforeGenericSpawn(trace, workspace, { id: 'finish-reviewer', filename: 'impeccino-finish-reviewer.md' });
        assertRoleLoadedBeforeGenericSpawn(trace, workspace, { id: 'documenter', filename: 'impeccino-documenter.md' });
        assert.equal(fs.existsSync(path.join(workspace, 'index.html')), true, 'new-work must still produce the requested artifact');
      } finally {
        cleanupWorkspace(workspace);
      }
    });

    it('redesign approves and records the direction before code, then documents the built world', async () => {
      const workspace = prepareWorkspace({
        files: {
          'PRODUCT.md': PRODUCT_MD_SAMPLE,
          'DESIGN.md': LEGACY_DESIGN,
          'current.html': EXISTING_PAGE,
        },
      });
      try {
        const { trace } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino redesign current.html for this product. Ask me to choose a replacement direction, record its surface brief before code, and leave the result at current.html. Complete the shipped finish review and record the built design system.',
        });
        const question = firstCall(trace, ({ name }) => name === 'ask_user_question');
        assert.ok(fileLoaded(trace, 'new-work.md'), `redesign did not route through new-work.\n${workflowTraceMessage(trace)}`);
        assert.ok(question >= 0, `replacement world was not put to the user.\n${workflowTraceMessage(trace)}`);
        assertNewWorkLifecycle(trace, { target: 'current.html', requireDiscovery: true, persistBrief: true, recordSystem: true });
        assertFreshCaptures(trace, workspace, 'current.html');
        assert.ok(fileLoaded(trace, 'finish-reviewer.md'), 'redesign must run the shipped finish review');
        assert.ok(fileLoaded(trace, 'documenter.md'), 'redesign must run the shipped documentation pass');
        const design = fs.readFileSync(path.join(workspace, 'DESIGN.md'), 'utf8');
        assert.notEqual(design.trim(), LEGACY_DESIGN.trim(), 'redesign preserved the old visual world verbatim');
        assertDocumentationArtifacts(design, fs.readFileSync(path.join(workspace, 'DESIGN.json'), 'utf8'));
      } finally {
        cleanupWorkspace(workspace);
      }
    });

    it('bolder refinement preserves the world and everything outside scope', async () => {
      const workspace = prepareWorkspace({
        files: {
          'PRODUCT.md': PRODUCT_MD_SAMPLE,
          'DESIGN.md': DESIGN_MD_SAMPLE,
          'current.html': EXISTING_PAGE,
        },
      });
      try {
        const { trace } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino bolder current.html, only the #case-study section. Keep everything else untouched.',
        });
        const productWrite = firstMutation(trace, /(^|\/)PRODUCT\.md$/i);
        const designWrite = firstMutation(trace, /(^|\/)DESIGN\.md$/i);
        const implementation = firstMutation(trace, /(^|\/)current\.html$/i);
        assert.ok(fileLoaded(trace, 'bolder.md'), `bolder.md was not loaded.\n${workflowTraceMessage(trace)}`);
        assert.equal(productWrite, -1, `refinement rewrote PRODUCT.md.\n${workflowTraceMessage(trace)}`);
        assert.equal(designWrite, -1, `refinement rewrote DESIGN.md.\n${workflowTraceMessage(trace)}`);
        assert.ok(implementation >= 0, `refinement did not write current.html.\n${workflowTraceMessage(trace)}`);
        assertFreshCaptures(trace, workspace, 'current.html');
        const artifact = fs.readFileSync(path.join(workspace, 'current.html'), 'utf8');
        assert.match(artifact, /data-untouched="header"/);
        assert.match(artifact, /data-untouched="footer"/);
        assert.match(artifact, /id="case-study"/);
      } finally {
        cleanupWorkspace(workspace);
      }
    });

    // A settled read-only review does not owe an interview or skip ceremony.
    it('critique returns evidence without a compulsory question', async () => {
      const workspace = prepareWorkspace({
        files: {
          'PRODUCT.md': PRODUCT_MD_SAMPLE,
          'DESIGN.md': DESIGN_MD_SAMPLE,
          'current.html': FLAWED_PAGE,
        },
      });
      try {
        const { trace, responseMessages } = await runTurn({
          workspace,
          model,
          userPrompt: '/impeccino critique current.html. Return the findings without a follow-up interview or edits.',
        });
        assert.ok(fileLoaded(trace, 'critique.md'), `critique.md was not loaded.\n${workflowTraceMessage(trace)}`);
        assertFreshCaptures(trace, workspace, 'current.html');

        const parts = assistantParts(responseMessages);
        const allText = parts.filter((p) => p.kind === 'text').map((p) => p.value).join('\n');
        const reportPattern = /finding|contrast|accessibility|priority issue/i;
        assert.match(allText, reportPattern, `no report reached the user.\n${workflowTraceMessage(trace)}`);

        assert.equal(trace.toolCalls.some(({ name }) => name === 'ask_user_question'), false,
          'a settled critique must not force an interview');
        assert.equal(trace.toolCalls.some(({ mutatedPaths = [] }) => mutatedPaths.length > 0), false,
          'a critique request must remain read-only');

      } finally {
        cleanupWorkspace(workspace);
      }
    });
  });
}
