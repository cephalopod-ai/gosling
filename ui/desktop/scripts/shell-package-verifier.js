const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const asar = require('@electron/asar');
const plist = require('plist');
const { buildManifest, canonicalJson, parseJsonWithoutDuplicateKeys } = require('./shell-profile');
const { resolveConsumerManifest } = require('./shell-consumer');
const { targetFor } = require('./shell-forge-profile');

const REQUIRED_ASAR_FILES = [
  '/.vite/build/main.js',
  '/.vite/build/shell-preload.js',
  '/.vite/renderer/shell_window/shell.html',
];
const FORBIDDEN_ASAR_FILES = ['/.vite/build/preload.js', '/.vite/renderer/main_window'];
const FORBIDDEN_MAIN_SENTINELS = ['get-setting', 'write-file', 'install-update', 'main_window'];
const FORBIDDEN_RENDERER_SENTINELS = [
  'getAcpUrl',
  'get-setting',
  'write-file',
  'GOSLING_SERVER__SECRET',
  'token=',
  'ws://127.0.0.1',
  'wss://127.0.0.1',
];
const MACOS_ELECTRON_RESOURCE_NAMES = new Set([
  'af.lproj',
  'am.lproj',
  'ar.lproj',
  'bg.lproj',
  'bn.lproj',
  'ca.lproj',
  'cs.lproj',
  'da.lproj',
  'de.lproj',
  'el.lproj',
  'electron.icns',
  'en.lproj',
  'en_GB.lproj',
  'es.lproj',
  'es_419.lproj',
  'et.lproj',
  'fa.lproj',
  'fi.lproj',
  'fil.lproj',
  'fr.lproj',
  'gu.lproj',
  'he.lproj',
  'hi.lproj',
  'hr.lproj',
  'hu.lproj',
  'id.lproj',
  'it.lproj',
  'ja.lproj',
  'kn.lproj',
  'ko.lproj',
  'lt.lproj',
  'lv.lproj',
  'ml.lproj',
  'mr.lproj',
  'ms.lproj',
  'nb.lproj',
  'nl.lproj',
  'pl.lproj',
  'pt_BR.lproj',
  'pt_PT.lproj',
  'ro.lproj',
  'ru.lproj',
  'sk.lproj',
  'sl.lproj',
  'sr.lproj',
  'sv.lproj',
  'sw.lproj',
  'ta.lproj',
  'te.lproj',
  'th.lproj',
  'tr.lproj',
  'uk.lproj',
  'ur.lproj',
  'vi.lproj',
  'zh_CN.lproj',
  'zh_TW.lproj',
]);

function fail(message) {
  throw new Error(message);
}

function sha256(contents) {
  return crypto.createHash('sha256').update(contents).digest('hex');
}

function readRequired(file, label) {
  try {
    return fs.readFileSync(file);
  } catch {
    fail(`${label} is missing or unreadable`);
  }
}

function parseJson(contents, label) {
  return parseJsonWithoutDuplicateKeys(contents.toString('utf8'), label);
}

function exactJson(actual, expected, label) {
  if (canonicalJson(actual) !== canonicalJson(expected))
    fail(`${label} does not match the resolved profile`);
}

function expectedPackageDirectory(desktopRoot, productName, platform, architecture) {
  return path.join(desktopRoot, 'out', `${productName}-${platform}-${architecture}`);
}

function packageLayout(profile, platform, architecture, packageDirectory) {
  const product = profile.product;
  const expectedName = `${product.displayName}-${platform}-${architecture}`;
  if (path.basename(packageDirectory) !== expectedName) {
    fail(`package directory name must be ${expectedName}`);
  }

  if (platform === 'darwin') {
    const app = path.join(packageDirectory, `${product.displayName}.app`);
    return {
      app,
      appAsar: path.join(app, 'Contents', 'Resources', 'app.asar'),
      appExecutable: path.join(app, 'Contents', 'MacOS', product.executableName),
      binary: path.join(app, 'Contents', 'Resources', 'bin', 'gosling'),
      metadata: path.join(app, 'Contents', 'Info.plist'),
      resources: path.join(app, 'Contents', 'Resources'),
    };
  }

  const executableSuffix = platform === 'win32' ? '.exe' : '';
  return {
    app: packageDirectory,
    appAsar: path.join(packageDirectory, 'resources', 'app.asar'),
    appExecutable: path.join(packageDirectory, `${product.executableName}${executableSuffix}`),
    binary: path.join(packageDirectory, 'resources', 'bin', `gosling${executableSuffix}`),
    metadata: null,
    resources: path.join(packageDirectory, 'resources'),
  };
}

