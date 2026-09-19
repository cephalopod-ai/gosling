// Builds the ACP SDK only when its inputs changed.
//
// `pnpm install` runs build-gosling-sdk through postinstall, and `start-gui` and
// `package` then run it again, so `just run-ui` and `just package-ui` each paid
// for two full schema generations and TypeScript builds per invocation. Each one
// also cleared Vite's dependency cache, and the second clear landed immediately
// before Electron started, guaranteeing a cold pre-bundle on every launch.
//
// The stamp covers every input the build reads. Anything not hashed here would
// produce a stale SDK, so add new inputs to SOURCE_PATHS rather than relying on
// the build being re-run by hand.

const { createHash } = require('node:crypto');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const desktopRoot = path.resolve(__dirname, '..');
const repoRoot = path.resolve(desktopRoot, '../..');
const sdkRoot = path.join(repoRoot, 'ui/sdk');
const stampPath = path.join(sdkRoot, 'dist', '.build-stamp');

/// Every file the SDK build reads. `src/generated` is an output, not an input.
const SOURCE_PATHS = [
  path.join(repoRoot, 'crates/gosling/acp-schema.json'),
  path.join(repoRoot, 'crates/gosling/acp-meta.json'),
  path.join(sdkRoot, 'generate-schema.ts'),
  path.join(sdkRoot, 'package.json'),
  path.join(sdkRoot, 'tsconfig.json'),
  path.join(sdkRoot, 'src'),
  path.join(sdkRoot, 'scripts'),
];

function hashInto(hash, target) {
  let stats;
  try {
    stats = fs.statSync(target);
  } catch {
    hash.update(`missing:${target}\n`);
    return;
  }
  if (stats.isDirectory()) {
    for (const entry of fs.readdirSync(target).sort()) {
      if (entry === 'generated' || entry === 'node_modules') continue;
      hashInto(hash, path.join(target, entry));
    }
    return;
  }
  hash.update(`${path.relative(repoRoot, target)}:${stats.size}:`);
  hash.update(fs.readFileSync(target));
  hash.update('\n');
}

function currentStamp() {
  const hash = createHash('sha256');
  for (const source of SOURCE_PATHS) hashInto(hash, source);
  return hash.digest('hex');
}

function run(command, args, cwd) {
  execFileSync(command, args, { cwd, stdio: 'inherit' });
}

const stamp = currentStamp();
let previous = null;
try {
  previous = fs.readFileSync(stampPath, 'utf8').trim();
} catch {
  previous = null;
}

if (previous === stamp && fs.existsSync(path.join(sdkRoot, 'dist', 'index.js'))) {
  console.log('Gosling SDK is up to date; skipping rebuild.');
  process.exit(0);
}

run('pnpm', ['--filter', '@repo-makeover/gosling-sdk', 'run', 'build'], repoRoot);
// Vite pre-bundles the SDK, so its cache is only stale when the SDK actually changed.
run('node', [path.join(desktopRoot, 'scripts/clean-vite-cache.js')], desktopRoot);

fs.writeFileSync(stampPath, `${currentStamp()}\n`);
