const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const APPROVED_PROFILE_ROOTS = ['shell-products', 'fixtures/shell-products'];
const PROFILE_FILE_NAME = 'product-profile.json';
const TARGETS = new Set(['macos-arm64', 'macos-x64', 'windows-x64', 'linux-x64']);
const METHODS = new Set([
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
]);
const APPROVED_RELEASE_DESTINATIONS = new Set();
const SECRET_PATTERN =
  /(api.?key|authorization|bearer|cookie|credential.?value|password|private.?key|secret|token)/i;
const PRIVATE_KEY_PATTERN = /-----BEGIN [^-]*PRIVATE KEY-----/i;
const DOMAIN_KEY_PATTERN =
  /^(workspace(?:Id)?|credentialProfile(?:Id)?|provider|model|extensions?|skillIds?|deniedMethods?|domainAdapter|prompt|action|payload|handoff)$/i;
const LOWER_KEBAB = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/;
const REVERSE_DNS = /^[A-Za-z0-9]+(?:[._-][A-Za-z0-9]+){2,}$/;
const WINDOWS_ID = /^[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)+$/;
const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

function fail(message) {
  throw new Error(message);
}

function object(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function exactKeys(value, required, optional, label) {
  object(value, label);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (SECRET_PATTERN.test(key)) fail(`${label} contains a secret-shaped field`);
    if (DOMAIN_KEY_PATTERN.test(key)) fail(`${label}.${key} belongs to runtime provisioning`);
    if (!allowed.has(key)) fail(`${label}.${key} is unknown`);
  }
  for (const key of required) {
    if (!Object.hasOwn(value, key)) fail(`${label}.${key} is required`);
  }
}

function string(value, label, pattern, min, max) {
  if (
    typeof value !== 'string' ||
    value.trim() !== value ||
    value.length < min ||
    value.length > max ||
    !pattern.test(value)
  ) {
    fail(`${label} is invalid`);
  }
}

function assertSafeValues(value, label = 'profile') {
  if (typeof value === 'string' && (SECRET_PATTERN.test(value) || PRIVATE_KEY_PATTERN.test(value))) {
    fail(`${label} contains secret-shaped content`);
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertSafeValues(entry, `${label}[${index}]`));
    return;
  }
  if (value && typeof value === 'object') {
    for (const [key, entry] of Object.entries(value)) {
      if (SECRET_PATTERN.test(key)) fail(`${label} contains a secret-shaped field`);
      assertSafeValues(entry, `${label}.${key}`);
    }
  }
}

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical(value[key])])
    );
  }
  return value;
}

function canonicalJson(value) {
  return JSON.stringify(canonical(value));
}

function parseJsonWithoutDuplicateKeys(contents, label) {
  let parsed;
  try {
    parsed = JSON.parse(contents);
  } catch {
    fail(`${label} is malformed JSON`);
  }

  const stack = [];
  for (let index = 0; index < contents.length; index += 1) {
    const character = contents[index];
    if (character === '"') {
      const start = index;
      index += 1;
      while (index < contents.length) {
        if (contents[index] === '\\') {
          index += 2;
          continue;
        }
        if (contents[index] === '"') break;
        index += 1;
      }
      const context = stack.at(-1);
      if (context?.type === 'object' && context.expectingKey) {
        const key = JSON.parse(contents.slice(start, index + 1));
        if (context.keys.has(key)) fail(`${label} contains a duplicate JSON key`);
        context.keys.add(key);
        context.expectingKey = false;
      }
    } else if (character === '{') {
      stack.push({ type: 'object', expectingKey: true, keys: new Set() });
    } else if (character === '[') {
      stack.push({ type: 'array' });
    } else if (character === '}' || character === ']') {
      stack.pop();
    } else if (character === ',' && stack.at(-1)?.type === 'object') {
      stack.at(-1).expectingKey = true;
    }
  }
  return parsed;
}

