const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { canonicalJson, parseJsonWithoutDuplicateKeys, resolveProfile } = require('./shell-profile');

const APPROVED_CONSUMER_ROOTS = ['fixtures/shell-consumers'];
const CONSUMER_MANIFEST_FILE_NAME = 'shell-consumer.json';
const LOWER_KEBAB = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/;
const SEMVER =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
const SECRET_PATTERN =
  /(api.?key|authorization|bearer|cookie|credential.?value|password|private.?key|secret|token)/i;
const PRIVATE_KEY_PATTERN = /-----BEGIN [^-]*PRIVATE KEY-----/i;
const APPLICATION_OPERATIONS = new Set([
  'credential.select',
  'directory.select',
  'session.create',
  'session.artifacts.read',
  'session.detach',
  'session.list',
  'session.resume',
  'session.transcript.read',
  'prompt.submit',
  'prompt.cancel',
  'permission.respond',
  'elicitation.respond',
  'domain.snapshot',
  'domain.action',
  'confirmation.respond',
]);
const CUSTOM_METHODS_BY_OPERATION = {
  'directory.select': '_gosling/unstable/shell/directory/validate',
  'credential.select': '_gosling/unstable/shell/credentials/list',
  'domain.snapshot': '_gosling/unstable/shell/domain/snapshot',
  'domain.action': '_gosling/unstable/shell/domain/action',
  'confirmation.respond': '_gosling/unstable/shell/domain/action/confirm',
  'session.artifacts.read': '_gosling/unstable/shell/session/artifacts/list',
};
const AGENT_CAPABILITIES_BY_OPERATION = {
  'session.create': 'loadSession',
  'session.list': 'sessionList',
  'session.resume': 'loadSession',
};
const OPERATION_PREREQUISITES = {
  'domain.action': ['confirmation.respond'],
  'prompt.cancel': ['prompt.submit'],
  'prompt.submit': ['elicitation.respond', 'permission.respond', 'session.create'],
  'session.create': ['directory.select'],
  'session.artifacts.read': ['session.create'],
  'session.detach': ['session.create'],
  'session.list': ['directory.select'],
  'session.resume': ['session.list'],
  'session.transcript.read': ['session.create'],
  'confirmation.respond': ['domain.action'],
};

function fail(message) {
  throw new Error(message);
}

function object(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    fail(`${label} must be an object`);
  return value;
}

function exactKeys(value, required, optional, label) {
  object(value, label);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    if (SECRET_PATTERN.test(key)) fail(`${label} contains a secret-shaped field`);
    if (!allowed.has(key)) fail(`${label}.${key} is unknown`);
  }
  for (const key of required) {
    if (!Object.hasOwn(value, key)) fail(`${label}.${key} is required`);
  }
}

function string(value, label, pattern, minimum, maximum) {
  if (
    typeof value !== 'string' ||
    value.trim() !== value ||
    value.length < minimum ||
    value.length > maximum ||
    !pattern.test(value)
  ) {
    fail(`${label} is invalid`);
  }
}

function assertSafeValues(value, label = 'consumer') {
  if (
    typeof value === 'string' &&
    (SECRET_PATTERN.test(value) || PRIVATE_KEY_PATTERN.test(value))
  ) {
    fail(`${label} contains secret-shaped content`);
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertSafeValues(entry, `${label}[${index}]`));
  } else if (value && typeof value === 'object') {
    for (const [key, entry] of Object.entries(value)) {
      if (SECRET_PATTERN.test(key)) fail(`${label} contains a secret-shaped field`);
      assertSafeValues(entry, `${label}.${key}`);
    }
  }
}