function verifyMacMetadata(file, profile) {
  let metadata;
  try {
    metadata = plist.parse(readRequired(file, 'macOS Info.plist').toString('utf8'));
  } catch (error) {
    if (error instanceof Error && error.message.includes('missing or unreadable')) throw error;
    fail('macOS Info.plist is malformed');
  }
  const product = profile.product;
  if (metadata.CFBundleName !== product.displayName)
    fail('macOS product name does not match profile');
  if (metadata.CFBundleShortVersionString !== product.version)
    fail('macOS product version does not match profile');
  if (metadata.CFBundleIdentifier !== product.macosBundleId)
    fail('macOS bundle identifier does not match profile');
  if (metadata.CFBundleExecutable !== product.executableName)
    fail('macOS executable name does not match profile');
  const schemes = Array.isArray(metadata.CFBundleURLTypes)
    ? metadata.CFBundleURLTypes.flatMap((entry) => entry.CFBundleURLSchemes ?? [])
    : [];
  if (!schemes.includes(product.protocolScheme))
    fail('macOS protocol scheme does not match profile');
  for (const key of [
    'CFBundleDocumentTypes',
    'NSCalendarsUsageDescription',
    'NSRemindersUsageDescription',
  ]) {
    if (Object.hasOwn(metadata, key)) fail(`macOS shell metadata contains inherited ${key}`);
  }
}

function verifyResourceInventory(layout, resolved, target) {
  const targetAssets = resolved.assetsByTarget[target];
  const expected = new Set([
    'app.asar',
    'bin',
    'manifest.json',
    'profile.json',
    path.basename(resolved.provisioningPath),
    ...Object.values(targetAssets).map((file) => path.basename(file)),
  ]);
  const actual = fs.readdirSync(layout.resources).filter((name) => name !== '.DS_Store');
  const electronResources = actual.filter((name) => MACOS_ELECTRON_RESOURCE_NAMES.has(name));
  if (
    electronResources.length > 0 &&
    (electronResources.length !== MACOS_ELECTRON_RESOURCE_NAMES.size ||
      electronResources.some((name) => !MACOS_ELECTRON_RESOURCE_NAMES.has(name)))
  ) {
    fail('packaged Electron resource inventory is not exact');
  }
  const applicationResources = actual.filter((name) => !MACOS_ELECTRON_RESOURCE_NAMES.has(name));
  if (
    applicationResources.length !== expected.size ||
    applicationResources.some((name) => !expected.has(name))
  ) {
    fail('packaged shell resource inventory is not exact');
  }
  const binaryEntries = fs.readdirSync(path.dirname(layout.binary));
  if (binaryEntries.length !== 1 || binaryEntries[0] !== path.basename(layout.binary)) {
    fail('packaged shell binary directory is not exact');
  }
}

