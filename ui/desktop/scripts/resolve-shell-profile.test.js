const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { spawnSync } = require('node:child_process');
const { after, test } = require('node:test');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const cli = path.join(__dirname, 'resolve-shell-profile.js');
const fixtureA = path.join(repositoryRoot, 'fixtures', 'shell-products', 'fixture-a', 'product-profile.json');
const cliOutput = path.join(repositoryRoot, 'build', `cli-profile-test-${process.pid}`);

after(() => fs.rmSync(cliOutput, { recursive: true, force: true }));

function run(args, cwd = repositoryRoot) {
  return spawnSync(process.execPath, [cli, ...args], { cwd, encoding: 'utf8' });
}

test('CLI check emits deterministic machine-readable summary', () => {
  const result = run(['check', fixtureA]);
  assert.equal(result.status, 0, result.stderr);
  const output = JSON.parse(result.stdout);
  assert.equal(output.length, 1);
  assert.equal(output[0].productId, 'gosling-shell-fixture-a');
  assert.match(output[0].profileHash, /^[0-9a-f]{64}$/);
  assert.equal(result.stderr, '');
});

test('CLI reports field-addressed validation failures without source content', () => {
  const result = run(['check', 'does-not-exist-secret-value.json']);
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, 'profile file does not exist\n');
  assert.doesNotMatch(result.stderr, /secret-value/);
});

test('CLI rejects duplicate and unknown resolve options', () => {
  const duplicate = run([
    'resolve',
    fixtureA,
    '--target',
    'macos-arm64',
    '--target',
    'macos-x64',
  ]);
  assert.equal(duplicate.status, 1);
  assert.match(duplicate.stderr, /--target may be provided only once/);

  const unknown = run(['resolve', fixtureA, '--target', 'macos-arm64', '--unknown', 'value']);
  assert.equal(unknown.status, 1);
  assert.match(unknown.stderr, /^Usage:/);
});

test('CLI resolves relative build output against repository root, not caller cwd', () => {
  const output = path.relative(repositoryRoot, cliOutput);
  const result = run(['resolve', fixtureA, '--target', 'macos-arm64', '--output', output], path.join(repositoryRoot, 'ui', 'desktop'));
  assert.equal(result.status, 0, result.stderr);
  const resolved = JSON.parse(result.stdout);
  assert.equal(resolved.outputDirectory, path.join(repositoryRoot, output));
});