function isContained(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function repositoryRoot(manifestPath) {
  let current = path.dirname(manifestPath);
  while (current !== path.dirname(current)) {
    if (
      fs.existsSync(path.join(current, 'Cargo.toml')) &&
      fs.existsSync(path.join(current, 'ui', 'desktop'))
    ) {
      return fs.realpathSync(current);
    }
    current = path.dirname(current);
  }
  fail('consumer manifest is not inside a Gosling repository');
}

function approvedRoots(root) {
  return APPROVED_CONSUMER_ROOTS.map((entry) => path.join(root, entry)).filter((entry) =>
    fs.existsSync(entry)
  );
}

function assertApproved(root, candidate, label) {
  if (!approvedRoots(root).some((approved) => isContained(fs.realpathSync(approved), candidate))) {
    fail(`${label} is outside approved consumer roots`);
  }
}

function containedPath(root, consumerRoot, candidate, label, kind) {
  if (
    typeof candidate !== 'string' ||
    !candidate ||
    path.isAbsolute(candidate) ||
    candidate.includes('\0') ||
    candidate.includes('\\') ||
    candidate.split('/').includes('..')
  ) {
    fail(`${label} must be a safe consumer-relative path`);
  }
  const resolved = path.resolve(consumerRoot, candidate);
  if (!isContained(consumerRoot, resolved)) fail(`${label} escapes the consumer root`);
  let stat;
  let real;
  try {
    stat = fs.lstatSync(resolved);
    if (stat.isSymbolicLink()) fail(`${label} must not be a symlink`);
    real = fs.realpathSync(resolved);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith(label)) throw error;
    fail(`${label} does not exist`);
  }
  if (!isContained(consumerRoot, real)) fail(`${label} resolves outside the consumer root`);
  assertApproved(root, real, label);
  const realStat = fs.statSync(real);
  if ((kind === 'file' && !realStat.isFile()) || (kind === 'directory' && !realStat.isDirectory)) {
    fail(`${label} has the wrong type`);
  }
  return real;
}

function profilePath(root, consumerRoot, candidate) {
  if (
    typeof candidate !== 'string' ||
    !candidate ||
    path.isAbsolute(candidate) ||
    candidate.includes('\0') ||
    candidate.includes('\\') ||
    candidate.split('/').includes('..')
  ) {
    fail('consumer.productProfilePath must be a safe path');
  }
  const fromConsumer = path.resolve(consumerRoot, candidate);
  const fromRepository = path.resolve(root, candidate);
  const selected = fs.existsSync(fromConsumer) ? fromConsumer : fromRepository;
  if (!isContained(root, selected)) fail('consumer.productProfilePath escapes the repository');
  return selected;
}

function sortedUnique(values, label, allowed) {
  if (
    !Array.isArray(values) ||
    values.length === 0 ||
    values.some((value) => typeof value !== 'string') ||
    new Set(values).size !== values.length ||
    values.some((value) => !allowed.has(value)) ||
    values.some((value, index) => index > 0 && values[index - 1] >= value)
  ) {
    fail(`${label} is invalid`);
  }
}

