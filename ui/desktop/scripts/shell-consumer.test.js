const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { resolveConsumerManifest } = require('./shell-consumer');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const consumerA = path.join(
  repositoryRoot,
  'fixtures',
  'shell-consumers',
  'consumer-a',
  'shell-consumer.json'
);
const consumerB = path.join(
  repositoryRoot,
  'fixtures',
  'shell-consumers',
  'consumer-b',
  'shell-consumer.json'
);

test('two neutral consumers resolve distinct renderers without changing their product profiles', () => {
  const alpha = resolveConsumerManifest(consumerA);
  const bravo = resolveConsumerManifest(consumerB);
  assert.equal(alpha.consumer.consumerId, 'shell-consumer-a');
  assert.equal(bravo.consumer.consumerId, 'shell-consumer-b');
  assert.match(alpha.rendererEntry.split(path.sep).join('/'), /consumer-a\/renderer\.ts$/);
  assert.match(bravo.rendererEntry.split(path.sep).join('/'), /consumer-b\/renderer\.ts$/);
  assert.equal(alpha.profile.profile.product.id, 'gosling-shell-fixture-a');
  assert.equal(bravo.profile.profile.product.id, 'gosling-shell-fixture-b');
  assert.deepEqual(alpha.requiredAgentCapabilities, ['loadSession']);
  assert.deepEqual(bravo.requiredAgentCapabilities, []);
  assert.ok(bravo.requiredMethods.includes('_gosling/unstable/shell/domain/action'));
  assert.ok(bravo.requiredMethods.includes('_gosling/unstable/shell/domain/action/confirm'));
  assert.ok(bravo.requiredMethods.includes('_gosling/unstable/shell/domain/snapshot'));
});

test('consumer manifests reject host authority, undeclared operations, and traversal', () => {
  const directory = fs.mkdtempSync(
    path.join(repositoryRoot, 'fixtures', 'shell-consumers', 'consumer-test-')
  );
  const manifest = path.join(directory, 'shell-consumer.json');
  fs.writeFileSync(path.join(directory, 'renderer.ts'), 'export {};\n');
  const base = {
    schemaVersion: 1,
    consumerId: 'consumer-test',
    requiredShellKit: 'current',
    productProfilePath: 'fixtures/shell-products/fixture-a/product-profile.json',
    rendererEntry: 'renderer.ts',
    declaredCapabilities: ['directory.select', 'session.create'],
  };
  try {
    fs.writeFileSync(manifest, JSON.stringify({ ...base, preloadEntry: 'src/shell/preload.ts' }));
    assert.throws(() => resolveConsumerManifest(manifest), /consumer\.preloadEntry is unknown/);
    fs.writeFileSync(manifest, JSON.stringify({ ...base, declaredCapabilities: ['process.exec'] }));
    assert.throws(
      () => resolveConsumerManifest(manifest),
      /consumer\.declaredCapabilities is invalid/
    );
    fs.writeFileSync(manifest, JSON.stringify({ ...base, declaredCapabilities: ['session.create'] }));
    assert.throws(
      () => resolveConsumerManifest(manifest),
      /session\.create requires directory\.select/
    );
    fs.writeFileSync(
      manifest,
      JSON.stringify({ ...base, declaredCapabilities: ['directory.select', 'session.detach'] })
    );
    assert.throws(
      () => resolveConsumerManifest(manifest),
      /session\.detach requires session\.create/
    );
    fs.writeFileSync(
      manifest,
      JSON.stringify({
        ...base,
        declaredCapabilities: [
          'directory.select',
          'session.create',
          'session.extensions.write',
        ],
      })
    );
    assert.throws(
      () => resolveConsumerManifest(manifest),
      /session\.extensions\.write requires session\.extensions\.read/
    );
    fs.writeFileSync(
      manifest,
      JSON.stringify({ ...base, rendererEntry: '../consumer-a/renderer.ts' })
    );
    assert.throws(
      () => resolveConsumerManifest(manifest),
      /consumer\.rendererEntry must be a safe consumer-relative path/
    );
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
