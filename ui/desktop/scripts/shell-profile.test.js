const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { after, before, test } = require('node:test');
const {
  COLLISION_FIELDS,
  buildManifest,
  canonicalJson,
  discoverProfiles,
  parseJsonWithoutDuplicateKeys,
  resolveProfile,
  resolveProfiles,
  validateProfile,
  writeBuildResolution,
} = require('./shell-profile');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixtureRoot = path.join(repositoryRoot, 'fixtures', 'shell-products');
const fixtureA = path.join(fixtureRoot, 'fixture-a', 'product-profile.json');
const fixtureB = path.join(fixtureRoot, 'fixture-b', 'product-profile.json');
const scratchRoot = path.join(fixtureRoot, `.profile-test-${process.pid}`);
const buildScratch = path.join(repositoryRoot, 'build', `.profile-test-${process.pid}`);

function read(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function clone(value) {
  return structuredClone(value);
}

function fixtureRaw(name = 'fixture-a') {
  return read(path.join(fixtureRoot, name, 'product-profile.json'));
}

function writeProduct(name, raw, provisioning) {
  const directory = path.join(scratchRoot, name);
  const assets = path.join(directory, 'assets');
  fs.mkdirSync(assets, { recursive: true });
  for (const extension of ['.icns', '.ico', '.png', '.svg']) {
    fs.copyFileSync(path.join(fixtureRoot, 'fixture-a', 'assets', `icon${extension}`), path.join(assets, `icon${extension}`));
  }
  const relativeDirectory = path.relative(repositoryRoot, directory).split(path.sep).join('/');
  raw.provisioningPath = `${relativeDirectory}/provisioning.json`;
  raw.assets.root = `${relativeDirectory}/assets`;
  raw.assets.iconBase = `${relativeDirectory}/assets/icon`;
  fs.writeFileSync(path.join(directory, 'provisioning.json'), `${JSON.stringify(provisioning, null, 2)}\n`);
  const profile = path.join(directory, 'product-profile.json');
  fs.writeFileSync(profile, `${JSON.stringify(raw, null, 2)}\n`);
  return profile;
}

function matchingProvisioning(raw) {
  return {
    schemaVersion: 1,
    identity: {
      id: raw.product.id,
      displayName: raw.product.displayName,
      version: raw.product.version,
    },
    settingsAuthority: 'main_gosling',
    protocolPolicy: { mode: 'restricted', deniedMethods: [] },
    session: {},
  };
}

function validationError(mutator) {
  const raw = fixtureRaw();
  mutator(raw);
  assert.throws(() => validateProfile(raw, fixtureA));
}

before(() => {
  fs.mkdirSync(scratchRoot, { recursive: true });
});

after(() => {
  fs.rmSync(scratchRoot, { recursive: true, force: true });
  fs.rmSync(buildScratch, { recursive: true, force: true });
});

test('fixture profiles resolve deterministically with sorted canonical content and target assets', () => {
  const first = resolveProfile(fixtureA);
  const second = resolveProfile(fixtureA);
  assert.equal(first.profileHash, second.profileHash);
  assert.equal(first.profileJson, second.profileJson);
  assert.equal(first.profileHash.length, 64);
  assert.deepEqual(first.profile.compatibility.requiredMethods, [...first.profile.compatibility.requiredMethods].sort());
  assert.deepEqual(first.profile.assets.requiredTargets, [...first.profile.assets.requiredTargets].sort());
  assert.deepEqual(Object.keys(first.assetsByTarget), ['linux-x64', 'macos-arm64', 'macos-x64', 'windows-x64']);
  assert.deepEqual(Object.keys(first.assetsByTarget['linux-x64']), ['iconPng', 'iconSvg']);
  assert.match(first.assetsByTarget['macos-arm64'].icon, /icon\.icns$/);
  assert.match(first.assetsByTarget['windows-x64'].icon, /icon\.ico$/);
  assert.equal(first.resolvedGoslingRevision, execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repositoryRoot, encoding: 'utf8' }).trim());
});

