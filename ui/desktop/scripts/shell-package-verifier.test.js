const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const asar = require('@electron/asar');
const plist = require('plist');
const { buildManifest, resolveProfile } = require('./shell-profile');
const { resolveConsumerManifest } = require('./shell-consumer');
const { verifyShellPackage } = require('./shell-package-verifier');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixtureA = path.join(
  repositoryRoot,
  'fixtures',
  'shell-products',
  'fixture-a',
  'product-profile.json'
);
const consumerA = path.join(
  repositoryRoot,
  'fixtures',
  'shell-consumers',
  'consumer-a',
  'shell-consumer.json'
);

async function syntheticPackage(consumerFile = consumerA) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-package-'));
  const packageDirectory = path.join(root, 'Gosling Shell Fixture A-darwin-arm64');
  const app = path.join(packageDirectory, 'Gosling Shell Fixture A.app');
  const resources = path.join(app, 'Contents', 'Resources');
  const binary = Buffer.from('just-built-gosling-binary');
  const builtBinary = path.join(root, 'target', 'gosling');
  fs.mkdirSync(path.dirname(builtBinary), { recursive: true });
  fs.writeFileSync(builtBinary, binary);
  fs.mkdirSync(path.join(resources, 'bin'), { recursive: true });
  fs.mkdirSync(path.join(app, 'Contents', 'MacOS'), { recursive: true });
  fs.writeFileSync(path.join(resources, 'bin', 'gosling'), binary);
  fs.writeFileSync(path.join(app, 'Contents', 'MacOS', 'gosling-shell-fixture-a'), 'app');

  const consumer = consumerFile ? resolveConsumerManifest(consumerFile) : undefined;
  const resolved = consumer ? consumer.profile : resolveProfile(fixtureA);
  const { manifestJson } = buildManifest(resolved, 'macos-arm64', consumer);
  fs.writeFileSync(path.join(resources, 'profile.json'), resolved.profileJson);
  fs.writeFileSync(path.join(resources, 'manifest.json'), manifestJson);
  fs.copyFileSync(resolved.provisioningPath, path.join(resources, 'provisioning.json'));
  fs.copyFileSync(resolved.assetsByTarget['macos-arm64'].icon, path.join(resources, 'icon.icns'));
  fs.writeFileSync(
    path.join(app, 'Contents', 'Info.plist'),
    plist.build({
      CFBundleName: resolved.profile.product.displayName,
      CFBundleShortVersionString: resolved.profile.product.version,
      CFBundleIdentifier: resolved.profile.product.macosBundleId,
      CFBundleExecutable: resolved.profile.product.executableName,
      CFBundleURLTypes: [
        {
          CFBundleURLName: 'FixtureProtocol',
          CFBundleURLSchemes: [resolved.profile.product.protocolScheme],
        },
      ],
    })
  );

  const asarSource = path.join(root, 'asar-source');
  fs.mkdirSync(path.join(asarSource, '.vite', 'build'), { recursive: true });
  fs.mkdirSync(path.join(asarSource, '.vite', 'renderer', 'shell_window', 'assets'), {
    recursive: true,
  });
  fs.writeFileSync(path.join(asarSource, '.vite', 'build', 'main.js'), 'main process owns backend');
  fs.writeFileSync(
    path.join(asarSource, '.vite', 'build', 'shell-preload.js'),
    'contextBridge.exposeInMainWorld("goslingShell", {})'
  );
  fs.writeFileSync(
    path.join(asarSource, '.vite', 'renderer', 'shell_window', 'shell.html'),
    '<meta http-equiv="Content-Security-Policy" content="connect-src none">'
  );
  fs.writeFileSync(
    path.join(asarSource, '.vite', 'renderer', 'shell_window', 'assets', 'shell.js'),
    'window.goslingShell.runtime.read()'
  );
  await asar.createPackage(asarSource, path.join(resources, 'app.asar'));
  return { app, builtBinary, consumerFile, packageDirectory, resources, root };
}

function verify(value) {
  return verifyShellPackage({
    profileFile: fixtureA,
    ...(value.consumerFile ? { consumerFile: value.consumerFile } : {}),
    platform: 'darwin',
    architecture: 'arm64',
    packageDirectory: value.packageDirectory,
    builtBinary: value.builtBinary,
  });
}

test('synthetic shell package passes exact resource and identity readback', async (t) => {
  const value = await syntheticPackage();
  t.after(() => fs.rmSync(value.root, { recursive: true, force: true }));
  const result = verify(value);
  assert.equal(result.productId, 'gosling-shell-fixture-a');
  assert.equal(result.target, 'macos-arm64');
  assert.equal(
    result.binaryHash,
    crypto.createHash('sha256').update('just-built-gosling-binary').digest('hex')
  );
});

