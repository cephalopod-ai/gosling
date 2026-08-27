const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const {
  APPROVED_PROFILE_ROOTS,
  COLLISION_FIELDS,
  discoverProfiles,
  parseJsonWithoutDuplicateKeys,
} = require('./shell-profile');
const { APPROVED_CONSUMER_ROOTS, resolveConsumerManifest } = require('./shell-consumer');

const LOWER_KEBAB = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/;
const SECRET_PATTERN =
  /(api.?key|authorization|bearer|cookie|credential.?value|password|private.?key|secret|token)/i;
const NAMED_DOMAIN_PATTERN = /(dawes|physics|cst|chemistry|biology|mathematics)/i;
const REQUIRED_TARGETS = ['linux-x64', 'macos-arm64', 'macos-x64', 'windows-x64'];
const ICON_EXTENSIONS = ['.icns', '.ico', '.png', '.svg'];
const DECLARED_CAPABILITIES = [
  'directory.select',
  'elicitation.respond',
  'permission.respond',
  'prompt.cancel',
  'prompt.submit',
  'session.artifacts.read',
  'session.create',
  'session.detach',
  'session.extensions.read',
  'session.extensions.write',
  'session.library.read',
  'session.library.write',
  'session.list',
  'session.resume',
  'session.transcript.read',
];
const REQUIRED_METHODS = [
  '_gosling/unstable/session/info',
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
];

function fail(message) {
  throw new Error(message);
}

function repositoryRoot() {
  return fs.realpathSync(path.resolve(__dirname, '..', '..', '..'));
}

function isContained(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function safeRelative(candidate, label) {
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
  return candidate;
}

/// Rejects a destination whose *real* location is not inside an approved root.
///
/// Checking only the immediate parent is not enough: a directory symlink at any component below an
/// approved root keeps `path.resolve` looking contained while `mkdirSync` would follow the link out
/// of the repository. Every component is walked, and the nearest existing ancestor is compared by
/// its real path.
function approvedDestination(root, candidate, approvedRoots, label) {
  const relative = safeRelative(candidate, label);
  const resolved = path.resolve(root, relative);
  if (!isContained(root, resolved)) fail(`${label} escapes the repository`);
  const approved = approvedRoots
    .map((entry) => path.resolve(root, entry))
    .filter((entry) => isContained(entry, resolved));
  if (approved.length === 0) {
    fail(`${label} is outside the approved roots (${approvedRoots.join(', ')})`);
  }
  for (const component of walkAncestors(root, resolved)) {
    if (fs.lstatSync(component).isSymbolicLink()) {
      fail(`${label} resolves through a symlink`);
    }
  }
  const existingAncestor = nearestExistingAncestor(resolved);
  const realAncestor = fs.realpathSync(existingAncestor);
  if (
    !approved.some((entry) =>
      isContained(fs.existsSync(entry) ? fs.realpathSync(entry) : entry, realAncestor)
    )
  ) {
    fail(`${label} resolves outside the approved roots`);
  }
  if (fs.existsSync(resolved) || fs.existsSync(`${resolved}.tmp-scaffold`)) {
    fail(`${label} already exists; the scaffold never merges into or overwrites existing work`);
  }
  return { relative, resolved };
}

function walkAncestors(root, resolved) {
  const components = [];
  let current = resolved;
  while (current !== root && isContained(root, current) && current !== path.dirname(current)) {
    if (fs.existsSync(current) || isSymbolicLink(current)) components.push(current);
    current = path.dirname(current);
  }
  return components.reverse();
}

function isSymbolicLink(candidate) {
  try {
    return fs.lstatSync(candidate).isSymbolicLink();
  } catch {
    return false;
  }
}

function nearestExistingAncestor(resolved) {
  let current = resolved;
  while (!fs.existsSync(current) && current !== path.dirname(current)) {
    current = path.dirname(current);
  }
  return current;
}

function identifier(value, label, pattern, minimum, maximum) {
  if (
    typeof value !== 'string' ||
    value.length < minimum ||
    value.length > maximum ||
    !pattern.test(value)
  ) {
    fail(`${label} is invalid`);
  }
  if (SECRET_PATTERN.test(value)) fail(`${label} contains secret-shaped content`);
  if (NAMED_DOMAIN_PATTERN.test(value)) fail(`${label} contains named-domain content`);
  return value;
}

function displayName(value, label) {
  if (typeof value !== 'string' || value.trim() !== value || !value || value.length > 64) {
    fail(`${label} is invalid`);
  }
  if (SECRET_PATTERN.test(value)) fail(`${label} contains secret-shaped content`);
  if (NAMED_DOMAIN_PATTERN.test(value)) fail(`${label} contains named-domain content`);
  return value;
}

function productProfile(input) {
  return {
    schemaVersion: 1,
    product: {
      id: input.productId,
      displayName: input.displayName,
      version: '0.0.0-template',
      runtimeNamespace: input.runtimeNamespace,
      protocolScheme: input.protocolScheme,
      executableName: input.productId,
      macosBundleId: input.macosBundleId,
      windowsAppId: input.windowsAppId,
      linuxPackageName: input.productId,
      flatpakId: input.flatpakId,
    },
    provisioningPath: `${input.productRelative}/provisioning.json`,
    compatibility: {
      goslingVersion: '0.1.0',
      goslingRevision: 'current',
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: [...REQUIRED_METHODS],
    },
    assets: {
      root: `${input.productRelative}/assets`,
      iconBase: `${input.productRelative}/assets/icon`,
      requiredTargets: [...REQUIRED_TARGETS],
    },
    update: { enabled: false, channel: `${input.productId}-disabled` },
    distribution: { publishable: false, artifactPrefix: input.productId, signingPolicy: 'none' },
  };
}

function provisioning(input) {
  return {
    schemaVersion: 1,
    identity: {
      id: input.productId,
      displayName: input.displayName,
      version: '0.0.0-template',
      runtimeNamespace: input.runtimeNamespace,
    },
    settingsAuthority: 'main_gosling',
    settingsSchemaVersion: 1,
    protocolPolicy: { mode: 'restricted', deniedMethods: ['_gosling/unstable/config/upsert'] },
    session: { credentialPolicy: 'fixed', extensions: [], skillIds: [] },
    instructions: {
      systemPrompt: `You are the ${input.displayName} assistant. Work only inside the folder the operator selected, explain what you are about to change before changing it, and ask for confirmation when an action is not reversible.`,
    },
  };
}

function consumerManifest(input) {
  return {
    schemaVersion: 1,
    consumerId: input.consumerId,
    requiredShellKit: 'current',
    productProfilePath: `${input.productRelative}/product-profile.json`,
    rendererEntry: 'renderer.ts',
    declaredCapabilities: [...DECLARED_CAPABILITIES],
  };
}

function rendererEntry(input) {
  return `const root = document.querySelector<HTMLElement>('#root');

if (root) {
  root.textContent = ${JSON.stringify(`${input.displayName} conformance surface`)};
}
`;
}

function readme(input) {
  return `# ${input.displayName}

Development template generated by \`pnpm run shell:scaffold\`. It is not a production product and
not a named-domain shell.

Before this template can be built or packaged, an operator must supply:

${ICON_EXTENSIONS.map((extension) => `- \`assets/icon${extension}\``).join('\n')}