test('fixture identities match the frozen contract and remain non-publishable', () => {
  const [a, b] = resolveProfiles([fixtureB, fixtureA]);
  assert.equal(a.profile.product.id, 'gosling-shell-fixture-a');
  assert.equal(b.profile.product.id, 'gosling-shell-fixture-b');
  for (const entry of [a, b]) {
    assert.equal(entry.profile.product.version, '0.0.0-test');
    assert.equal(entry.profile.distribution.publishable, false);
    assert.equal(entry.profile.distribution.signingPolicy, 'none');
    assert.equal(entry.profile.update.enabled, false);
  }
});

test('canonical JSON recursively sorts object keys without changing semantic array order', () => {
  assert.equal(canonicalJson({ z: 1, a: { y: 2, b: 3 }, list: ['z', 'a'] }), '{"a":{"b":3,"y":2},"list":["z","a"],"z":1}');
});

test('strict JSON parsing rejects malformed and duplicate keys without echoing content', () => {
  assert.throws(() => parseJsonWithoutDuplicateKeys('{"schemaVersion":', 'profile'), /^Error: profile is malformed JSON$/);
  assert.throws(() => parseJsonWithoutDuplicateKeys('{"secret-value":1,"secret-value":2}', 'profile'), (error) => {
    assert.equal(error.message, 'profile contains a duplicate JSON key');
    assert.doesNotMatch(error.message, /secret-value/);
    return true;
  });
});

test('unknown versions, fields, and runtime provisioning content fail closed', () => {
  validationError((raw) => { raw.schemaVersion = 2; });
  validationError((raw) => { raw.unreviewed = true; });
  validationError((raw) => { raw.workspaceId = 'workspace'; });
  validationError((raw) => { raw.product.provider = 'example'; });
});

test('secret-shaped keys and values are rejected without reflecting the value', () => {
  const raw = fixtureRaw();
  raw.product.displayName = 'bearer abcdefghijklmnopqrstuvwxyz';
  assert.throws(() => validateProfile(raw, fixtureA), (error) => {
    assert.match(error.message, /secret-shaped content/);
    assert.doesNotMatch(error.message, /abcdefghijklmnopqrstuvwxyz/);
    return true;
  });
  const keyed = fixtureRaw();
  keyed.update.apiToken = 'not-reflected';
  assert.throws(() => validateProfile(keyed, fixtureA), (error) => {
    assert.equal(error.message, 'profile.update contains a secret-shaped field');
    assert.doesNotMatch(error.message, /apiToken|not-reflected/);
    return true;
  });
});

test('all product and distribution identifiers enforce their field contracts', () => {
  const cases = [
    ['id', 'ab'],
    ['displayName', ' trailing '],
    ['version', '01.2.3'],
    ['runtimeNamespace', 'Uppercase'],
    ['protocolScheme', '1invalid'],
    ['executableName', '../binary'],
    ['macosBundleId', 'not-dotted'],
    ['windowsAppId', 'not-dotted'],
    ['linuxPackageName', 'UPPER'],
    ['flatpakId', 'not-dotted'],
  ];
  for (const [field, value] of cases) {
    const raw = fixtureRaw();
    raw.product[field] = value;
    assert.throws(() => validateProfile(raw, fixtureA), new RegExp(`profile\\.product\\.${field}`));
  }
  validationError((raw) => { raw.update.channel = 'UPPER'; });
  validationError((raw) => { raw.distribution.artifactPrefix = 'ab'; });
});

test('compatibility fields reject unsupported schemas, methods, revisions, and duplicate methods', () => {
  validationError((raw) => { raw.compatibility.goslingVersion = 'latest'; });
  validationError((raw) => { raw.compatibility.goslingRevision = 'deadbeef'; });
  validationError((raw) => { raw.compatibility.provisioningSchemaVersion = 2; });
  validationError((raw) => { raw.compatibility.handoffSchemaVersion = 2; });
  validationError((raw) => { raw.compatibility.requiredMethods = []; });
  validationError((raw) => { raw.compatibility.requiredMethods.push(raw.compatibility.requiredMethods[0]); });
  validationError((raw) => { raw.compatibility.requiredMethods = ['_gosling/unknown']; });
});

