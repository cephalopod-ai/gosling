import { createRequire } from 'node:module';
import { defineConfig } from 'vite';

const require = createRequire(import.meta.url);
const { resolveForgeProjection } = require('./scripts/shell-forge-profile.js');
const product = resolveForgeProjection();
if (!product.shell) {
  throw new Error('shell main entry requires GOSLING_SHELL_PROFILE');
}

export default defineConfig({
  define: {
    __GOSLING_SHELL_RESOURCE_FILES__: JSON.stringify(product.shellResources),
  },
  build: {
    lib: {
      entry: 'src/shell/main.ts',
      fileName: () => 'main.js',
      formats: ['cjs'],
    },
  },
});
