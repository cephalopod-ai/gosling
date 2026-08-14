const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const desktopRoot = path.resolve(__dirname, '..');
const configPath = path.join(desktopRoot, 'forge.config.ts');
const fixtureA = path.resolve(
  desktopRoot,
  '..',
  '..',
  'fixtures',
  'shell-products',
  'fixture-a',
  'product-profile.json'
);
const consumerA = path.resolve(
  desktopRoot,
  '..',
  '..',
  'fixtures',
  'shell-consumers',
  'consumer-a',
  'shell-consumer.json'
);
const controlledEnvironment = [
  'APPLE_TEAM_ID',
  'APPLE_ID',
  'APPLE_ID_PASSWORD',
  'KEYCHAIN_PATH',
  'WINDOWS_CERTIFICATE_FILE',
  'WINDOW_SIGNING_ROLE',
  'GOSLING_SHELL_PROFILE',
  'GOSLING_SHELL_CONSUMER_MANIFEST',
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
  assert.deepEqual(config.packagerConfig.extraResource, [
    'src/bin',
    'src/images',
    'src/app-update.yml',
  ]);
  assert.equal(config.packagerConfig.protocols[0].schemes[0], 'gosling');
  assert.equal(config.packagerConfig.osxNotarize.teamId, 'team');
  assert.equal(config.packagerConfig.win32.certificateFile, 'certificate.pfx');
  assert.equal(config.publishers.length, 1);
  assert.equal(config.publishers[0].config.repository.name, 'gosling');
  assert.deepEqual(config.plugins[0].config, {
    build: [
      { entry: 'src/main.ts', config: 'vite.main.config.mts' },
      { entry: 'src/preload.ts', config: 'vite.preload.config.mts' },
    ],
    renderer: [{ name: 'main_window', config: 'vite.renderer.config.mts' }],
  });
});

test('fixture Forge config cannot enable signing, notarization, updater, or publication through environment', () => {
  const config = loadConfig({
    GOSLING_SHELL_PROFILE: fixtureA,
    GOSLING_SHELL_CONSUMER_MANIFEST: consumerA,
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
  assert.equal(config.packagerConfig.extendInfo, undefined);
  assert.ok(config.packagerConfig.extraResource.some((entry) => entry.endsWith('manifest.json')));
  assert.ok(
    config.packagerConfig.extraResource.every((entry) => !entry.includes('app-update.yml'))
  );
  assert.deepEqual(config.plugins[0].config, {
    build: [
      { entry: 'src/shell/main.ts', config: 'vite.shell.main.config.mts' },
      { entry: 'src/shell/preload.ts', config: 'vite.shell.preload.config.mts' },
    ],
    renderer: [{ name: 'shell_window', config: 'vite.shell.renderer.config.mts' }],
  });
  const flatpak = config.makers.find((maker) => maker.name === '@electron-forge/maker-flatpak');
  assert.deepEqual(flatpak.config.options.finishArgs, [
    '--share=ipc',
    '--socket=x11',
    '--socket=wayland',
    '--device=dri',
  ]);
  assert.equal(flatpak.config.options.modules, undefined);
});

test('consumer manifest selects the profile and embeds only its declared renderer capabilities', () => {
  const config = loadConfig({ GOSLING_SHELL_CONSUMER_MANIFEST: consumerA });
  assert.equal(config.packagerConfig.name, 'Gosling Shell Fixture A');
  const manifestResource = config.packagerConfig.extraResource.find((entry) =>
    entry.endsWith('manifest.json')
  );
  assert.ok(manifestResource);
  const manifest = JSON.parse(fs.readFileSync(path.resolve(desktopRoot, manifestResource), 'utf8'));
  assert.deepEqual(manifest.consumer, {
    consumerId: 'shell-consumer-a',
    consumerHash: manifest.consumer.consumerHash,
    rendererHash: manifest.consumer.rendererHash,
    declaredCapabilities: ['session.create'],
    requiredAgentCapabilities: ['loadSession'],
    requiredMethods: [
      '_gosling/unstable/session/info',
      '_gosling/unstable/shell/handoff/prepare',
      '_gosling/unstable/shell/provisioning/read',
      '_gosling/unstable/shell/provisioning/validate',
    ],
  });
  assert.match(manifest.consumer.consumerHash, /^[0-9a-f]{64}$/);
  assert.match(manifest.consumer.rendererHash, /^[0-9a-f]{64}$/);
});

test('Forge config rejects retired independent shell identity overrides', () => {
  assert.throws(
    () => loadConfig({ GOSLING_SHELL_PRODUCT_NAME: 'Override' }),
    /GOSLING_SHELL_PRODUCT_NAME is retired/
  );
});
