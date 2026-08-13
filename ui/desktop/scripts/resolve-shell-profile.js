#!/usr/bin/env node

const path = require('node:path');
const {
  discoverProfiles,
  resolveProfile,
  resolveProfiles,
  writeBuildResolution,
} = require('./shell-profile');

function usage() {
  return [
    'Usage:',
    '  node scripts/resolve-shell-profile.js check <profile> [<profile> ...]',
    '  node scripts/resolve-shell-profile.js check-all',
    '  node scripts/resolve-shell-profile.js resolve <profile> --target <target> [--output <build-directory>]',
  ].join('\n');
}

function fail(message) {
  console.error(message);
  process.exitCode = 1;
}

function repositoryRoot() {
  return path.resolve(__dirname, '..', '..', '..');
}

function option(args, name) {
  const indexes = args.flatMap((entry, index) => (entry === name ? [index] : []));
  if (indexes.length === 0) return undefined;
  if (indexes.length > 1) throw new Error(`${name} may be provided only once`);
  const index = indexes[0];
  const value = args[index + 1];
  if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`);
  args.splice(index, 2);
  return value;
}

function summary(resolved) {
  return {
    productId: resolved.profile.product.id,
    profileHash: resolved.profileHash,
    resolvedGoslingRevision: resolved.resolvedGoslingRevision,
    sourceClean: resolved.sourceClean,
  };
}

function main(argv) {
  const [command, ...rest] = argv;
  if (command === 'check') {
    if (rest.length === 0) throw new Error(usage());
    return resolveProfiles(rest).map(summary);
  }
  if (command === 'check-all') {
    if (rest.length !== 0) throw new Error(usage());
    const files = discoverProfiles(repositoryRoot());
    if (files.length === 0) throw new Error('no source-controlled shell profiles found');
    return resolveProfiles(files).map(summary);
  }
  if (command === 'resolve') {
    const args = [...rest];
    const target = option(args, '--target');
    const output = option(args, '--output');
    if (args.length !== 1 || !target) throw new Error(usage());
    const resolved = resolveProfile(args[0]);
    const written = writeBuildResolution(resolved, target, output);
    return {
      ...summary(resolved),
      target,
      outputDirectory: written.outputDirectory,
      profileOutput: written.profileOutput,
      manifestOutput: written.manifestOutput,
    };
  }
  throw new Error(usage());
}

try {
  const result = main(process.argv.slice(2));
  process.stdout.write(`${JSON.stringify(result)}\n`);
} catch (error) {
  fail(error instanceof Error ? error.message : 'shell profile resolution failed');
}
