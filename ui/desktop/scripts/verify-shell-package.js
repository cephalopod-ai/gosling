#!/usr/bin/env node

const path = require('node:path');
const { verifyShellPackage } = require('./shell-package-verifier');

function usage() {
  return [
    'Usage:',
    '  node scripts/verify-shell-package.js <profile> --platform <platform> --arch <architecture> --package <directory> --binary <file>',
  ].join('\n');
}

function option(args, name) {
  const indexes = args.flatMap((entry, index) => (entry === name ? [index] : []));
  if (indexes.length !== 1) throw new Error(`${name} must be provided exactly once`);
  const index = indexes[0];
  const value = args[index + 1];
  if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`);
  args.splice(index, 2);
  return value;
}

function main(argv) {
  const args = [...argv];
  if (args[0] === '--') args.shift();
  const platform = option(args, '--platform');
  const architecture = option(args, '--arch');
  const packageDirectory = option(args, '--package');
  const builtBinary = option(args, '--binary');
  if (args.length !== 1) throw new Error(usage());
  return verifyShellPackage({
    profileFile: path.resolve(args[0]),
    platform,
    architecture,
    packageDirectory: path.resolve(packageDirectory),
    builtBinary: path.resolve(builtBinary),
  });
}

try {
  process.stdout.write(`${JSON.stringify(main(process.argv.slice(2)))}\n`);
} catch (error) {
  console.error(error instanceof Error ? error.message : 'shell package verification failed');
  process.exitCode = 1;
}
