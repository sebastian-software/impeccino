import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';

const original = '---\nname: Record\ncolors:\n  primary: "#123456"\n---\n<!-- impeccino-disable side-tab: ink rule -->\n## Colors\nOriginal palette.\n## Iconography\nPreserve this exact foreign guidance.\n';
const draft = original.replace('#123456', '#654321').replace('Original palette.', 'Observed palette.');
const env = { IMPECCINO_CONTEXT_DIR: null, IMPECCINO_PROVIDER_ID: null };
const normalize = [['("previewRevision":\\s*)"[a-f0-9]{64}"', 'g', '$1"<PREVIEW_REVISION>"']];

const setup = (ws) => {
  fs.mkdirSync(path.join(ws, '.git'));
  fs.writeFileSync(path.join(ws, 'package.json'), '{}\n');
  fs.writeFileSync(path.join(ws, 'DESIGN.md'), original);
  fs.writeFileSync(path.join(ws, 'draft.md'), draft);
};

export default function cases() {
  const writeSteps = [{ args: ['preview', '--draft', 'draft.md'] }, { args: [] }];
  return [
    { id: 'design-document-validate-seed', verb: 'design-document', args: ['validate', '--draft', '-'], stdin: '---\nname: Seed\n---\n## Overview\nProvisional direction.\n', env },
    { id: 'design-document-validate-errors', verb: 'design-document', args: ['validate', '--draft', '-'], stdin: '---\ncolors: true\ncomponents:\n  button:\n    textColor: "{colors.missing}"\n---\n## Shapes\n## Colors\n', env },
    { id: 'design-document-preview-preservation', verb: 'design-document', setup, args: ['preview', '--draft', '-'], stdin: '---\nname: Record\n---\n## Colors\nObserved palette.\n', env, normalize, files: ['*'] },
    { id: 'design-document-preview-metadata-only', verb: 'design-document', setup, args: ['preview', '--draft', '-', '--metadata-only'], stdin: '<!-- impeccino:design-metadata -->\n```json\n{"schemaVersion":2,"extensions":{"roundedMeta":{"pill":"999px"}}}\n```\n', env, normalize, files: ['*'] },
    {
      id: 'design-document-reviewed-write', verb: 'design-document', env, normalize, files: ['*'], steps: writeSteps,
      setup(ws) {
        setup(ws);
        const report = JSON.parse(execFileSync(process.env.IMPECCINO_BIN, ['design-document', 'preview', '--draft', 'draft.md'], { cwd: ws, encoding: 'utf8' }));
        writeSteps[1].args = ['write', '--draft', 'draft.md', '--expected', report.previewRevision];
      },
    },
    { id: 'design-document-stale-write', verb: 'design-document', setup, args: ['write', '--draft', 'draft.md', '--expected', 'stale-preview'], env, files: ['*'] },
    { id: 'design-document-write-needs-preview', verb: 'design-document', setup, args: ['write', '--draft', 'draft.md'], env, files: ['*'] },
  ];
}