Everything else — product profile, provisioning, consumer manifest, instruction profile, credential
policy, settings schema version, and the conformance renderer entry — is already generated.

Run \`pnpm run shell:conformance ${input.consumerRelative}/shell-consumer.json\` after adding the
icons.
`;
}

/// Rejects an identity that already belongs to another source-controlled product profile.
///
/// This reuses the resolver's own `COLLISION_FIELDS` rather than a second hand-maintained list, and
/// runs before anything is written, so the scaffold cannot report success and leave a repository
/// that `shell:check-profiles` then rejects.
function assertNoIdentityCollision(root, candidate) {
  // Existing profiles are compared as raw documents rather than resolved ones: a template whose
  // operator has not supplied icons yet cannot resolve, and that incompleteness is not a reason to
  // stop checking the identity it already claims.
  const existing = discoverProfiles(root).flatMap((file) => {
    try {
      return [parseJsonWithoutDuplicateKeys(fs.readFileSync(file, 'utf8'), 'product profile')];
    } catch {
      return [];
    }
  });
  for (const [field, valueFor] of COLLISION_FIELDS) {
    const value = valueFor(candidate);
    const claimed = existing.some((profile) => {
      try {
        return valueFor(profile) === value;
      } catch {
        return false;
      }
    });
    if (claimed) fail(`${field} collides with an existing shell product profile`);
  }
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o644 });
}