function extractText(appAsar, file, label) {
  try {
    return asar.extractFile(appAsar, file.replace(/^\//, '')).toString('utf8');
  } catch {
    fail(`${label} is missing from app.asar`);
  }
}

function verifyAsar(appAsar) {
  let entries;
  try {
    asar.uncache(appAsar);
    entries = new Set(asar.listPackage(appAsar));
  } catch {
    fail('app.asar is missing or unreadable');
  }
  for (const file of REQUIRED_ASAR_FILES) {
    if (!entries.has(file)) fail(`${file} is missing from app.asar`);
  }
  for (const file of FORBIDDEN_ASAR_FILES) {
    if ([...entries].some((entry) => entry === file || entry.startsWith(`${file}/`))) {
      fail(`${file} must not be present in a shell package`);
    }
  }

  const inspected = [...entries].filter(
    (entry) =>
      (entry === '/.vite/build/shell-preload.js' ||
        entry.startsWith('/.vite/renderer/shell_window/')) &&
      path.extname(entry) !== ''
  );
  for (const file of inspected) {
    const source = extractText(appAsar, file, file);
    const sentinel = FORBIDDEN_RENDERER_SENTINELS.find((value) => source.includes(value));
    if (sentinel) fail(`${file} contains forbidden renderer authority sentinel ${sentinel}`);
  }
  const mainSource = extractText(appAsar, '/.vite/build/main.js', 'shell main bundle');
  const mainSentinel = FORBIDDEN_MAIN_SENTINELS.find((value) => mainSource.includes(value));
  if (mainSentinel) fail(`shell main bundle contains broad Desktop sentinel ${mainSentinel}`);
}

function verifyShellPackage(input) {
  if (!input.consumerFile) fail('shell package verification requires a consumer manifest');
  const consumer = resolveConsumerManifest(input.consumerFile);
  const resolved = consumer.profile;
  if (resolved.profilePath !== path.resolve(input.profileFile)) {
    fail('shell profile and consumer manifest select different product profiles');
  }
  const profile = resolved.profile;
  const target = targetFor(input.platform, input.architecture);
  if (!profile.assets.requiredTargets.includes(target)) fail(`profile does not support ${target}`);
  if (
    profile.distribution.publishable ||
    profile.distribution.signingPolicy !== 'none' ||
    profile.update.enabled
  ) {
    fail('local shell package verification accepts only non-publishable unsigned profiles');
  }

  const layout = packageLayout(profile, input.platform, input.architecture, input.packageDirectory);
  const profileContents = readRequired(
    path.join(layout.resources, 'profile.json'),
    'packaged profile'
  );
  const manifestContents = readRequired(
    path.join(layout.resources, 'manifest.json'),
    'packaged manifest'
  );
  const provisioningContents = readRequired(
    path.join(layout.resources, path.basename(resolved.provisioningPath)),
    'packaged provisioning document'
  );
  const packagedProfile = parseJson(profileContents, 'packaged profile');
  const manifest = parseJson(manifestContents, 'packaged manifest');
  exactJson(packagedProfile, profile, 'packaged profile');
  if (sha256(Buffer.from(canonicalJson(packagedProfile))) !== manifest.profileHash) {
    fail('packaged profile hash does not match manifest');
  }
  exactJson(manifest.product, profile.product, 'packaged manifest identity');
  const expectedManifest = buildManifest(resolved, target, consumer).manifest;
  if (manifest.sourceClean !== expectedManifest.sourceClean) {
    fail('packaged manifest source state does not match the checkout');
  }
  if (canonicalJson(manifest) !== canonicalJson(expectedManifest)) {
    fail('packaged manifest does not match the resolved profile and checkout');
  }
  if (
    !provisioningContents.equals(
      readRequired(resolved.provisioningPath, 'source provisioning document')
    )
  ) {
    fail('packaged provisioning document does not match source');
  }

  readRequired(layout.appExecutable, 'packaged application executable');
  const packagedBinary = readRequired(layout.binary, 'packaged Gosling binary');
  const builtBinary = readRequired(input.builtBinary, 'just-built Gosling binary');
  if (sha256(packagedBinary) !== sha256(builtBinary)) {
    fail('packaged Gosling binary does not match the just-built binary');
  }
  if (fs.existsSync(path.join(layout.resources, 'app-update.yml'))) {
    fail('non-publishable shell package contains updater configuration');
  }
  verifyResourceInventory(layout, resolved, target);
  verifyAsar(layout.appAsar);
  if (layout.metadata) verifyMacMetadata(layout.metadata, profile);

  return {
    productId: profile.product.id,
    profileHash: manifest.profileHash,
    target,
    packageDirectory: layout.app,
    binaryHash: sha256(packagedBinary),
  };
}

module.exports = {
  FORBIDDEN_RENDERER_SENTINELS,
  REQUIRED_ASAR_FILES,
  expectedPackageDirectory,
  packageLayout,
  verifyShellPackage,
};