test('paths reject absolute, traversal, backslash, missing, wrong-type, and out-of-root inputs', () => {
  validationError((raw) => { raw.provisioningPath = path.join(os.tmpdir(), 'provisioning.json'); });
  validationError((raw) => { raw.provisioningPath = '../provisioning.json'; });
  validationError((raw) => { raw.provisioningPath = 'fixtures\\shell-products\\fixture-a\\provisioning.json'; });
  validationError((raw) => { raw.provisioningPath = 'fixtures/shell-products/fixture-a/missing.json'; });
  validationError((raw) => { raw.provisioningPath = 'fixtures/shell-products/fixture-a/assets'; });
  validationError((raw) => { raw.assets.root = 'ui/desktop/src/images'; });
});

test('profile files outside approved roots are rejected', () => {
  const outside = path.join(repositoryRoot, 'build', `outside-profile-${process.pid}.json`);
  fs.mkdirSync(path.dirname(outside), { recursive: true });
  fs.writeFileSync(outside, JSON.stringify(fixtureRaw()));
  try {
    assert.throws(() => resolveProfile(outside), /outside approved profile roots/);
  } finally {
    fs.rmSync(outside, { force: true });
  }
});

test('asset inventory rejects an extension, missing target asset, and symlinked asset root', () => {
  validationError((raw) => { raw.assets.iconBase += '.png'; });
  const raw = fixtureRaw();
  raw.assets.iconBase = 'fixtures/shell-products/fixture-a/assets/missing-icon';
  assert.throws(() => validateProfile(raw, fixtureA), /iconBase\.(?:icns|ico|png|svg) does not exist/);

  const link = path.join(scratchRoot, 'asset-link');
  fs.symlinkSync(path.join(fixtureRoot, 'fixture-a', 'assets'), link, 'dir');
  const linked = fixtureRaw();
  linked.assets.root = path.relative(repositoryRoot, link).split(path.sep).join('/');
  linked.assets.iconBase = `${linked.assets.root}/icon`;
  assert.throws(() => validateProfile(linked, fixtureA), /must not be a symlink/);
});

test('asset format and dimensions are validated instead of trusting file extensions', () => {
  const raw = fixtureRaw();
  const provisioning = matchingProvisioning(raw);
  const profile = writeProduct('invalid-assets', raw, provisioning);
  const directory = path.dirname(profile);
  fs.writeFileSync(path.join(directory, 'assets', 'icon.png'), 'not-a-png');
  assert.throws(() => resolveProfile(profile), /not a valid PNG icon/);

  fs.copyFileSync(
    path.join(fixtureRoot, 'fixture-a', 'assets', 'icon.png'),
    path.join(directory, 'assets', 'icon.png')
  );
  fs.writeFileSync(
    path.join(directory, 'assets', 'icon.svg'),
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 16"></svg>\n'
  );
  assert.throws(() => resolveProfile(profile), /square SVG viewBox/);
});

test('symlink escapes are rejected for referenced files', () => {
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-profile-'));
  const outsideFile = path.join(outside, 'provisioning.json');
  fs.writeFileSync(outsideFile, '{}');
  const link = path.join(scratchRoot, 'outside-provisioning.json');
  fs.symlinkSync(outsideFile, link);
  const raw = fixtureRaw();
  raw.provisioningPath = path.relative(repositoryRoot, link).split(path.sep).join('/');
  try {
    assert.throws(() => validateProfile(raw, fixtureA), /resolves outside the repository/);
  } finally {
    fs.rmSync(outside, { recursive: true, force: true });
  }
});

test('profile and provisioning identities must match exactly', () => {
  validationError((raw) => { raw.product.displayName = 'Different Name'; });
  validationError((raw) => { raw.product.version = '1.0.0'; });
});

test('update, signing, destination, publishability, and fixture promotion contradictions are rejected', () => {
  validationError((raw) => { raw.update.enabled = true; });
  validationError((raw) => { raw.update.owner = 'owner'; });
  validationError((raw) => { raw.distribution.signingPolicy = 'required'; });
  validationError((raw) => { raw.distribution.releaseDestination = 'production'; });
  validationError((raw) => {
    raw.distribution.publishable = true;
    raw.update.enabled = true;
    raw.update.owner = 'owner';
    raw.update.repository = 'repository';
    raw.distribution.signingPolicy = 'required';
    raw.distribution.releaseDestination = 'production';
  });
});

