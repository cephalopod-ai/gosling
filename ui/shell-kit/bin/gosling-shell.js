#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');
const { resolveConsumerManifest } = require('../src/shell-consumer');
const { writeBuildResolution } = require('../src/shell-profile');
const metadata = require('../package.json');

const LOWER_KEBAB = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$/;
const REVERSE_DNS = /^[A-Za-z0-9]+(?:[._-][A-Za-z0-9]+){2,}$/;
const METHODS = [
  '_gosling/unstable/session/info',
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
];
const CAPABILITIES = [
  'directory.select',
  'elicitation.respond',
  'permission.respond',
  'prompt.cancel',
  'prompt.submit',
  'session.create',
  'session.detach',
  'session.list',
  'session.resume',
  'session.transcript.read',
];
const PNG =
  'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAAL0lEQVR42u3OIQEAAAgDMLoQjfiEgBg3E/Ornr2kEhAQEBAQEBAQEBAQEBAQSAcecybAiJxVvEkAAAAASUVORK5CYII=';
const ICO =
  'AAABAAEAICAAAAEAIABoAAAAFgAAAIlQTkcNChoKAAAADUlIRFIAAAAgAAAAIAgGAAAAc3p69AAAAC9JREFUeNrtziEBAAAIAzC6EI34hIAYNxPzq569pBIQEBAQEBAQEBAQEBAQEEgHHnMmwIicVbxJAAAAAElFTkSuQmCC';
const ICNS =
  'aWNucwAAAHhpY3A1AAAAcIlQTkcNChoKAAAADUlIRFIAAAAgAAAAIAgGAAAAc3p69AAAAC9JREFUeNrtziEBAAAIAzC6EI34hIAYNxPzq569pBIQEBAQEBAQEBAQEBAQEEgHHnMmwIicVbxJAAAAAElFTkSuQmCC';

function fail(message) {
  throw new Error(message);
}

function option(args, name) {
  const index = args.indexOf(name);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (!value || value.startsWith('--')) fail(`${name} requires a value`);
  return value;
}

function identifier(value, label) {
  if (
    typeof value !== 'string' ||
    value.length < 3 ||
    value.length > 48 ||
    !LOWER_KEBAB.test(value)
  ) {
    fail(`${label} is invalid`);
  }
  return value;
}

function displayName(value) {
  if (typeof value !== 'string' || !value.trim() || value.trim() !== value || value.length > 64) {
    fail('display name is invalid');
  }
  return value;
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, {
    mode: 0o644,
  });
}

function assertRegisteredPackage(root) {
  const packageFile = path.join(root, 'package.json');
  if (!fs.existsSync(packageFile)) fail('init requires an existing consumer package.json');
  const consumerPackage = JSON.parse(fs.readFileSync(packageFile, 'utf8'));
  if (consumerPackage.dependencies?.[metadata.name] !== metadata.version) {
    fail(`consumer package must pin ${metadata.name} exactly to ${metadata.version}`);
  }
}

