import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { defineConfig } from 'vite';

const require = createRequire(import.meta.url);
const { resolveConsumerManifest } = require('./scripts/shell-consumer');

const consumerFile = process.env.GOSLING_SHELL_CONSUMER_MANIFEST;
if (!consumerFile) throw new Error('shell renderer build requires a consumer manifest');
const consumer = resolveConsumerManifest(consumerFile);
const rendererEntry = consumer.rendererEntry;
const virtualConsumerRenderer = 'virtual:gosling-shell-consumer-renderer';
const resolvedVirtualConsumerRenderer = `\0${virtualConsumerRenderer}`;

export default defineConfig({
  base: './',
  resolve: {
    alias: {
      // The stable specifier a consumer renderer imports to compose the reusable Default Shell
      // application. Consumers may ignore it entirely and render their own tree.
      '@gosling-shell-ui': resolve(__dirname, 'src/shell-ui'),
    },
  },
  // No React plugin: its dev preamble injects an inline script, which shell.html's
  // `script-src 'self'` CSP forbids. Vite's esbuild transform handles .tsx through the
  // `jsx: react-jsx` setting in tsconfig.json, so the shell builds and serves without it.
  build: {
    target: 'esnext',
    rollupOptions: {
      input: resolve(__dirname, 'shell.html'),
    },
  },
  plugins: [
    {
      name: 'gosling-shell-consumer-renderer',
      resolveId(id: string) {
        return id === virtualConsumerRenderer ? resolvedVirtualConsumerRenderer : undefined;
      },
      load(id: string) {
        return id === resolvedVirtualConsumerRenderer
          ? `import ${JSON.stringify(rendererEntry)};`
          : undefined;
      },
    },
  ],
});
