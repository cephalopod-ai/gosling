const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const { main, packageShell, rustTarget } = require('./package-shell');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixtureA = path.join(
  repositoryRoot,
  'fixtures',
  'shell-products',
  'fixture-a',
  'product-profile.json'
);
const consumerA = path.join(
  repositoryRoot,
  'fixtures',
  'shell-consumers',
  'consumer-a',
  'shell-consumer.json'
);

test('local shell package wrapper builds, stages, packages, and verifies in order', () => {
  const events = [];
  const result = packageShell(
    { profileFile: fixtureA, consumerFile: consumerA, platform: 'darwin', architecture: 'arm64' },
    {
      hostTarget: () => 'aarch64-apple-darwin',
      run(command, args, options) {
        events.push({ type: 'run', command, args, options });
      },
      copyBinary(source, destination, platform) {
        events.push({ type: 'copy', source, destination, platform });
      },
      verify(input) {
        events.push({ type: 'verify', input });
        return { verified: true };
      },
    }
  );

  assert.deepEqual(result, { verified: true });
  assert.deepEqual(
    events.map((event) => event.type),
    ['run', 'copy', 'run', 'run', 'verify']
  );
  assert.deepEqual(events[0].args, [
    'cargo',
    'build',
    '--release',
    '--target',
    'aarch64-apple-darwin',
    '-p',
    'gosling-cli',
    '--bin',
    'gosling',
  ]);
  assert.match(events[1].source, /target\/aarch64-apple-darwin\/release\/gosling$/);
  assert.match(
    events[1].destination,
    /build\/shell-packages\/gosling-shell-fixture-a\/macos-arm64\/bin\/gosling$/
  );
  assert.deepEqual(events[2].args, ['run', 'build-gosling-sdk']);
  assert.deepEqual(events[3].args, [
    'exec',
    'electron-forge',
    'package',
    '--platform',
    'darwin',
    '--arch',
    'arm64',
  ]);
  assert.equal(events[3].options.env.GOSLING_SHELL_PROFILE, fixtureA);
  assert.equal(events[3].options.env.GOSLING_SHELL_CONSUMER_MANIFEST, consumerA);
  assert.equal(events[3].options.env.ELECTRON_ARCH, 'arm64');
  assert.equal(events[3].options.env.APPLE_TEAM_ID, '');
  assert.match(events[4].input.packageDirectory, /Gosling Shell Fixture A-darwin-arm64$/);
});

test('wrapper requires exactly one source-controlled profile and a supported target', () => {
  assert.throws(() => main([]), /Usage/);
  assert.throws(() => main([fixtureA, fixtureA]), /Usage/);
  assert.throws(
    () =>
      packageShell(
        { profileFile: fixtureA, consumerFile: consumerA, platform: 'linux', architecture: 'arm64' },
        {
          hostTarget: () => 'aarch64-unknown-linux-gnu',
          run() {},
          copyBinary() {},
          verify() {},
        }
      ),
    /unsupported Forge target/
  );
  assert.equal(rustTarget('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.throws(
    () =>
      packageShell(
        { profileFile: fixtureA, consumerFile: consumerA, platform: 'darwin', architecture: 'x64' },
        {
          hostTarget: () => 'aarch64-apple-darwin',
          run() {},
          copyBinary() {},
          verify() {},
        }
      ),
    /requires the selected platform and architecture to match this host/
  );
  assert.throws(
    () =>
      packageShell(
        { profileFile: fixtureA, platform: 'darwin', architecture: 'arm64' },
        { hostTarget: () => 'aarch64-apple-darwin' }
      ),
    /requires a consumer manifest/
  );
});
