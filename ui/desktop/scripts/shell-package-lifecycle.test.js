const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const {
  assertEmptyRegistries,
  runPackageCoexistence,
  runPackageLifecycle,
} = require('./shell-package-lifecycle');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixture = (name) => ({
  profileFile: path.join(
    repositoryRoot,
    'fixtures',
    'shell-products',
    name,
    'product-profile.json'
  ),
  consumerFile: path.join(
    repositoryRoot,
    'fixtures',
    'shell-consumers',
    `consumer-${name.at(-1)}`,
    'shell-consumer.json'
  ),
  packageDirectory: path.join(
    '/packages',
    `Gosling Shell Fixture ${name.at(-1).toUpperCase()}-darwin-arm64`
  ),
  platform: 'darwin',
  architecture: 'arm64',
  target: 'macos-arm64',
  workingDirectory: repositoryRoot,
});

function controller(events, productId, root) {
  let generation = 1;
  let lifecycleState = 'ready';
  return {
    productId,
    root,
    async waitForState(expected) {
      events.push(`${productId}:wait:${expected}`);
      assert.equal(lifecycleState, expected);
      return { lifecycleState, generation };
    },
    async read() {
      events.push(`${productId}:read`);
      return { lifecycleState, generation };
    },
    async stop(value) {
      assert.equal(value, generation);
      lifecycleState = 'stopped';
      events.push(`${productId}:stop`);
      return { accepted: true, generation, state: lifecycleState };
    },
    async retry(value) {
      assert.equal(value, generation);
      assert.equal(lifecycleState, 'offline');
      generation += 1;
      lifecycleState = 'ready';
      events.push(`${productId}:retry`);
      return { accepted: true, generation, state: lifecycleState };
    },
    interruptBackend() {
      lifecycleState = 'offline';
      events.push(`${productId}:interrupt`);
      return 1234;
    },
    async close() {
      events.push(`${productId}:close`);
    },
    async terminate() {
      events.push(`${productId}:terminate`);
    },
  };
}

test('packaged lifecycle requires recovery, stop, clean close, and a second launch', async () => {
  const events = [];
  const result = await runPackageLifecycle(fixture('fixture-a'), {
    launch: async (input) => {
      events.push(`${input.productId}:launch`);
      return controller(events, input.productId, input.root);
    },
    inspectRegistries: () => {
      events.push('registry:empty');
      return 1;
    },
  });
  assert.equal(result.passed, true);
  assert.equal(result.observations.length, 2);
  assert.equal(events.filter((entry) => entry.endsWith(':launch')).length, 2);
  assert.equal(events.filter((entry) => entry.endsWith(':close')).length, 2);
  assert.equal(events.filter((entry) => entry === 'registry:empty').length, 2);
  assert.ok(events.includes('gosling-shell-fixture-a:stop'));
  assert.ok(events.includes('gosling-shell-fixture-a:interrupt'));
  assert.ok(events.includes('gosling-shell-fixture-a:retry'));
  assert.equal(
    events.some((entry) => entry.endsWith(':terminate')),
    false
  );
});

test('coexistence keeps the second product ready when the first exits', async () => {
  const events = [];
  const result = await runPackageCoexistence([fixture('fixture-a'), fixture('fixture-b')], {
    launch: async (input) => controller(events, input.productId, input.root),
    inspectRegistries: () => 1,
  });
  assert.deepEqual(result.productIds, ['gosling-shell-fixture-a', 'gosling-shell-fixture-b']);
  assert.equal(result.passed, true);
  const closeA = events.indexOf('gosling-shell-fixture-a:close');
  const readB = events.indexOf('gosling-shell-fixture-b:read');
  assert.ok(closeA >= 0 && readB > closeA);
});

test('coexistence refuses duplicate product identity before launching', async () => {
  let launches = 0;
  await assert.rejects(
    runPackageCoexistence([fixture('fixture-a'), fixture('fixture-a')], {
      launch: async () => {
        launches += 1;
      },
    }),
    /must be distinct/
  );
  assert.equal(launches, 0);
});

test('registry evidence requires one well-formed empty registry', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'shell-registry-test-'));
  try {
    assert.throws(() => assertEmptyRegistries(root), /exactly one/);
    const registry = path.join(root, 'app-data', 'product', 'backend-processes.json');
    fs.mkdirSync(path.dirname(registry), { recursive: true });
    fs.writeFileSync(registry, '{"version":1,"processes":[]}\n');
    assert.equal(assertEmptyRegistries(root), 1);
    fs.writeFileSync(registry, '{"version":1,"processes":[{"pid":1}]}\n');
    assert.throws(() => assertEmptyRegistries(root), /left a backend process/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