/// Generates one neutral Default Shell template into a fresh, approved, non-existent destination.
///
/// Everything is staged in a sibling temporary directory and validated through the existing strict
/// resolvers before it is renamed into place, so a failed generation never leaves a partial product.
function scaffoldShell(input) {
  const root = input.repositoryRoot ?? repositoryRoot();
  const productId = identifier(input.productId, 'productId', LOWER_KEBAB, 3, 48);
  const consumerId = identifier(input.consumerId ?? productId, 'consumerId', LOWER_KEBAB, 3, 48);
  const runtimeNamespace = identifier(
    input.runtimeNamespace,
    'runtimeNamespace',
    LOWER_KEBAB,
    3,
    48
  );
  const protocolScheme = identifier(input.protocolScheme, 'protocolScheme', LOWER_KEBAB, 3, 48);
  const name = displayName(input.displayName, 'displayName');

  const product = approvedDestination(
    root,
    input.productDestination,
    APPROVED_PROFILE_ROOTS,
    'productDestination'
  );
  const consumer = approvedDestination(
    root,
    input.consumerDestination,
    APPROVED_CONSUMER_ROOTS,
    'consumerDestination'
  );

  const context = {
    productId,
    consumerId,
    runtimeNamespace,
    protocolScheme,
    displayName: name,
    productRelative: product.relative,
    consumerRelative: consumer.relative,
    macosBundleId: identifier(
      input.macosBundleId,
      'macosBundleId',
      /^[a-z0-9]+(?:[.-][a-z0-9]+)+$/,
      3,
      128
    ),
    windowsAppId: identifier(
      input.windowsAppId,
      'windowsAppId',
      /^[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)+$/,
      3,
      128
    ),
    flatpakId: identifier(
      input.flatpakId,
      'flatpakId',
      /^[A-Za-z0-9_]+(?:\.[A-Za-z0-9_]+)+$/,
      3,
      128
    ),
  };

  assertNoIdentityCollision(root, productProfile(context));

  const productStaging = `${product.resolved}.tmp-scaffold`;
  const consumerStaging = `${consumer.resolved}.tmp-scaffold`;
  const created = [];
  const finalized = [];
  try {
    fs.mkdirSync(path.join(productStaging, 'assets'), { recursive: true, mode: 0o755 });
    fs.mkdirSync(consumerStaging, { recursive: true, mode: 0o755 });

    writeJson(path.join(productStaging, 'product-profile.json'), productProfile(context));
    writeJson(path.join(productStaging, 'provisioning.json'), provisioning(context));
    writeJson(path.join(consumerStaging, 'shell-consumer.json'), consumerManifest(context));
    fs.writeFileSync(path.join(consumerStaging, 'renderer.ts'), rendererEntry(context), {
      mode: 0o644,
    });
    fs.writeFileSync(path.join(consumerStaging, 'README.md'), readme(context), { mode: 0o644 });

    created.push(
      `${product.relative}/product-profile.json`,
      `${product.relative}/provisioning.json`,
      `${consumer.relative}/shell-consumer.json`,
      `${consumer.relative}/renderer.ts`,
      `${consumer.relative}/README.md`
    );

    fs.renameSync(productStaging, product.resolved);
    finalized.push(product.resolved);
    fs.renameSync(consumerStaging, consumer.resolved);
    finalized.push(consumer.resolved);
  } catch (error) {
    // A failure between the two renames would otherwise leave a finalized half behind and block a
    // clean retry. Only directories this call created are removed, and only after the destination
    // was proven not to exist beforehand.
    for (const directory of finalized) {
      fs.rmSync(directory, { recursive: true, force: true });
    }
    fs.rmSync(productStaging, { recursive: true, force: true });
    fs.rmSync(consumerStaging, { recursive: true, force: true });
    throw error;
  }

  return {
    productId,
    consumerId,
    productDirectory: product.relative,
    consumerDirectory: consumer.relative,
    consumerManifestPath: `${consumer.relative}/shell-consumer.json`,
    created: created.sort(),
    pendingOperatorInputs: ICON_EXTENSIONS.map(
      (extension) => `${product.relative}/assets/icon${extension}`
    ),
  };
}

const CONFORMANCE_CHECKS = [
  'consumer manifest resolves through the strict resolver',
  'product profile resolves through the strict resolver',
  'provisioning declares an instruction profile',
  'provisioning declares an explicit credential policy',
  'provisioning declares a settings schema version',
  'provisioning enables no developer builtin',
  'provisioning declares no domain adapter without a consumer declaration',
  'declared capabilities derive their required methods',
  'update, publishing, and signing stay disabled',
  'renderer entry exists and is consumer owned',
  'target icons exist for every required target',
];

