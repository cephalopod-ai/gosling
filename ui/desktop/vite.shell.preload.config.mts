import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    ssr: true,
    outDir: '.vite/build',
    rollupOptions: {
      input: 'src/shell/preload.ts',
      output: {
        format: 'cjs',
        entryFileNames: 'shell-preload.js',
      },
      external: ['electron'],
    },
  },
});