function resolveConsumerManifest(manifestFile) {
  let manifestPath;
  try {
    if (fs.lstatSync(manifestFile).isSymbolicLink())
      fail('consumer manifest must not be a symlink');
    manifestPath = fs.realpathSync(manifestFile);
  } catch (error) {
    if (error instanceof Error && error.message.startsWith('consumer manifest')) throw error;
    fail('consumer manifest does not exist');
  }
  const root = repositoryRoot(manifestPath);
  assertApproved(root, manifestPath, 'consumer manifest');
  const consumerRoot = path.dirname(manifestPath);
  const raw = parseJsonWithoutDuplicateKeys(
    fs.readFileSync(manifestPath, 'utf8'),
    'consumer manifest'
  );
  exactKeys(
    raw,
    [
      'schemaVersion',
      'consumerId',
      'requiredShellKit',
      'productProfilePath',
      'rendererEntry',
      'declaredCapabilities',
    ],
    ['domainAdapter', 'assetsRoot', 'testFixturesRoot'],
    'consumer'
  );
  if (raw.schemaVersion !== 1) fail('consumer.schemaVersion must be 1');
  assertSafeValues(raw);
  string(raw.consumerId, 'consumer.consumerId', LOWER_KEBAB, 3, 48);
  if (raw.requiredShellKit !== 'current') {
    fail('consumer.requiredShellKit is invalid');
  }
  sortedUnique(raw.declaredCapabilities, 'consumer.declaredCapabilities', APPLICATION_OPERATIONS);
  const declaresDomainOperation = raw.declaredCapabilities.some((operation) =>
    ['domain.snapshot', 'domain.action', 'confirmation.respond'].includes(operation)
  );
  if (declaresDomainOperation && raw.domainAdapter === undefined) {
    fail('consumer.domainAdapter is required when domain operations are declared');
  }
  for (const [operation, prerequisites] of Object.entries(OPERATION_PREREQUISITES)) {
    if (!raw.declaredCapabilities.includes(operation)) continue;
    for (const prerequisite of prerequisites) {
      if (!raw.declaredCapabilities.includes(prerequisite)) {
        fail(`${operation} requires ${prerequisite}`);
      }
    }
  }
  if (raw.domainAdapter !== undefined) {
    exactKeys(
      raw.domainAdapter,
      ['descriptorId', 'protocolVersion', 'actions'],
      [],
      'consumer.domainAdapter'
    );
    string(
      raw.domainAdapter.descriptorId,
      'consumer.domainAdapter.descriptorId',
      LOWER_KEBAB,
      3,
      64
    );
    string(
      raw.domainAdapter.protocolVersion,
      'consumer.domainAdapter.protocolVersion',
      SEMVER,
      5,
      64
    );
    sortedUnique(
      raw.domainAdapter.actions,
      'consumer.domainAdapter.actions',
      new Set(raw.domainAdapter.actions)
    );
  }
  const rendererEntry = containedPath(
    root,
    consumerRoot,
    raw.rendererEntry,
    'consumer.rendererEntry',
    'file'
  );
  if (!/\.[cm]?[jt]sx?$/.test(rendererEntry))
    fail('consumer.rendererEntry has an unsupported extension');
  const assetsRoot =
    raw.assetsRoot === undefined
      ? undefined
      : containedPath(root, consumerRoot, raw.assetsRoot, 'consumer.assetsRoot', 'directory');
  const testFixturesRoot =
    raw.testFixturesRoot === undefined
      ? undefined
      : containedPath(
          root,
          consumerRoot,
          raw.testFixturesRoot,
          'consumer.testFixturesRoot',
          'directory'
        );
  const profile = resolveProfile(profilePath(root, consumerRoot, raw.productProfilePath));
  if (profile.repositoryRoot !== root)
    fail('consumer.productProfilePath belongs to another repository');
  const normalized = structuredClone(raw);
  const consumerJson = canonicalJson(normalized);
  const consumerHash = crypto.createHash('sha256').update(consumerJson).digest('hex');
  const rendererHash = crypto
    .createHash('sha256')
    .update(fs.readFileSync(rendererEntry))
    .digest('hex');
  const requiredMethods = new Set(profile.profile.compatibility.requiredMethods);
  const requiredAgentCapabilities = new Set();
  for (const operation of normalized.declaredCapabilities) {
    if (CUSTOM_METHODS_BY_OPERATION[operation])
      requiredMethods.add(CUSTOM_METHODS_BY_OPERATION[operation]);
    if (AGENT_CAPABILITIES_BY_OPERATION[operation]) {
      requiredAgentCapabilities.add(AGENT_CAPABILITIES_BY_OPERATION[operation]);
    }
  }
  return {
    consumer: normalized,
    consumerJson,
    consumerHash,
    rendererHash,
    manifestPath,
    consumerRoot,
    repositoryRoot: root,
    rendererEntry,
    assetsRoot,
    testFixturesRoot,
    profile,
    requiredMethods: [...requiredMethods].sort(),
    requiredAgentCapabilities: [...requiredAgentCapabilities].sort(),
  };
}

module.exports = {
  APPLICATION_OPERATIONS,
  APPROVED_CONSUMER_ROOTS,
  CONSUMER_MANIFEST_FILE_NAME,
  resolveConsumerManifest,
};
