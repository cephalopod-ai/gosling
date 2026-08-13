import { beforeEach, describe, expect, it, vi } from 'vitest';

const startGoslingServe = vi.hoisted(() => vi.fn());
vi.mock('./goslingServe', () => ({ startGoslingServe }));

import { createMinimalShellHost } from './shellHost';

describe('minimal shell host', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    startGoslingServe.mockResolvedValue({ acpUrl: 'ws://127.0.0.1/acp?token=secret' });
  });

  it('passes fixed runtime inputs and applies the dedicated preload and partition', async () => {
    const runtime = await createMinimalShellHost({
      profile: {
        id: 'fixture-a',
        displayName: 'Fixture A',
        version: '0.0.0-test',
        runtimeNamespace: 'fixture-a-runtime',
        provisioningPath: '/resources/provisioning.json',
      },
      serverSecret: 'secret',
      workingDir: '/workspace',
      diagnosticsDir: '/user/diagnostics',
      processRegistryPath: '/user/backend-processes.json',
      isPackaged: true,
      resourcesPath: '/resources',
      preloadPath: '/app/shell-preload.js',
      sessionPartition: 'persist:gosling-shell-fixture-a',
    });
    expect(startGoslingServe).toHaveBeenCalledWith({
      dir: '/workspace',
      serverSecret: 'secret',
      shell: {
        id: 'fixture-a',
        displayName: 'Fixture A',
        version: '0.0.0-test',
        runtimeNamespace: 'fixture-a-runtime',
        provisioningPath: '/resources/provisioning.json',
      },
      diagnosticsDir: '/user/diagnostics',
      processRegistryPath: '/user/backend-processes.json',
      isPackaged: true,
      resourcesPath: '/resources',
    });
    expect(runtime.windowOptions.webPreferences).toMatchObject({
      preload: '/app/shell-preload.js',
      partition: 'persist:gosling-shell-fixture-a',
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    });
  });
});
