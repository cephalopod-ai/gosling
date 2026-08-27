const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const YAML = require('yaml');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const reusable = fs
  .readFileSync(
    path.join(repositoryRoot, '.github', 'workflows', 'shell-package-reusable.yml'),
    'utf8'
  )
  .replaceAll('\r\n', '\n');
const caller = fs
  .readFileSync(
    path.join(repositoryRoot, '.github', 'workflows', 'shell-package-smoke.yml'),
    'utf8'
  )
  .replaceAll('\r\n', '\n');
const reusableDocument = YAML.parse(reusable);
const callerDocument = YAML.parse(caller);

test('shell package workflows parse as YAML documents', () => {
  assert.equal(typeof reusableDocument, 'object');
  assert.equal(typeof callerDocument, 'object');
});

test('shell package workflows are read-only, unsigned, and retain only bounded reports', () => {
  assert.match(reusable, /permissions:\n\s+contents: read/);
  assert.doesNotMatch(reusable, /secrets\./);
  assert.doesNotMatch(reusable, /release|publish|notar/i);
  const environment = reusableDocument.jobs['package-and-smoke'].env;
  assert.equal(environment.APPLE_TEAM_ID, '');
  assert.equal(environment.WINDOWS_CERTIFICATE_FILE, '');
  assert.match(reusable, /retention-days: 5/);
  assert.match(reusable, /if-no-files-found: warn/);
  assert.doesNotMatch(reusable, /ui\/desktop\/out\/|\.app\*|\.exe\*/);
});

test('shell package workflow runs contract, integration, readback, and lifecycle gates', () => {
  for (const command of [
    'shell:test-profile',
    'shell_session_runtime.test.ts',
    'shell:package-local',
    'shell:test-package-lifecycle',
    'shell:test-package-coexistence',
  ]) {
    assert.match(reusable, new RegExp(command.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  }
  assert.match(reusable, /persist-credentials: false/);
  assert.match(reusable, /--profile "\.\.\/\.\.\/\$\{PROFILE\}"/);
  assert.match(reusable, /--consumer "\.\.\/\.\.\/\$\{CONSUMER\}"/);
});

test('caller covers every supported target on PR, main, manual, and nightly triggers', () => {
  for (const value of [
    'macos-15',
    'macos-15-intel',
    'ubuntu-24.04',
    'windows-2025',
    'darwin',
    'linux',
    'win32',
    'arm64',
    'x64',
    'pull_request:',
    'push:',
    'schedule:',
    'workflow_dispatch:',
  ]) {
    assert.ok(caller.includes(value), `missing ${value}`);
  }
  assert.match(caller, /uses: \.\/\.github\/workflows\/shell-package-reusable\.yml/);
  assert.match(caller, /coexistence: true/);
  assert.match(caller, /permissions:\n\s+contents: read/);
});
