const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

test('Desktop discovers dependencies without the shell consumer virtual entry', async () => {
  const { resolveConfig, optimizeDeps } = await import('vite');
  const desktopRoot = path.resolve(__dirname, '..');
  const cacheDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-desktop-vite-'));

  try {
    const config = await resolveConfig(
      {
        root: desktopRoot,
        configFile: path.join(desktopRoot, 'vite.renderer.config.mts'),
        cacheDir,
      },
      'serve'
    );
    const metadata = await optimizeDeps(config, true, true);
    assert.ok(metadata.optimized.react);
    assert.ok(metadata.optimized['react-dom/client']);
  } finally {
    fs.rmSync(cacheDir, { recursive: true, force: true });
  }
});
