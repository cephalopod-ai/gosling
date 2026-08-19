const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const { checkShellConformance, scaffoldShell } = require('./shell-scaffold');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const generated = [];

function uniqueSuffix() {
  return crypto.randomBytes(4).toString('hex');
}

function inputs(overrides = {}) {
  const suffix = overrides.suffix ?? uniqueSuffix();
  const name = `scaffold-${suffix}`;
  return {
    productDestination: `fixtures/shell-products/${name}`,
    consumerDestination: `fixtures/shell-consumers/${name}`,
    productId: `gosling-shell-${name}`,
    consumerId: name,
    displayName: `Scaffold ${suffix}`,
    runtimeNamespace: name,
    protocolScheme: `gosling-${name}`,
    macosBundleId: `io.github.repo-makeover.gosling.${name}`,
    windowsAppId: `Gosling.Shell.S${suffix}`,
    flatpakId: `io.github.repo_makeover.Gosling.S${suffix}`,
    ...overrides,
  };
}

function create(overrides) {
  const input = inputs(overrides);
  const result = scaffoldShell(input);
  generated.push(result.productDirectory, result.consumerDirectory);
  return { input, result };
}

function addIcons(productDirectory) {
  const assets = path.join(repositoryRoot, productDirectory, 'assets');
  for (const extension of ['.icns', '.ico', '.png', '.svg']) {
    fs.copyFileSync(
      path.join(repositoryRoot, 'fixtures/shell-products/fixture-a/assets', `icon${extension}`),
      path.join(assets, `icon${extension}`)
    );
  }
}

function cleanup() {
  for (const directory of generated.splice(0)) {
    fs.rmSync(path.join(repositoryRoot, directory), { recursive: true, force: true });
  }
}

test.after(cleanup);

test('emits a complete, non-production template and reports the operator inputs it cannot invent', () => {
  const { input, result } = create();
  try {
    assert.deepEqual(result.created, [
      `${input.consumerDestination}/README.md`,
      `${input.consumerDestination}/renderer.ts`,
      `${input.consumerDestination}/shell-consumer.json`,
      `${input.productDestination}/product-profile.json`,
      `${input.productDestination}/provisioning.json`,
    ]);
    assert.deepEqual(result.pendingOperatorInputs, [
      `${input.productDestination}/assets/icon.icns`,
      `${input.productDestination}/assets/icon.ico`,
      `${input.productDestination}/assets/icon.png`,
      `${input.productDestination}/assets/icon.svg`,
    ]);

    const profile = JSON.parse(
      fs.readFileSync(
        path.join(repositoryRoot, input.productDestination, 'product-profile.json'),
        'utf8'
      )
    );
    const provisioning = JSON.parse(
      fs.readFileSync(
        path.join(repositoryRoot, input.productDestination, 'provisioning.json'),
        'utf8'
      )
    );
    assert.equal(profile.update.enabled, false);
    assert.equal(profile.distribution.publishable, false);
    assert.equal(profile.distribution.signingPolicy, 'none');
    assert.equal(profile.distribution.releaseDestination, undefined);
    assert.equal(provisioning.session.credentialPolicy, 'fixed');
    assert.equal(provisioning.settingsSchemaVersion, 1);
    assert.deepEqual(provisioning.session.extensions, []);
    assert.deepEqual(provisioning.session.skillIds, []);
    assert.equal(provisioning.domainAdapter, undefined);
    assert.ok(provisioning.instructions.systemPrompt.length > 0);
    assert.doesNotMatch(
      JSON.stringify({ profile, provisioning }),
      /secret|token|password|dawes|physics|developer/i
    );
  } finally {
    cleanup();
  }
});

test('refuses an existing destination without changing what is already there', () => {
  const { input, result } = create();
  try {
    const manifest = path.join(repositoryRoot, result.consumerDirectory, 'shell-consumer.json');
    const before = fs.readFileSync(manifest);

    assert.throws(() => scaffoldShell(input), /already exists/);

    assert.deepEqual(fs.readFileSync(manifest), before);
  } finally {
    cleanup();
  }
});

test('refuses destinations outside the approved roots and unsafe paths', () => {
  for (const destination of [
    'ui/desktop/src/shell/injected',
    '../outside-repo',
    'fixtures/shell-products/../../etc/hijack',
    '/absolute/path',
    'fixtures\\shell-products\\backslash',
  ]) {
    assert.throws(
      () => scaffoldShell(inputs({ productDestination: destination })),
      /safe repository-relative path|approved roots|escapes the repository/
    );
  }
  assert.throws(
    () => scaffoldShell(inputs({ consumerDestination: 'fixtures/shell-products/wrong-root' })),
    /approved roots/
  );
});

test('refuses a destination reached through a symlinked ancestor', () => {
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-scaffold-outside-'));
  const link = path.join(repositoryRoot, 'fixtures/shell-products/scaffold-link');
  fs.symlinkSync(outside, link, 'dir');
  const input = inputs({ productDestination: 'fixtures/shell-products/scaffold-link/escaped' });
  try {
    assert.throws(() => scaffoldShell(input), /symlink|outside the approved roots/);
    assert.deepEqual(fs.readdirSync(outside), []);
  } finally {
    fs.unlinkSync(link);
    fs.rmSync(outside, { recursive: true, force: true });
  }
});

