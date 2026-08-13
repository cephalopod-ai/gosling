const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const desktopRoot = path.resolve(__dirname, '..');
const configPath = path.join(desktopRoot, 'forge.config.ts');
const fixtureA = path.resolve(desktopRoot, '..', '..', 'fixtures', 'shell-products', 'fixture-a', 'product-profile.json');
const controlledEnvironment = [
  'APPLE_TEAM_ID',
  'APPLE_ID',
  'APPLE_ID_PASSWORD',
  'KEYCHAIN_PATH',
  'WINDOWS_CERTIFICATE_FILE',
  'WINDOW_SIGNING_ROLE',
  'GOSLING_SHELL_PROFILE',
  'GOSLING_SHELL_PRODUCT_NAME',
  'GOSLING_SHELL_PROTOCOL_SCHEME',
  'GOSLING_SHELL_PACKAGE_ID',
  'ELECTRON_ARCH',
];

function loadConfig(values = {}) {
  const saved = Object.fromEntries(controlledEnvironment.map((name) => [name, process.env[name]]));
  for (const name of controlledEnvironment) delete process.env[name];
  Object.assign(process.env, values);
  delete require.cache[require.resolve(configPath)];
  try {
    return require(configPath);
  } finally {
    delete require.cache[require.resolve(configPath)];
    for (const name of controlledEnvironment) {
      if (saved[name] === undefined) delete process.env[name];
      else process.env[name] = saved[name];
    }
  }
}

test('default Gosling Forge config preserves packaging, updater, and signing behavior', () => {
  const config = loadConfig({
    APPLE_TEAM_ID: 'team',
    APPLE_ID: 'operator@example.invalid',
    APPLE_ID_PASSWORD: 'credential-reference',
    WINDOWS_CERTIFICATE_FILE: 'certificate.pfx',
    WINDOW_SIGNING_ROLE: 'role',
  });
  assert.equal(config.packagerConfig.name, 'Gosling');
  assert.equal(config.packagerConfig.executableName, 'Gosling');
  assert.equal(config.packagerConfig.icon, 'src/images/icon');
  assert.deepEqual(config.packagerConfig.extraResource, ['src/bin', 'src/images', 'src/app-update.yml']);
  assert.equal(config.packagerConfig.protocols[0].schemes[0], 'gosling');
  assert.equal(config.packagerConfig.osxNotarize.teamId, 'team');
  assert.equal(config.packagerConfig.win32.certificateFile, 'certificate.pfx');
  assert.equal(config.publishers.length, 1);
  assert.equal(config.publishers[0].config.repository.name, 'gosling');
});

test('fixture Forge config cannot enable signing, notarization, updater, or publication through environment', () => {
  const config = loadConfig({
    GOSLING_SHELL_PROFILE: fixtureA,
    APPLE_TEAM_ID: 'team',
    APPLE_ID: 'operator@example.invalid',
    APPLE_ID_PASSWORD: 'credential-reference',
    WINDOWS_CERTIFICATE_FILE: 'certificate.pfx',
    WINDOW_SIGNING_ROLE: 'role',
    GITHUB_OWNER: 'attacker',
    GITHUB_REPO: 'attacker',
  });
  assert.equal(config.packagerConfig.name, 'Gosling Shell Fixture A');
  assert.equal(config.packagerConfig.osxSign, undefined);
  assert.equal(config.packagerConfig.osxNotarize, undefined);
  assert.equal(config.packagerConfig.win32.certificateFile, undefined);
  assert.equal(config.packagerConfig.win32.signingRole, undefined);
  assert.deepEqual(config.publishers, []);
  assert.ok(config.packagerConfig.extraResource.some((entry) => entry.endsWith('manifest.json')));
  assert.ok(config.packagerConfig.extraResource.every((entry) => !entry.includes('app-update.yml')));
});

test('Forge config rejects retired independent shell identity overrides', () => {
  assert.throws(
    () => loadConfig({ GOSLING_SHELL_PRODUCT_NAME: 'Override' }),
    /GOSLING_SHELL_PRODUCT_NAME is retired/
  );
});
