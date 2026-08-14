const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const test = require('node:test');

const desktopRoot = path.resolve(__dirname, '..');
const repositoryRoot = path.resolve(desktopRoot, '..', '..');
const fixtures = [
  ['consumer-a', 'Shell consumer fixture A'],
  ['consumer-b', 'Shell consumer fixture B'],
];

function sourceHash(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

test('two consumers compile through the fixed shell host and retain distinct renderer output', () => {
  const hostFiles = ['shell.html', 'src/shell/main.ts', 'src/shell/preload.ts'].map((file) =>
    path.join(desktopRoot, file)
  );
  const hostHashes = hostFiles.map(sourceHash);
  const outputRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-consumer-vite-'));
  try {
    for (const [consumerId, expectedText] of fixtures) {
      const manifest = path.join(
        repositoryRoot,
        'fixtures',
        'shell-consumers',
        consumerId,
        'shell-consumer.json'
      );
      const output = path.join(outputRoot, consumerId);
      const result = spawnSync(
        'pnpm',
        [
          'exec',
          'vite',
          'build',
          '--config',
          'vite.shell.renderer.config.mts',
          '--outDir',
          output,
          '--emptyOutDir',
        ],
        {
          cwd: desktopRoot,
          encoding: 'utf8',
          env: { ...process.env, GOSLING_SHELL_CONSUMER_MANIFEST: manifest },
        }
      );
      assert.equal(result.status, 0, result.stderr);
      const bundle = fs
        .readdirSync(path.join(output, 'assets'))
        .find((file) => file.endsWith('.js'));
      assert.ok(bundle);
      assert.match(
        fs.readFileSync(path.join(output, 'assets', bundle), 'utf8'),
        new RegExp(expectedText)
      );
    }
    assert.deepEqual(hostFiles.map(sourceHash), hostHashes);
  } finally {
    fs.rmSync(outputRoot, { recursive: true, force: true });
  }
});
