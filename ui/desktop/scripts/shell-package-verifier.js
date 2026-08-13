const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const asar = require('@electron/asar');
const plist = require('plist');
const {
  buildManifest,
  canonicalJson,
  parseJsonWithoutDuplicateKeys,
  resolveProfile,
} = require('./shell-profile');
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
  const resolved = resolveProfile(input.profileFile);
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
  const expectedManifest = buildManifest(resolved, target).manifest;
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