test('tampered profile and manifest resources fail closed', async (t) => {
  const profile = await syntheticPackage();
  t.after(() => fs.rmSync(profile.root, { recursive: true, force: true }));
  const changedProfile = JSON.parse(fs.readFileSync(path.join(profile.resources, 'profile.json')));
  changedProfile.update.channel = 'tampered';
  fs.writeFileSync(path.join(profile.resources, 'profile.json'), JSON.stringify(changedProfile));
  assert.throws(() => verify(profile), /packaged profile does not match/);

  const manifest = await syntheticPackage();
  t.after(() => fs.rmSync(manifest.root, { recursive: true, force: true }));
  const changedManifest = JSON.parse(
    fs.readFileSync(path.join(manifest.resources, 'manifest.json'))
  );
  changedManifest.target = 'macos-x64';
  fs.writeFileSync(path.join(manifest.resources, 'manifest.json'), JSON.stringify(changedManifest));
  assert.throws(() => verify(manifest), /packaged manifest does not match/);
});

test('tampered binary, updater resource, or package identity fails closed', async (t) => {
  const binary = await syntheticPackage();
  t.after(() => fs.rmSync(binary.root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(binary.resources, 'bin', 'gosling'), 'stale-binary');
  assert.throws(() => verify(binary), /does not match the just-built binary/);

  const updater = await syntheticPackage();
  t.after(() => fs.rmSync(updater.root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(updater.resources, 'app-update.yml'), 'provider: github');
  assert.throws(() => verify(updater), /contains updater configuration/);

  const identity = await syntheticPackage();
  t.after(() => fs.rmSync(identity.root, { recursive: true, force: true }));
  const metadataPath = path.join(identity.app, 'Contents', 'Info.plist');
  const metadata = plist.parse(fs.readFileSync(metadataPath, 'utf8'));
  metadata.CFBundleIdentifier = 'io.invalid.fixture';
  fs.writeFileSync(metadataPath, plist.build(metadata));
  assert.throws(() => verify(identity), /bundle identifier does not match/);
});

test('unexpected resources, inherited macOS metadata, and consumer renderer provenance fail closed', async (t) => {
  const resource = await syntheticPackage();
  t.after(() => fs.rmSync(resource.root, { recursive: true, force: true }));
  fs.writeFileSync(path.join(resource.resources, 'unapproved-resource'), 'unexpected');
  assert.throws(() => verify(resource), /resource inventory is not exact/);

  const metadata = await syntheticPackage();
  t.after(() => fs.rmSync(metadata.root, { recursive: true, force: true }));
  const metadataPath = path.join(metadata.app, 'Contents', 'Info.plist');
  const parsed = plist.parse(fs.readFileSync(metadataPath, 'utf8'));
  parsed.NSCalendarsUsageDescription = 'inherited';
  fs.writeFileSync(metadataPath, plist.build(parsed));
  assert.throws(() => verify(metadata), /inherited NSCalendarsUsageDescription/);

  const consumer = await syntheticPackage(consumerA);
  t.after(() => fs.rmSync(consumer.root, { recursive: true, force: true }));
  assert.doesNotThrow(() => verify(consumer));
  const manifestPath = path.join(consumer.resources, 'manifest.json');
  const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  manifest.consumer.rendererHash = '0'.repeat(64);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
  assert.throws(() => verify(consumer), /packaged manifest does not match/);
});

test('broad preload and renderer authority sentinels fail closed', async (t) => {
  const broad = await syntheticPackage();
  t.after(() => fs.rmSync(broad.root, { recursive: true, force: true }));
  const extracted = path.join(broad.root, 'extracted');
  asar.extractAll(path.join(broad.resources, 'app.asar'), extracted);
  fs.writeFileSync(path.join(extracted, '.vite', 'build', 'preload.js'), 'broad preload');
  await asar.createPackage(extracted, path.join(broad.resources, 'app.asar'));
  assert.throws(() => verify(broad), /preload\.js must not be present/);

  const sentinel = await syntheticPackage();
  t.after(() => fs.rmSync(sentinel.root, { recursive: true, force: true }));
  const sentinelExtracted = path.join(sentinel.root, 'extracted');
  asar.extractAll(path.join(sentinel.resources, 'app.asar'), sentinelExtracted);
  fs.writeFileSync(
    path.join(sentinelExtracted, '.vite', 'build', 'shell-preload.js'),
    'const api = "getAcpUrl"'
  );
  await asar.createPackage(sentinelExtracted, path.join(sentinel.resources, 'app.asar'));
  assert.throws(() => verify(sentinel), /forbidden renderer authority sentinel getAcpUrl/);

  const broadMain = await syntheticPackage();
  t.after(() => fs.rmSync(broadMain.root, { recursive: true, force: true }));
  const broadMainExtracted = path.join(broadMain.root, 'extracted');
  asar.extractAll(path.join(broadMain.resources, 'app.asar'), broadMainExtracted);
  fs.writeFileSync(
    path.join(broadMainExtracted, '.vite', 'build', 'main.js'),
    'ipcMain.handle("get-setting", broadDesktopHandler)'
  );
  await asar.createPackage(broadMainExtracted, path.join(broadMain.resources, 'app.asar'));
  assert.throws(() => verify(broadMain), /broad Desktop sentinel get-setting/);
});