test('every identity-sensitive field is collision-checked only against the same field', () => {
  assert.equal(COLLISION_FIELDS.length, 10);
  const baseA = fixtureRaw('fixture-a');
  const baseB = fixtureRaw('fixture-b');
  for (const [index, [field]] of COLLISION_FIELDS.entries()) {
    const a = clone(baseA);
    const b = clone(baseB);
    const keys = field.split('.');
    const value = keys.reduce((entry, key) => entry[key], a);
    keys.slice(0, -1).reduce((entry, key) => entry[key], b)[keys.at(-1)] = value;
    const suffix = `${index}-${field.replaceAll('.', '-')}`;
    const fileA = writeProduct(`collision-${suffix}-a`, a, matchingProvisioning(a));
    const fileB = writeProduct(`collision-${suffix}-b`, b, matchingProvisioning(b));
    assert.throws(() => resolveProfiles([fileA, fileB]), new RegExp(`collision at ${field.replaceAll('.', '\\.')} `));
  }

  const crossFieldA = clone(baseA);
  const crossFieldB = clone(baseB);
  crossFieldB.product.runtimeNamespace = crossFieldA.product.id;
  const fileA = writeProduct('cross-field-a', crossFieldA, matchingProvisioning(crossFieldA));
  const fileB = writeProduct('cross-field-b', crossFieldB, matchingProvisioning(crossFieldB));
  assert.doesNotThrow(() => resolveProfiles([fileA, fileB]));
});

test('explicit revisions must match HEAD while current resolves to exact HEAD', () => {
  const current = resolveProfile(fixtureA);
  assert.match(current.resolvedGoslingRevision, /^[0-9a-f]{40}$/);
  const raw = fixtureRaw();
  raw.compatibility.goslingRevision = '0'.repeat(40);
  const file = writeProduct('wrong-revision', raw, matchingProvisioning(raw));
  assert.throws(() => resolveProfile(file), /does not match checkout HEAD/);
});

test('manifest resolution is deterministic, target-specific, and never embeds source paths', () => {
  const resolved = resolveProfile(fixtureA);
  const first = buildManifest(resolved, 'linux-x64');
  const second = buildManifest(resolved, 'linux-x64');
  assert.equal(first.manifestJson, second.manifestJson);
  assert.equal(first.manifest.target, 'linux-x64');
  assert.equal(first.manifest.platform, 'linux');
  assert.equal(first.manifest.architecture, 'x64');
  assert.equal(first.manifest.profileHash, resolved.profileHash);
  assert.equal(first.manifest.compatibility.goslingRevision, resolved.resolvedGoslingRevision);
  assert.doesNotMatch(first.manifestJson, new RegExp(repositoryRoot.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  assert.throws(() => buildManifest(resolved, 'windows-arm64'), /unsupported/);
});

test('build resolution writes canonical files atomically only under ignored build root', () => {
  const resolved = resolveProfile(fixtureA);
  const output = path.join(buildScratch, 'fixture-a', 'macos-arm64');
  const first = writeBuildResolution(resolved, 'macos-arm64', output);
  const bytes = fs.readFileSync(first.manifestOutput, 'utf8');
  const second = writeBuildResolution(resolved, 'macos-arm64', output);
  assert.equal(fs.readFileSync(second.manifestOutput, 'utf8'), bytes);
  assert.equal(fs.readFileSync(first.profileOutput, 'utf8'), resolved.profileJson);
  assert.deepEqual(fs.readdirSync(output).sort(), ['manifest.json', 'profile.json']);
  const relative = writeBuildResolution(
    resolved,
    'macos-arm64',
    path.relative(repositoryRoot, path.join(buildScratch, 'relative-output'))
  );
  assert.equal(relative.outputDirectory, path.join(buildScratch, 'relative-output'));
  assert.throws(() => writeBuildResolution(resolved, 'macos-arm64', os.tmpdir()), /build output/);
});

test('approved profile discovery returns the two source-controlled fixtures without following symlinks', () => {
  const link = path.join(scratchRoot, 'fixture-link');
  fs.symlinkSync(path.join(fixtureRoot, 'fixture-a'), link, 'dir');
  const discovered = discoverProfiles(repositoryRoot).filter((file) => !file.startsWith(scratchRoot));
  assert.deepEqual(discovered, [fixtureA, fixtureB]);
});