function initialize(args) {
  const root = fs.realpathSync(option(args, '--root') ?? process.cwd());
  assertRegisteredPackage(root);
  const id = identifier(option(args, '--id'), 'id');
  const name = displayName(option(args, '--display-name'));
  const reverseDns = option(args, '--reverse-dns') ?? `org.example.${id.replaceAll('-', '')}`;
  if (!REVERSE_DNS.test(reverseDns)) fail('reverse DNS identifier is invalid');
  const destinations = ['shell-consumer.json', 'renderer.js', 'product'];
  if (destinations.some((entry) => fs.existsSync(path.join(root, entry)))) {
    fail('init never overwrites an existing consumer, renderer, or product directory');
  }

  const productRoot = path.join(root, 'product');
  const assetsRoot = path.join(productRoot, 'assets');
  fs.mkdirSync(assetsRoot, { recursive: true, mode: 0o755 });
  const product = {
    id,
    displayName: name,
    version: '0.1.0',
    runtimeNamespace: id,
    protocolScheme: id,
    executableName: id,
    macosBundleId: reverseDns,
    windowsAppId: reverseDns,
    linuxPackageName: id,
    flatpakId: reverseDns,
  };
  writeJson(path.join(root, 'shell-consumer.json'), {
    schemaVersion: 1,
    consumerId: id,
    requiredShellKit: metadata.version,
    productProfilePath: 'product/product-profile.json',
    rendererEntry: 'renderer.js',
    declaredCapabilities: CAPABILITIES,
  });
  writeJson(path.join(productRoot, 'product-profile.json'), {
    schemaVersion: 1,
    product,
    provisioningPath: 'product/provisioning.json',
    compatibility: {
      goslingVersion: metadata.version,
      goslingRevision: metadata.goslingRevision,
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: METHODS,
    },
    assets: {
      root: 'product/assets',
      iconBase: 'product/assets/icon',
      requiredTargets: ['linux-x64', 'macos-arm64', 'macos-x64', 'windows-x64'],
    },
    update: { enabled: false, channel: `${id}-disabled` },
    distribution: {
      publishable: false,
      artifactPrefix: id,
      signingPolicy: 'none',
    },
  });
  writeJson(path.join(productRoot, 'provisioning.json'), {
    schemaVersion: 1,
    identity: {
      id,
      displayName: name,
      version: product.version,
      runtimeNamespace: id,
    },
    settingsAuthority: 'main_gosling',
    settingsSchemaVersion: 1,
    protocolPolicy: {
      mode: 'restricted',
      deniedMethods: ['_gosling/unstable/config/upsert'],
    },
    session: { credentialPolicy: 'fixed', extensions: [], skillIds: [] },
    instructions: {
      systemPrompt: `You are the ${name} assistant. Work only inside the operator-selected folder.`,
    },
  });
  fs.writeFileSync(
    path.join(root, 'renderer.js'),
    `const root = document.querySelector('#root');\nif (root) root.textContent = ${JSON.stringify(`${name} shell`)};\n`,
    { mode: 0o644 }
  );
  fs.writeFileSync(path.join(assetsRoot, 'icon.png'), Buffer.from(PNG, 'base64'));
  fs.writeFileSync(path.join(assetsRoot, 'icon.ico'), Buffer.from(ICO, 'base64'));
  fs.writeFileSync(path.join(assetsRoot, 'icon.icns'), Buffer.from(ICNS, 'base64'));
  fs.writeFileSync(
    path.join(assetsRoot, 'icon.svg'),
    '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" fill="#4a5568"/></svg>\n'
  );
  return check(path.join(root, 'shell-consumer.json'));
}

function check(manifestFile) {
  const resolved = resolveConsumerManifest(manifestFile);
  const provisioning = JSON.parse(fs.readFileSync(resolved.profile.provisioningPath, 'utf8'));
  const findings = [];
  if (!provisioning.instructions?.systemPrompt?.trim()) {
    findings.push('provisioning must declare instructions.systemPrompt');
  }
  if (!['fixed', 'selectable_catalog'].includes(provisioning.session?.credentialPolicy)) {
    findings.push('provisioning must declare session.credentialPolicy explicitly');
  }
  if (provisioning.settingsSchemaVersion !== 1) {
    findings.push('provisioning must declare settingsSchemaVersion 1');
  }
  if (resolved.profile.profile.update.enabled) findings.push('updates must remain disabled');
  if (resolved.profile.profile.distribution.publishable)
    findings.push('publishing must remain disabled');
  if (resolved.profile.profile.distribution.signingPolicy !== 'none') {
    findings.push('signing must remain disabled');
  }
  return {
    conformant: findings.length === 0,
    findings,
    consumerId: resolved.consumer.consumerId,
    productId: resolved.profile.profile.product.id,
    shellKitVersion: metadata.version,
    consumerHash: resolved.consumerHash,
    rendererHash: resolved.rendererHash,
    profileHash: resolved.profile.profileHash,
    requiredMethods: resolved.requiredMethods,
    requiredAgentCapabilities: resolved.requiredAgentCapabilities,
  };
}

function resolve(args) {
  const manifest = option(args, '--manifest');
  const target = option(args, '--target');
  const output = option(args, '--output');
  if (!manifest || !target) fail('resolve requires --manifest and --target');
  const consumer = resolveConsumerManifest(path.resolve(manifest));
  return writeBuildResolution(consumer.profile, target, output, consumer);
}

function usage() {
  return [
    'Usage:',
    '  gosling-shell init --id <lower-kebab> --display-name <name> [--reverse-dns <id>]',
    '  gosling-shell check <shell-consumer.json>',
    '  gosling-shell resolve --manifest <shell-consumer.json> --target <target> [--output <path>]',
  ].join('\n');
}

const [command, ...args] = process.argv.slice(2);
try {
  let result;
  if (command === 'init') result = initialize(args);
  else if (command === 'check' && args.length === 1) result = check(path.resolve(args[0]));
  else if (command === 'resolve') result = resolve(args);
  else fail(usage());
  console.log(JSON.stringify(result, null, 2));
  if (result.conformant === false) process.exitCode = 1;
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
