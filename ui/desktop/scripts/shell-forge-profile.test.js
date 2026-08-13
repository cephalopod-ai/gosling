const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const {
  RETIRED_IDENTITY_ENV,
  defaultProjection,
  profileProjection,
  resolveForgeProjection,
  targetFor,
} = require('./shell-forge-profile');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixtureA = path.join(repositoryRoot, 'fixtures', 'shell-products', 'fixture-a', 'product-profile.json');

test('no profile preserves the existing Gosling Forge identity and resources', () => {
  assert.deepEqual(defaultProjection(), {
    shell: false,
    productName: 'Gosling',
    executableName: 'Gosling',
    version: undefined,
    protocolScheme: 'gosling',
    macosBundleId: undefined,
    windowsAppId: undefined,
    linuxPackageName: 'Gosling',
    flatpakId: 'io.github.repo_makeover.Gosling',
    iconBase: 'src/images/icon',
    iconIco: 'src/images/icon.ico',
    iconPng: 'src/images/icon.png',
    iconFlatpak512: 'src/images/icon-512.png',
    iconSvg: 'src/images/icon.svg',
    extraResource: ['src/bin', 'src/images', 'src/app-update.yml'],
    update: { enabled: true, owner: 'repo-makeover', repository: 'gosling' },
    resolved: undefined,
  });
  assert.deepEqual(resolveForgeProjection({}, 'darwin', 'arm64'), defaultProjection({}));
  const customPublisher = defaultProjection({ GITHUB_OWNER: 'custom-owner', GITHUB_REPO: 'custom-repo' });
  assert.equal(customPublisher.update.owner, 'custom-owner');
  assert.equal(customPublisher.update.repository, 'custom-repo');
});

test('a selected profile projects all Forge identities from one resolved source', () => {
  const projection = profileProjection(fixtureA, 'darwin', 'arm64');
  assert.equal(projection.shell, true);
  assert.equal(projection.productName, 'Gosling Shell Fixture A');
  assert.equal(projection.executableName, 'gosling-shell-fixture-a');
  assert.equal(projection.version, '0.0.0-test');
  assert.equal(projection.protocolScheme, 'gosling-fixture-a');
  assert.equal(projection.macosBundleId, 'io.github.repo_makeover.gosling.fixture.a');
  assert.equal(projection.windowsAppId, 'Gosling.Shell.Fixture.A');
  assert.equal(projection.linuxPackageName, 'gosling-shell-fixture-a');
  assert.equal(projection.flatpakId, 'io.github.repo_makeover.Gosling.FixtureA');
  assert.equal(projection.update.enabled, false);
  assert.match(projection.iconBase, /fixture-a\/assets\/icon$/);
  assert.equal(projection.iconIco, undefined);
  assert.equal(projection.extraResource.length, 5);
  assert.ok(projection.extraResource.every((entry) => !path.isAbsolute(entry)));
});

test('platform target selects only the required platform asset set', () => {
  const windows = profileProjection(fixtureA, 'win32', 'x64');
  assert.match(windows.iconIco, /icon\.ico$/);
  assert.equal(windows.iconBase, undefined);
  assert.equal(windows.iconPng, undefined);
  assert.equal(windows.iconFlatpak512, undefined);
  const linux = profileProjection(fixtureA, 'linux', 'x64');
  assert.match(linux.iconPng, /icon\.png$/);
  assert.match(linux.iconFlatpak512, /icon\.png$/);
  assert.match(linux.iconSvg, /icon\.svg$/);
  assert.equal(linux.iconIco, undefined);
});

test('retired independent environment identity overrides fail even without a profile', () => {
  for (const variable of RETIRED_IDENTITY_ENV) {
    assert.throws(
      () => resolveForgeProjection({ [variable]: 'override' }, 'darwin', 'arm64'),
      new RegExp(`${variable} is retired`)
    );
  }
});

test('only supported platform and architecture pairs map to profile targets', () => {
  assert.equal(targetFor('darwin', 'arm64'), 'macos-arm64');
  assert.equal(targetFor('darwin', 'x64'), 'macos-x64');
  assert.equal(targetFor('win32', 'x64'), 'windows-x64');
  assert.equal(targetFor('linux', 'x64'), 'linux-x64');
  assert.throws(() => targetFor('linux', 'arm64'), /unsupported Forge target/);
  assert.throws(() => targetFor('freebsd', 'x64'), /unsupported Forge target/);
});

test('ELECTRON_ARCH selects cross-architecture profile assets', () => {
  const projection = resolveForgeProjection(
    { GOSLING_SHELL_PROFILE: fixtureA, ELECTRON_ARCH: 'x64' },
    'darwin'
  );
  assert.match(projection.iconBase, /fixture-a\/assets\/icon$/);
  assert.ok(projection.resolved.profile.assets.requiredTargets.includes('macos-x64'));
});
