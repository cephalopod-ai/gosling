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