function readJson(file, label) {
  let contents;
  try {
    contents = fs.readFileSync(file, 'utf8');
  } catch {
    fail(`${label} cannot be read`);
  }
  return parseJsonWithoutDuplicateKeys(contents, label);
}

function repositoryRoot(profilePath) {
  let current = path.dirname(profilePath);
  while (current !== path.dirname(current)) {
    if (
      fs.existsSync(path.join(current, 'Cargo.toml')) &&
      fs.existsSync(path.join(current, 'ui', 'desktop'))
    ) {
      return fs.realpathSync(current);
    }
    current = path.dirname(current);
  }
  fail('profile is not inside a Gosling repository');
}

function isContained(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function approvedRoots(root) {
  return APPROVED_PROFILE_ROOTS.map((entry) => path.join(root, entry)).filter((entry) =>
    fs.existsSync(entry)
  );
}

function assertApproved(root, candidate, label) {
  if (!approvedRoots(root).some((approved) => isContained(fs.realpathSync(approved), candidate))) {
    fail(`${label} is outside approved profile roots`);
  }
}

function relativeCandidate(root, candidate, label) {
  if (
    typeof candidate !== 'string' ||
    !candidate ||
    path.isAbsolute(candidate) ||
    candidate.includes('\0') ||
    candidate.includes('\\') ||
    candidate.split('/').includes('..')
  ) {
    fail(`${label} must be a safe repository-relative path`);
  }
  const resolved = path.resolve(root, candidate);
  if (!isContained(root, resolved)) fail(`${label} escapes the repository`);
  return resolved;
}

function containedPath(root, candidate, label, kind, requireNonSymlink = false) {
  const resolved = relativeCandidate(root, candidate, label);
  let stat;
  let real;
  try {
    stat = fs.lstatSync(resolved);
    if (requireNonSymlink && stat.isSymbolicLink()) fail(`${label} must not be a symlink`);
    real = fs.realpathSync(resolved);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith(label)) throw error;
    fail(`${label} does not exist`);
  }
  if (!isContained(root, real)) fail(`${label} resolves outside the repository`);
  assertApproved(root, real, label);
  const realStat = fs.statSync(real);
  if (
    (kind === 'file' && !realStat.isFile()) ||
    (kind === 'directory' && !realStat.isDirectory())
  ) {
    fail(`${label} has the wrong type`);
  }
  return real;
}

function validatePng(contents, label) {
  const signature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  if (
    contents.length < 45 ||
    !contents.subarray(0, 8).equals(signature) ||
    contents.readUInt32BE(8) !== 13 ||
    contents.toString('ascii', 12, 16) !== 'IHDR' ||
    contents.readUInt32BE(contents.length - 12) !== 0 ||
    contents.toString('ascii', contents.length - 8, contents.length - 4) !== 'IEND'
  ) {
    fail(`${label} is not a valid PNG icon`);
  }
  const width = contents.readUInt32BE(16);
  const height = contents.readUInt32BE(20);
  if (width !== height || width < 32 || width > 1024) {
    fail(`${label} must be a square PNG icon between 32 and 1024 pixels`);
  }
}

function validateIco(contents, label) {
  if (
    contents.length < 22 ||
    contents.readUInt16LE(0) !== 0 ||
    contents.readUInt16LE(2) !== 1 ||
    contents.readUInt16LE(4) < 1
  ) {
    fail(`${label} is not a valid ICO icon`);
  }
  const width = contents[6] || 256;
  const height = contents[7] || 256;
  const size = contents.readUInt32LE(14);
  const offset = contents.readUInt32LE(18);
  if (width !== height || width < 32 || offset < 22 || size < 1 || offset + size > contents.length) {
    fail(`${label} is not a valid square ICO icon`);
  }
  const payload = contents.subarray(offset, offset + size);
  if (payload.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))) {
    validatePng(payload, label);
  }
}

