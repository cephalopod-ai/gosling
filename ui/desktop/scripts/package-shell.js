#!/usr/bin/env node

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { expectedPackageDirectory, verifyShellPackage } = require('./shell-package-verifier');
const { resolveConsumerManifest } = require('./shell-consumer');
const { shellBinaryStagePath, targetFor } = require('./shell-forge-profile');

function usage() {
  return [
    'Usage:',
    '  node scripts/package-shell.js <profile> [--consumer <manifest>] [--platform <platform>] [--arch <architecture>]',
  ].join('\n');
}

function optionalOption(args, name, fallback) {
  const indexes = args.flatMap((entry, index) => (entry === name ? [index] : []));
  if (indexes.length === 0) return fallback;
  if (indexes.length > 1) throw new Error(`${name} may be provided only once`);
  const index = indexes[0];
  const value = args[index + 1];
  if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`);
  args.splice(index, 2);
  return value;
}

function rustTarget(platform, architecture) {
  const targets = {
    'darwin-arm64': 'aarch64-apple-darwin',
    'darwin-x64': 'x86_64-apple-darwin',
    'linux-x64': 'x86_64-unknown-linux-gnu',
    'win32-x64': 'x86_64-pc-windows-msvc',
  };
  const value = targets[`${platform}-${architecture}`];
  if (!value) throw new Error(`unsupported shell build target ${platform}/${architecture}`);
  return value;
}

function hostTarget(platform = process.platform, architecture = process.arch) {
  return rustTarget(platform, architecture);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: 'inherit', ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed with exit code ${result.status}`);
}

function packageShell(input, dependencies = {}) {
  const runCommand = dependencies.run ?? run;
  const currentHostTarget = dependencies.hostTarget ?? hostTarget;
  const copyBinary =
    dependencies.copyBinary ??
    ((source, destination, platform) => {
      fs.mkdirSync(path.dirname(destination), { recursive: true });
      fs.copyFileSync(source, destination);
      if (platform !== 'win32') fs.chmodSync(destination, 0o755);
    });
  const verify = dependencies.verify ?? verifyShellPackage;
  const desktopRoot = path.resolve(__dirname, '..');
  const repositoryRoot = path.resolve(desktopRoot, '..', '..');
  const profileFile = path.resolve(input.profileFile);
  if (!input.consumerFile) throw new Error('shell:package-local requires a consumer manifest');
  const consumer = resolveConsumerManifest(path.resolve(input.consumerFile));
  const resolved = consumer.profile;
  if (resolved.profilePath !== profileFile) {
    throw new Error('shell profile and consumer manifest select different product profiles');
  }
  const target = targetFor(input.platform, input.architecture);
  const cargoTarget = rustTarget(input.platform, input.architecture);
  if (cargoTarget !== currentHostTarget()) {
    throw new Error(
      'shell:package-local requires the selected platform and architecture to match this host'
    );
  }
  if (!resolved.profile.assets.requiredTargets.includes(target)) {
    throw new Error(`profile.assets.requiredTargets does not include ${target}`);
  }
  if (
    resolved.profile.distribution.publishable ||
    resolved.profile.distribution.signingPolicy !== 'none' ||
    resolved.profile.update.enabled
  ) {
    throw new Error('shell:package-local accepts only non-publishable unsigned profiles');
  }

  const binaryName = input.platform === 'win32' ? 'gosling.exe' : 'gosling';
  const builtBinary = path.join(repositoryRoot, 'target', cargoTarget, 'release', binaryName);
  const stagedBinary = shellBinaryStagePath(resolved, target, binaryName);
  const packageDirectory = expectedPackageDirectory(
    desktopRoot,
    resolved.profile.product.displayName,
    input.platform,
    input.architecture
  );

  const buildCommand =
    input.platform === 'win32'
      ? [
          'cargo',
          ['build', '--release', '--target', cargoTarget, '-p', 'gosling-cli', '--bin', 'gosling'],
        ]
      : [
          path.join(repositoryRoot, 'scripts', 'with-rusty-v8-cache.sh'),
          [
            'cargo',
            'build',
            '--release',
            '--target',
            cargoTarget,
            '-p',
            'gosling-cli',
            '--bin',
            'gosling',
          ],
        ];
  runCommand(buildCommand[0], buildCommand[1], { cwd: repositoryRoot });
  copyBinary(builtBinary, stagedBinary, input.platform);
  runCommand('pnpm', ['run', 'build-gosling-sdk'], { cwd: desktopRoot });
  runCommand(
    'pnpm',
    [
      'exec',
      'electron-forge',
      'package',
      '--platform',
      input.platform,
      '--arch',
      input.architecture,
    ],
    {
      cwd: desktopRoot,
      env: {
        ...process.env,
        GOSLING_SHELL_PROFILE: profileFile,
        GOSLING_SHELL_CONSUMER_MANIFEST: path.resolve(input.consumerFile),
        ELECTRON_ARCH: input.architecture,
        APPLE_TEAM_ID: '',
        APPLE_ID: '',
        APPLE_ID_PASSWORD: '',
        KEYCHAIN_PATH: '',
        WINDOWS_CERTIFICATE_FILE: '',
        WINDOW_SIGNING_ROLE: '',
      },
    }
  );
  return verify({
    profileFile,
    platform: input.platform,
    architecture: input.architecture,
    packageDirectory,
    builtBinary,
    consumerFile: path.resolve(input.consumerFile),
  });
}

function main(argv) {
  const args = [...argv];
  if (args[0] === '--') args.shift();
  const platform = optionalOption(args, '--platform', process.platform);
  const architecture = optionalOption(args, '--arch', process.arch);
  const consumerFile = optionalOption(args, '--consumer', undefined);
  if (args.length !== 1) throw new Error(usage());
  return packageShell({ profileFile: args[0], consumerFile, platform, architecture });
}

if (require.main === module) {
  try {
    process.stdout.write(`${JSON.stringify(main(process.argv.slice(2)))}\n`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : 'shell packaging failed');
    process.exitCode = 1;
  }
}

module.exports = { hostTarget, main, packageShell, rustTarget };