test('refuses secret-shaped and named-domain identities before writing anything', () => {
  for (const overrides of [
    { productId: 'gosling-shell-api-key' },
    { displayName: 'Physics Shell' },
    { runtimeNamespace: 'dawes-shell' },
    { protocolScheme: 'gosling-token' },
  ]) {
    const input = inputs(overrides);
    assert.throws(() => scaffoldShell(input), /secret-shaped|named-domain|is invalid/);
    assert.equal(fs.existsSync(path.join(repositoryRoot, input.productDestination)), false);
    assert.equal(fs.existsSync(path.join(repositoryRoot, input.consumerDestination)), false);
  }
});

test('refuses an identity that collides with an existing product profile', () => {
  for (const overrides of [
    { productId: 'gosling-shell-fixture-a' },
    { runtimeNamespace: 'shell-fixture-a' },
    { protocolScheme: 'gosling-fixture-a' },
    { macosBundleId: 'io.github.repo-makeover.gosling.fixture.a' },
  ]) {
    const input = inputs(overrides);
    assert.throws(() => scaffoldShell(input), /collides with an existing shell product profile/);
    assert.equal(fs.existsSync(path.join(repositoryRoot, input.productDestination)), false);
    assert.equal(fs.existsSync(path.join(repositoryRoot, input.consumerDestination)), false);
  }
});

test('escapes a display name that would otherwise break the generated renderer', () => {
  const { result } = create({ displayName: "Bob's \\ Shell" });
  try {
    const renderer = fs.readFileSync(
      path.join(repositoryRoot, result.consumerDirectory, 'renderer.ts'),
      'utf8'
    );
    assert.match(renderer, /root\.textContent = "Bob's \\\\ Shell conformance surface";/);
    assert.equal(checkShellConformance.length, 1);
  } finally {
    cleanup();
  }
});

test('a fresh second neutral identity is fully disjoint from the first', () => {
  const first = create();
  const second = create();
  try {
    addIcons(first.result.productDirectory);
    addIcons(second.result.productDirectory);

    const firstReport = checkShellConformance(
      path.join(repositoryRoot, first.result.consumerManifestPath)
    );
    const secondReport = checkShellConformance(
      path.join(repositoryRoot, second.result.consumerManifestPath)
    );

    assert.equal(firstReport.conformant, true);
    assert.equal(secondReport.conformant, true);
    assert.notEqual(firstReport.productId, secondReport.productId);
    assert.notEqual(firstReport.consumerId, secondReport.consumerId);
    assert.notEqual(firstReport.profileHash, secondReport.profileHash);
    assert.notEqual(firstReport.consumerHash, secondReport.consumerHash);
    assert.deepEqual(firstReport.requiredMethods, secondReport.requiredMethods);
  } finally {
    cleanup();
  }
});

test('conformance fails an incomplete template instead of certifying it', () => {
  const { result } = create();
  try {
    const manifest = path.join(repositoryRoot, result.consumerManifestPath);
    assert.throws(() => checkShellConformance(manifest), /iconBase/);

    addIcons(result.productDirectory);
    assert.equal(checkShellConformance(manifest).conformant, true);

    const provisioningFile = path.join(
      repositoryRoot,
      result.productDirectory,
      'provisioning.json'
    );
    const provisioning = JSON.parse(fs.readFileSync(provisioningFile, 'utf8'));
    delete provisioning.instructions;
    provisioning.session.extensions = [{ name: 'developer' }];
    delete provisioning.settingsSchemaVersion;
    fs.writeFileSync(provisioningFile, JSON.stringify(provisioning, null, 2));

    const report = checkShellConformance(manifest);
    assert.equal(report.conformant, false);
    assert.deepEqual(report.findings.sort(), [
      'provisioning must declare instructions.systemPrompt',
      'provisioning must declare settingsSchemaVersion 1',
      'provisioning must not enable the developer builtin',
    ]);
  } finally {
    cleanup();
  }
});

test('the committed neutral Default Shell sample stays conformant', () => {
  const report = checkShellConformance(
    path.join(repositoryRoot, 'fixtures/shell-consumers/default-shell-template/shell-consumer.json')
  );
  assert.equal(report.conformant, true);
  assert.deepEqual(report.findings, []);
  assert.deepEqual(report.declaredCapabilities, [
    'credential.select',
    'directory.select',
    'elicitation.respond',
    'permission.respond',
    'prompt.cancel',
    'prompt.submit',
    'session.artifacts.read',
    'session.create',
    'session.detach',
    'session.library.read',
    'session.library.write',
    'session.list',
    'session.resume',
    'session.transcript.read',
  ]);
  assert.deepEqual(report.requiredAgentCapabilities, ['loadSession', 'sessionList']);
  assert.ok(report.requiredMethods.includes('_gosling/unstable/shell/directory/validate'));
  assert.ok(report.requiredMethods.includes('_gosling/unstable/shell/session/library/resolve'));
  assert.ok(report.requiredMethods.includes('_gosling/unstable/shell/credentials/list'));
});