function validateIcns(contents, label) {
  if (
    contents.length < 16 ||
    contents.toString('ascii', 0, 4) !== 'icns' ||
    contents.readUInt32BE(4) !== contents.length
  ) {
    fail(`${label} is not a valid ICNS icon`);
  }
  let offset = 8;
  let iconFound = false;
  while (offset + 8 <= contents.length) {
    const size = contents.readUInt32BE(offset + 4);
    if (size < 8 || offset + size > contents.length) fail(`${label} is not a valid ICNS icon`);
    const payload = contents.subarray(offset + 8, offset + size);
    if (payload.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))) {
      validatePng(payload, label);
      iconFound = true;
    }
    offset += size;
  }
  if (!iconFound || offset !== contents.length) fail(`${label} contains no supported ICNS icon`);
}

function validateSvg(contents, label) {
  const source = contents.toString('utf8');
  const opening = source.match(/<svg\b[^>]*>/i)?.[0];
  const viewBox = opening?.match(/\bviewBox\s*=\s*["']\s*0\s+0\s+(\d+)\s+(\d+)\s*["']/i);
  if (!opening || /<!DOCTYPE/i.test(source) || !viewBox) fail(`${label} is not a safe SVG icon`);
  const width = Number(viewBox[1]);
  const height = Number(viewBox[2]);
  if (width !== height || width < 32 || width > 1024) {
    fail(`${label} must have a square SVG viewBox between 32 and 1024`);
  }
}

function validateAsset(file, label) {
  const contents = fs.readFileSync(file);
  const extension = path.extname(file).toLowerCase();
  if (extension === '.png') validatePng(contents, label);
  else if (extension === '.ico') validateIco(contents, label);
  else if (extension === '.icns') validateIcns(contents, label);
  else if (extension === '.svg') validateSvg(contents, label);
  else fail(`${label} has an unsupported icon format`);
}

function assetInventory(root, assetRoot, iconBase, targets) {
  if (typeof iconBase !== 'string' || path.extname(iconBase)) {
    fail('profile.assets.iconBase must be a repository-relative path stem');
  }
  const base = relativeCandidate(root, iconBase, 'profile.assets.iconBase');
  if (!isContained(assetRoot, base) || base === assetRoot) {
    fail('profile.assets.iconBase must be under profile.assets.root');
  }

  const icon = (extension) => {
    const label = `profile.assets.iconBase${extension}`;
    const file = containedPath(root, `${iconBase}${extension}`, label, 'file');
    validateAsset(file, label);
    return file;
  };
  return Object.fromEntries(
    [...targets].sort().map((target) => {
      if (target.startsWith('macos-')) return [target, { icon: icon('.icns') }];
      if (target === 'windows-x64') return [target, { icon: icon('.ico') }];
      return [target, { iconPng: icon('.png'), iconSvg: icon('.svg') }];
    })
  );
}

function gitOutput(root, args) {
  try {
    return execFileSync('git', ['-C', root, ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    }).trim();
  } catch {
    fail('profile checkout Git metadata is unavailable');
  }
}

function currentRevision(root) {
  const revision = gitOutput(root, ['rev-parse', 'HEAD']);
  if (!/^[0-9a-f]{40}$/.test(revision)) fail('profile checkout revision is invalid');
  return revision;
}

function checkoutIsClean(root) {
  if (gitOutput(root, ['status', '--porcelain', '--untracked-files=normal']) !== '') return false;
  try {
    execFileSync('git', ['-C', root, 'diff', '--quiet', '--no-ext-diff']);
    execFileSync('git', ['-C', root, 'diff', '--cached', '--quiet', '--no-ext-diff']);
    return true;
  } catch {
    return false;
  }
}

function validateProfile(raw, profilePath) {
  exactKeys(
    raw,
    ['schemaVersion', 'product', 'provisioningPath', 'compatibility', 'assets', 'update', 'distribution'],
    [],
    'profile'
  );
  if (raw.schemaVersion !== 1) fail('profile.schemaVersion must be 1');
  assertSafeValues(raw);

  const product = raw.product;
  exactKeys(
    product,
    [
      'id',
      'displayName',
      'version',
      'runtimeNamespace',
      'protocolScheme',
      'executableName',
      'macosBundleId',
      'windowsAppId',
      'linuxPackageName',
      'flatpakId',
    ],
    [],
    'profile.product'
  );
  string(product.id, 'profile.product.id', LOWER_KEBAB, 3, 48);
  string(product.displayName, 'profile.product.displayName', /^.{1,64}$/, 1, 64);
  string(product.version, 'profile.product.version', SEMVER, 5, 64);
  string(product.runtimeNamespace, 'profile.product.runtimeNamespace', LOWER_KEBAB, 3, 64);
  string(product.protocolScheme, 'profile.product.protocolScheme', /^[a-z][a-z0-9-]*$/, 3, 32);
  string(product.executableName, 'profile.product.executableName', LOWER_KEBAB, 1, 64);
  string(product.macosBundleId, 'profile.product.macosBundleId', REVERSE_DNS, 3, 255);
  string(product.windowsAppId, 'profile.product.windowsAppId', WINDOWS_ID, 3, 128);
  string(product.linuxPackageName, 'profile.product.linuxPackageName', LOWER_KEBAB, 1, 64);
  string(product.flatpakId, 'profile.product.flatpakId', REVERSE_DNS, 3, 255);

  const compatibility = raw.compatibility;
  exactKeys(
    compatibility,
    [
      'goslingVersion',
      'goslingRevision',
      'provisioningSchemaVersion',
      'handoffSchemaVersion',
      'requiredMethods',
    ],
    [],
    'profile.compatibility'
  );
  string(compatibility.goslingVersion, 'profile.compatibility.goslingVersion', SEMVER, 5, 64);
  if (
    compatibility.goslingRevision !== 'current' &&
    !/^[0-9a-f]{40}$/.test(compatibility.goslingRevision)
  ) {
    fail('profile.compatibility.goslingRevision is invalid');
  }
  if (
    compatibility.provisioningSchemaVersion !== 1 ||
    compatibility.handoffSchemaVersion !== 1
  ) {
    fail('profile compatibility schema versions must be 1');
  }
  if (
    !Array.isArray(compatibility.requiredMethods) ||
    compatibility.requiredMethods.length === 0 ||
    new Set(compatibility.requiredMethods).size !== compatibility.requiredMethods.length ||
    !compatibility.requiredMethods.every((method) => METHODS.has(method))
  ) {
    fail('profile.compatibility.requiredMethods is invalid');
  }

  const assets = raw.assets;
  exactKeys(assets, ['root', 'iconBase', 'requiredTargets'], [], 'profile.assets');
  if (
    !Array.isArray(assets.requiredTargets) ||
    assets.requiredTargets.length === 0 ||
    new Set(assets.requiredTargets).size !== assets.requiredTargets.length ||
    !assets.requiredTargets.every((target) => TARGETS.has(target))
  ) {
    fail('profile.assets.requiredTargets is invalid');
  }

  const update = raw.update;
  exactKeys(update, ['enabled', 'channel'], ['owner', 'repository'], 'profile.update');
  if (typeof update.enabled !== 'boolean') fail('profile.update.enabled must be boolean');
  string(update.channel, 'profile.update.channel', LOWER_KEBAB, 3, 64);
  for (const field of ['owner', 'repository']) {
    if (update[field] !== undefined) {
      string(update[field], `profile.update.${field}`, /^[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?$/, 1, 100);
    }
  }
  if (update.enabled && (!update.owner || !update.repository)) {
    fail('profile.update enabled requires owner and repository');
  }
  if (!update.enabled && (update.owner || update.repository)) {
    fail('profile.update disabled cannot name a repository');
  }

  const distribution = raw.distribution;
  exactKeys(
    distribution,
    ['publishable', 'artifactPrefix', 'signingPolicy'],
    ['releaseDestination'],
    'profile.distribution'
  );
  if (typeof distribution.publishable !== 'boolean') {
    fail('profile.distribution.publishable must be boolean');
  }
  string(distribution.artifactPrefix, 'profile.distribution.artifactPrefix', LOWER_KEBAB, 3, 64);
  if (!['none', 'required'].includes(distribution.signingPolicy)) {
    fail('profile.distribution.signingPolicy is invalid');
  }
  if (distribution.releaseDestination !== undefined) {
    string(
      distribution.releaseDestination,
      'profile.distribution.releaseDestination',
      LOWER_KEBAB,
      3,
      64
    );
    if (!APPROVED_RELEASE_DESTINATIONS.has(distribution.releaseDestination)) {
      fail('profile.distribution.releaseDestination is not approved');
    }
  }
  if (
    !distribution.publishable &&
    (update.enabled || distribution.releaseDestination || distribution.signingPolicy !== 'none')
  ) {
    fail('non-publishable profiles cannot update, sign, or release');
  }
  const fixtureShaped = [product.id, product.runtimeNamespace, distribution.artifactPrefix].some(
    (value) => value.includes('fixture')
  );
  if (
    distribution.publishable &&
    (!update.enabled ||
      !distribution.releaseDestination ||
      distribution.signingPolicy !== 'required' ||
      fixtureShaped)
  ) {
    fail('publishable profile is incomplete or fixture-shaped');
  }

  const root = repositoryRoot(profilePath);
  assertApproved(root, profilePath, 'profile');
  const provisioning = containedPath(
    root,
    raw.provisioningPath,
    'profile.provisioningPath',
    'file'
  );
  const assetRoot = containedPath(
    root,
    assets.root,
    'profile.assets.root',
    'directory',
    true
  );
  const assetsByTarget = assetInventory(root, assetRoot, assets.iconBase, assets.requiredTargets);
  const provisioningJson = readJson(provisioning, 'provisioning document');
  if (
    provisioningJson.identity?.id !== product.id ||
    provisioningJson.identity?.displayName !== product.displayName ||
    provisioningJson.identity?.version !== product.version
  ) {
    fail('profile and provisioning identity mismatch');
  }

  return { root, provisioning, assetRoot, assetsByTarget };
}

function resolveProfile(profileFile) {
  let profilePath;
  try {
    profilePath = fs.realpathSync(profileFile);
  } catch {
    fail('profile file does not exist');
  }
  const raw = readJson(profilePath, 'profile');
  const paths = validateProfile(raw, profilePath);
  const normalized = structuredClone(raw);
  normalized.compatibility.requiredMethods.sort();
  normalized.assets.requiredTargets.sort();
  const profileJson = canonicalJson(normalized);
  const profileHash = crypto.createHash('sha256').update(profileJson).digest('hex');
  const head = currentRevision(paths.root);
  const requested = normalized.compatibility.goslingRevision;
  if (requested !== 'current' && requested !== head) {
    fail('profile.compatibility.goslingRevision does not match checkout HEAD');
  }
  return {
    profile: normalized,
    profileJson,
    profileHash,
    resolvedGoslingRevision: head,
    sourceClean: checkoutIsClean(paths.root),
    profilePath,
    provisioningPath: paths.provisioning,
    assetRoot: paths.assetRoot,
    assetsByTarget: paths.assetsByTarget,
    repositoryRoot: paths.root,
  };
}

const COLLISION_FIELDS = [
  ['product.id', (profile) => profile.product.id],
  ['product.runtimeNamespace', (profile) => profile.product.runtimeNamespace],
  ['product.protocolScheme', (profile) => profile.product.protocolScheme],
  ['product.executableName', (profile) => profile.product.executableName],
  ['product.macosBundleId', (profile) => profile.product.macosBundleId.toLowerCase()],
  ['product.windowsAppId', (profile) => profile.product.windowsAppId.toLowerCase()],
  ['product.linuxPackageName', (profile) => profile.product.linuxPackageName],
  ['product.flatpakId', (profile) => profile.product.flatpakId.toLowerCase()],
  ['update.channel', (profile) => profile.update.channel],
  ['distribution.artifactPrefix', (profile) => profile.distribution.artifactPrefix],
];

function resolveProfiles(files) {
  const resolved = files.map(resolveProfile);
  const duplicate = resolved.find(
    (entry, index) => resolved.findIndex((candidate) => candidate.profilePath === entry.profilePath) !== index
  );
  if (duplicate) fail('profile file was provided more than once');
  resolved.sort((a, b) => a.profilePath.localeCompare(b.profilePath));
  for (const [field, valueFor] of COLLISION_FIELDS) {
    const seen = new Map();
    for (const entry of resolved) {
      const value = valueFor(entry.profile);
      const displayPath = path.relative(entry.repositoryRoot, entry.profilePath);
      if (seen.has(value)) {
        fail(`profile identity collision at ${field} between ${seen.get(value)} and ${displayPath}`);
      }
      seen.set(value, displayPath);
    }
  }
  return resolved;
}

function discoverProfiles(root) {
  const files = [];
  for (const approved of approvedRoots(root)) {
    const pending = [approved];
    while (pending.length > 0) {
      const directory = pending.pop();
      for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
        const candidate = path.join(directory, entry.name);
        if (entry.isSymbolicLink()) continue;
        if (entry.isDirectory()) pending.push(candidate);
        if (entry.isFile() && entry.name === PROFILE_FILE_NAME) files.push(candidate);
      }
    }
  }
  return files.sort();
}

function targetParts(target) {
  if (!TARGETS.has(target)) fail('build target is unsupported');
  const [platform, architecture] = target.split('-');
  return { platform, architecture };
}

function buildManifest(resolved, target) {
  const { platform, architecture } = targetParts(target);
  if (!resolved.profile.assets.requiredTargets.includes(target)) {
    fail(`profile.assets.requiredTargets does not include ${target}`);
  }
  if (resolved.profile.distribution.publishable && !resolved.sourceClean) {
    fail('publishable profile manifest requires a clean checkout');
  }
  const manifest = {
    schemaVersion: 1,
    profileSchemaVersion: resolved.profile.schemaVersion,
    profileHash: resolved.profileHash,
    product: resolved.profile.product,
    target,
    platform,
    architecture,
    sourceClean: resolved.sourceClean,
    compatibility: {
      goslingVersion: resolved.profile.compatibility.goslingVersion,
      goslingRevision: resolved.resolvedGoslingRevision,
      provisioningSchemaVersion: resolved.profile.compatibility.provisioningSchemaVersion,
      handoffSchemaVersion: resolved.profile.compatibility.handoffSchemaVersion,
      requiredMethods: resolved.profile.compatibility.requiredMethods,
    },
  };
  return { manifest, manifestJson: canonicalJson(manifest) };
}

function writeAtomic(file, contents) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.${crypto.randomBytes(8).toString('hex')}.tmp`;
  fs.writeFileSync(temporary, contents, { encoding: 'utf8', mode: 0o644 });
  fs.renameSync(temporary, file);
}

function writeBuildResolution(resolved, target, outputDirectory) {
  const buildRoot = path.join(resolved.repositoryRoot, 'build');
  const output = outputDirectory
    ? path.resolve(resolved.repositoryRoot, outputDirectory)
    : path.join(buildRoot, 'shell-profiles', resolved.profile.product.id, target);
  if (!isContained(buildRoot, output)) fail('build output must remain under the repository build directory');
  const { manifest, manifestJson } = buildManifest(resolved, target);
  const profileOutput = path.join(output, 'profile.json');
  const manifestOutput = path.join(output, 'manifest.json');
  writeAtomic(profileOutput, resolved.profileJson);
  writeAtomic(manifestOutput, manifestJson);
  return { manifest, manifestJson, outputDirectory: output, profileOutput, manifestOutput };
}

module.exports = {
  APPROVED_PROFILE_ROOTS,
  COLLISION_FIELDS,
  PROFILE_FILE_NAME,
  buildManifest,
  canonicalJson,
  discoverProfiles,
  parseJsonWithoutDuplicateKeys,
  resolveProfile,
  resolveProfiles,
  validateProfile,
  writeBuildResolution,
};