/// One command that refuses to certify an incomplete template.
///
/// It reuses the strict profile/consumer resolvers rather than re-implementing their rules, then
/// adds the Default Shell obligations those resolvers do not own.
function checkShellConformance(manifestFile) {
  const findings = [];
  const resolved = resolveConsumerManifest(manifestFile);
  const profile = resolved.profile.profile;
  const provisioningFile = path.resolve(resolved.repositoryRoot, profile.provisioningPath);
  const provisioningDocument = JSON.parse(fs.readFileSync(provisioningFile, 'utf8'));

  const require = (condition, message) => {
    if (!condition) findings.push(message);
  };

  require(typeof provisioningDocument.instructions?.systemPrompt === 'string' &&
    provisioningDocument.instructions.systemPrompt.trim().length >
      0, 'provisioning must declare instructions.systemPrompt');
  require(provisioningDocument.session?.credentialPolicy === 'fixed' ||
    provisioningDocument.session?.credentialPolicy ===
      'selectable_catalog', 'provisioning must declare session.credentialPolicy explicitly');
  require(provisioningDocument.settingsSchemaVersion ===
    1, 'provisioning must declare settingsSchemaVersion 1');
  require(!(provisioningDocument.session?.extensions ?? []).some(
    (extension) => extension.name === 'developer'
  ), 'provisioning must not enable the developer builtin');
  require(provisioningDocument.domainAdapter === undefined ||
    resolved.consumer.domainAdapter !==
      undefined, 'provisioning declares a domain adapter the consumer does not declare');
  require(profile.update.enabled === false, 'profile.update.enabled must be false for a template');
  require(profile.distribution.publishable ===
    false, 'profile.distribution.publishable must be false for a template');
  require(profile.distribution.signingPolicy ===
    'none', 'profile.distribution.signingPolicy must be none for a template');
  require(NAMED_DOMAIN_PATTERN.test(JSON.stringify(provisioningDocument)) ===
    false, 'provisioning contains named-domain content');

  for (const target of profile.assets.requiredTargets) {
    const extensions = target.startsWith('macos-')
      ? ['.icns']
      : target === 'windows-x64'
        ? ['.ico']
        : ['.png', '.svg'];
    for (const extension of extensions) {
      const icon = path.resolve(resolved.repositoryRoot, `${profile.assets.iconBase}${extension}`);
      require(fs.existsSync(icon), `missing target asset ${profile.assets.iconBase}${extension}`);
    }
  }

  return {
    consumerId: resolved.consumer.consumerId,
    productId: profile.product.id,
    consumerHash: resolved.consumerHash,
    rendererHash: resolved.rendererHash,
    profileHash: resolved.profile.profileHash,
    requiredMethods: resolved.requiredMethods,
    requiredAgentCapabilities: resolved.requiredAgentCapabilities,
    declaredCapabilities: resolved.consumer.declaredCapabilities,
    checks: CONFORMANCE_CHECKS,
    findings,
    conformant: findings.length === 0,
  };
}

module.exports = {
  DECLARED_CAPABILITIES,
  ICON_EXTENSIONS,
  REQUIRED_TARGETS,
  checkShellConformance,
  scaffoldShell,
};

function optionValue(args, name) {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith('--')) fail(`${name} requires a value`);
  return value;
}

function usage() {
  return [
    'Usage:',
    '  node scripts/shell-scaffold.js create \\',
    '    --product-destination fixtures/shell-products/<id> \\',
    '    --consumer-destination fixtures/shell-consumers/<id> \\',
    '    --product-id <lower-kebab> --consumer-id <lower-kebab> \\',
    '    --display-name "<Name>" --runtime-namespace <lower-kebab> \\',
    '    --protocol-scheme <lower-kebab> --macos-bundle-id <a.b.c> \\',
    '    --windows-app-id <A.B.C> --flatpak-id <a.b.C>',
    '  node scripts/shell-scaffold.js conformance <consumer-manifest>',
  ].join('\n');
}

if (require.main === module) {
  const [command, ...args] = process.argv.slice(2);
  try {
    if (command === 'create') {
      const result = scaffoldShell({
        productDestination: optionValue(args, '--product-destination'),
        consumerDestination: optionValue(args, '--consumer-destination'),
        productId: optionValue(args, '--product-id'),
        consumerId: optionValue(args, '--consumer-id'),
        displayName: optionValue(args, '--display-name'),
        runtimeNamespace: optionValue(args, '--runtime-namespace'),
        protocolScheme: optionValue(args, '--protocol-scheme'),
        macosBundleId: optionValue(args, '--macos-bundle-id'),
        windowsAppId: optionValue(args, '--windows-app-id'),
        flatpakId: optionValue(args, '--flatpak-id'),
      });
      console.log(JSON.stringify(result, null, 2));
    } else if (command === 'conformance') {
      const [manifest] = args;
      if (!manifest) fail(usage());
      const report = checkShellConformance(path.resolve(process.cwd(), manifest));
      console.log(JSON.stringify(report, null, 2));
      if (!report.conformant) process.exitCode = 1;
    } else {
      fail(usage());
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
