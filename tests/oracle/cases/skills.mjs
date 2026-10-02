/**
 * The retired installer verbs (`install`, `link`, `update`, `check`, and the
 * legacy `skills` namespace). Impeccino no longer installs itself
 * (docs/adr/0003-no-self-installer.md); each verb exits 1 with a pointer to
 * skill/, without touching harness directories or the network.
 */
export default [
  { id: 'skills-install-retired', verb: 'install', args: [] },
  { id: 'skills-install-help-retired', verb: 'install', args: ['--help'] },
  { id: 'skills-link-retired', verb: 'link', args: ['--source=.impeccino'] },
  { id: 'skills-update-retired', verb: 'update', args: [] },
  { id: 'skills-check-retired', verb: 'check', args: [] },
  { id: 'skills-namespace-install-retired', verb: 'skills', args: ['install'] },
];
