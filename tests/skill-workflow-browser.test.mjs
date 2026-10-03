import { it, vi } from 'vitest';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { prepareBrowser, imageOutput } from './skill-workflow/browser.mjs';
import { chromium } from 'playwright';

it('fails browser preflight with an actionable error before starting a workflow', async () => {
  const launch = vi.spyOn(chromium, 'launch').mockImplementation(async () => { throw new Error('browser missing'); });
  try {
    await assert.rejects(prepareBrowser('/unused'), /playwright install chromium/);
  } finally {
    launch.mockRestore();
  }
});

it('prepares real browser captures, interactions, and multimodal image results offline', async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'impeccino-workflow-browser-'));
  let browser;
  try {
    fs.writeFileSync(path.join(root, 'index.html'), '<!doctype html><title>Fixture</title><button onclick="this.textContent=\'Done\'">Start</button><img src="https://example.invalid/image.png">');
    browser = await prepareBrowser(root);
    const trace = { toolCalls: [] };
    const tools = browser.tools(trace);
    const capture = await tools.browser_snapshot.execute({ path: './index.html', viewport: 'desktop', click: 'button' });
    assert.equal(capture.target, 'index.html');
    assert.match(capture.text, /Done/);
    assert.ok(path.isAbsolute(capture.screenshot));
    assert.ok(fs.existsSync(capture.screenshot));
    const relativeCapture = path.relative(fs.realpathSync(root), fs.realpathSync(capture.screenshot));
    assert.ok(relativeCapture === '..' || relativeCapture.startsWith(`..${path.sep}`) || path.isAbsolute(relativeCapture),
      'review screenshots must live outside the project');
    assert.equal(capture.viewport, 'desktop');
    assert.equal(trace.toolCalls[0].name, 'browser_snapshot');
    const output = imageOutput({ output: capture });
    assert.equal(output.type, 'content');
    assert.ok(output.value.some((part) => part.mediaType === 'image/png'));
    const viewed = await tools.view_image.execute({ path: capture.screenshot });
    assert.equal(viewed.image, capture.image);
    const captures = await Promise.all(['desktop', 'mobile'].map((viewport) => tools.browser_snapshot.execute({ path: 'index.html', viewport })));
    assert.deepEqual(captures.map((result) => result.viewport), ['desktop', 'mobile']);
    assert.notEqual(captures[0].image, captures[1].image, 'parallel viewports must not share mutable page state');
    assert.ok(browser.blockedRequests.some((url) => url.includes('example.invalid')));
    await assert.rejects(tools.browser_snapshot.execute({ path: '../outside.html', viewport: 'mobile' }), /workspace/);
    await assert.rejects(tools.view_image.execute({ path: 'index.html' }), /PNG/);
    fs.symlinkSync(os.tmpdir(), path.join(root, 'escape'));
    const response = await fetch(`${browser.origin}/escape/`);
    assert.equal(response.status, 403);
  } finally {
    await browser?.close();
    fs.rmSync(root, { recursive: true, force: true });
  }
});
