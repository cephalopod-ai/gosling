const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');
const test = require('node:test');

const packageRoot = path.resolve(__dirname, '..');
const metadata = require('../package.json');

function run(command, args, cwd) {
  return execFileSync(command, args, {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function externalConsumer() {
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-kit-external-'));
  const packed = JSON.parse(
    run('npm', ['pack', '--json', '--pack-destination', temporary], packageRoot)
  )[0].filename;
  const root = path.join(temporary, 'consumer');
  const installed = path.join(root, 'node_modules', '@repo-makeover', 'gosling-shell-kit');
  fs.mkdirSync(root, { recursive: true });
  writeJson(path.join(root, 'package.json'), {
    name: 'external-shell-proof',
    private: true,
    dependencies: { [metadata.name]: metadata.version },
  });
  run(
    'npm',
    ['install', '--ignore-scripts', '--no-package-lock', '--no-save', path.join(temporary, packed)],
    root
  );
  run('git', ['init', '-q'], root);
  run('git', ['config', 'user.email', 'shell-kit-test@example.invalid'], root);
  run('git', ['config', 'user.name', 'Shell Kit Test'], root);
  run('git', ['add', 'package.json'], root);
  run('git', ['commit', '-qm', 'initialize external consumer'], root);
  const cli = path.join(installed, 'bin', 'gosling-shell.js');
  return { temporary, root, cli };
}

test('a packed shell-kit scaffolds, checks, and resolves a consumer outside Gosling', () => {
  const fixture = externalConsumer();
  try {
    const initialized = JSON.parse(
      run(
        process.execPath,
        [fixture.cli, 'init', '--id', 'external-shell', '--display-name', 'External Shell'],
        fixture.root
      )
    );
    assert.equal(initialized.conformant, true);
    assert.equal(initialized.shellKitVersion, metadata.version);
    assert.equal(fs.existsSync(path.join(fixture.root, 'Cargo.toml')), false);
    assert.equal(fs.existsSync(path.join(fixture.root, 'ui', 'desktop')), false);

    const checked = JSON.parse(
      run(process.execPath, [fixture.cli, 'check', 'shell-consumer.json'], fixture.root)
    );
    assert.equal(checked.conformant, true);
    assert.equal(checked.consumerId, 'external-shell');

    const resolved = JSON.parse(
      run(
        process.execPath,
        [
          fixture.cli,
          'resolve',
          '--manifest',
          'shell-consumer.json',
          '--target',
          'linux-x64',
          '--output',
          'build/external-shell',
        ],
        fixture.root
      )
    );
    assert.equal(resolved.manifest.consumer.consumerId, 'external-shell');
    assert.equal(resolved.manifest.compatibility.goslingRevision, metadata.goslingRevision);
    assert.ok(fs.existsSync(path.join(fixture.root, 'build', 'external-shell', 'manifest.json')));
  } finally {
    fs.rmSync(fixture.temporary, { recursive: true, force: true });
  }
});

test('external registration fails closed for unpinned versions and caller-declared roots', () => {
  const fixture = externalConsumer();
  try {
    run(
      process.execPath,
      [fixture.cli, 'init', '--id', 'external-shell', '--display-name', 'External Shell'],
      fixture.root
    );
    const packageFile = path.join(fixture.root, 'package.json');
    const manifestFile = path.join(fixture.root, 'shell-consumer.json');
    const consumerPackage = JSON.parse(fs.readFileSync(packageFile, 'utf8'));
    consumerPackage.dependencies[metadata.name] = `^${metadata.version}`;
    writeJson(packageFile, consumerPackage);
    let result = spawnSync(process.execPath, [fixture.cli, 'check', manifestFile], {
      cwd: fixture.root,
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must pin .* exactly/);

    consumerPackage.dependencies[metadata.name] = metadata.version;
    writeJson(packageFile, consumerPackage);
    const manifest = JSON.parse(fs.readFileSync(manifestFile, 'utf8'));
    manifest.approvedRoot = fixture.root;
    writeJson(manifestFile, manifest);
    result = spawnSync(process.execPath, [fixture.cli, 'check', manifestFile], {
      cwd: fixture.root,
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /consumer\.approvedRoot is unknown/);

    delete manifest.approvedRoot;
    manifest.requiredShellKit = '0.0.1';
    writeJson(manifestFile, manifest);
    result = spawnSync(process.execPath, [fixture.cli, 'check', manifestFile], {
      cwd: fixture.root,
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /consumer\.requiredShellKit is invalid/);

    manifest.requiredShellKit = metadata.version;
    manifest.rendererEntry = '../outside.js';
    writeJson(manifestFile, manifest);
    result = spawnSync(process.execPath, [fixture.cli, 'check', manifestFile], {
      cwd: fixture.root,
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /consumer\.rendererEntry must be a safe consumer-relative path/);
  } finally {
    fs.rmSync(fixture.temporary, { recursive: true, force: true });
  }
});
