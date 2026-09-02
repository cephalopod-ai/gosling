import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const directlyImportedModules = [
  'allowlist',
  'appIpc',
  'applicationMenu',
  'backendCertificateTrust',
  'fileIpc',
  'menuLocalization',
  'rendererIpc',
  'settingsIpc',
  'systemIpc',
  'windowChrome',
] as const;

describe('full Desktop compatibility facade', () => {
  it('keeps every extracted owner wired through the original entrypoint', () => {
    const mainSource = fs.readFileSync(path.join(process.cwd(), 'src', 'main.ts'), 'utf8');
    const rendererIpcSource = fs.readFileSync(
      path.join(process.cwd(), 'src', 'main', 'rendererIpc.ts'),
      'utf8'
    );

    expect(mainSource).toContain('Full Desktop compatibility facade');
    for (const moduleName of directlyImportedModules) {
      expect(mainSource).toContain(`from './main/${moduleName}'`);
    }
    expect(rendererIpcSource).toContain("from './gitIpc'");
  });

  it('keeps Forge pointed at the compatibility facade', () => {
    const forgeConfig = fs.readFileSync(path.join(process.cwd(), 'forge.config.ts'), 'utf8');

    expect(forgeConfig).toContain("entry: 'src/main.ts'");
    expect(forgeConfig).toContain("config: 'vite.main.config.mts'");
  });
});
