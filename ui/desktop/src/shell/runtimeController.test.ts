import { EventEmitter } from 'node:events';
import { describe, expect, it, vi } from 'vitest';
import { ShellCompatibilityError } from './acpRuntime';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';
import { createShellRuntimeController } from './runtimeController';

const product = {
  id: 'gosling-shell-fixture-a',
  displayName: 'Gosling Shell Fixture A',
  version: '0.0.0-test',
  runtimeNamespace: 'shell-fixture-a',
  protocolScheme: 'gosling-fixture-a',
  executableName: 'gosling-shell-fixture-a',
  macosBundleId: 'io.github.repo-makeover.gosling.fixture.a',
  windowsAppId: 'Gosling.Shell.Fixture.A',
  linuxPackageName: 'gosling-shell-fixture-a',
  flatpakId: 'io.github.repo_makeover.Gosling.FixtureA',
};
const profile = {
  schemaVersion: 1,
  product,
  provisioningPath: 'fixture/provisioning.json',
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'current',
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: ['provisioning/read'],
  },
  assets: { root: 'assets', iconBase: 'assets/icon', requiredTargets: ['linux-x64'] },
  update: { enabled: false, channel: 'disabled' },
  distribution: { publishable: false, artifactPrefix: 'fixture', signingPolicy: 'none' },
} satisfies ResolvedShellProductProfile;
const manifest = {
  schemaVersion: 1,
  profileSchemaVersion: 1,
  profileHash: 'a'.repeat(64),
  product,
  target: 'linux-x64',
  platform: 'linux',
  architecture: 'x64',
  sourceClean: false,
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'b'.repeat(40),
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: ['provisioning/read'],
  },
} satisfies ShellBuildManifest;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function harness() {
  let tick = 0;
  const processes: EventEmitter[] = [];
  const cleanups: Array<ReturnType<typeof vi.fn>> = [];
  const connections: Array<{ close: ReturnType<typeof vi.fn>; closed: Promise<void> }> = [];
  const createHost = vi.fn(async (options) => {
    const process = new EventEmitter();
    const cleanup = vi.fn(async () => {});
    processes.push(process);
    cleanups.push(cleanup);
    return {
      backend: {
        acpUrl: `ws://127.0.0.1:${7000 + processes.length}/acp?token=secret`,
        workingDir: options.workingDir,
        process,
        errorLog: [],
        certFingerprint: null,
        cleanup,
        hasExited: () => false,
        getExitDetails: () => ({ code: null, signal: null }),
        startupDiagnosticsPath: null,
        getStartupDiagnostics: () => null,
        recordStartupEvent: vi.fn(),
      },
      windowOptions: {},
    } as never;
  });
  const connectAcp = vi.fn(async () => {
    const close = vi.fn();
    const connection = {
      client: {},
      initializeResponse: { protocolVersion: 1 },
      provisioning: {
        provisioning: { schemaVersion: 1, identity: product },
        validation: { valid: true },
      },
      compatibility: { compatible: true as const },
      prepareHandoff: vi.fn(),
      closed: new Promise<void>(() => {}),
      close,
    };
    connections.push(connection);
    return connection as never;
  });
  const controller = createShellRuntimeController(
    {
      profile,
      manifest,
      provisioningPath: '/resources/provisioning.json',
      diagnosticsDir: '/user/diagnostics',
      processRegistryPath: '/user/backend-processes.json',
      workingDir: '/workspace',
      isPackaged: true,
      resourcesPath: '/resources',
      preloadPath: '/app/shell-preload.js',
      sessionPartition: 'persist:gosling-shell-fixture-a',
      clientName: product.id,
      clientVersion: product.version,
    },
    {
      createHost,
      connectAcp,
      generateSecret: vi.fn(() => `secret-${createHost.mock.calls.length + 1}`),
      now: () => `2026-08-13T00:00:${String(tick++).padStart(2, '0')}Z`,
    }
  );
  return { cleanups, connectAcp, connections, controller, createHost, processes };
}

describe('shell runtime controller', () => {
  it('owns boot, validation, ready, and bounded stop with exact host inputs', async () => {
    const value = harness();
    const observed: string[] = [];
    value.controller.onChanged((state) => observed.push(`${state.generation}:${state.name}`));
    await value.controller.start();
    expect(observed).toEqual(['1:validating', '1:ready']);
    expect(value.createHost).toHaveBeenCalledWith({
      profile: {
        id: product.id,
        displayName: product.displayName,
        version: product.version,
        runtimeNamespace: product.runtimeNamespace,
        provisioningPath: '/resources/provisioning.json',
      },
      serverSecret: 'secret-1',
      workingDir: '/workspace',
      diagnosticsDir: '/user/diagnostics',
      processRegistryPath: '/user/backend-processes.json',
      isPackaged: true,
      resourcesPath: '/resources',
      preloadPath: '/app/shell-preload.js',
      sessionPartition: 'persist:gosling-shell-fixture-a',
    });
    expect(value.connectAcp).toHaveBeenCalledWith(
      expect.objectContaining({
        acpUrl: 'ws://127.0.0.1:7001/acp?token=secret',
        profile,
        manifest,
      })
    );
    await value.controller.stop(1);
    expect(value.connections[0].close).toHaveBeenCalledOnce();
    expect(value.cleanups[0]).toHaveBeenCalledOnce();
    expect(value.controller.read().name).toBe('stopped');
  });

  it('maps startup and compatibility failures to actionable states after cleanup', async () => {
    const startup = harness();
    startup.createHost.mockRejectedValueOnce(new Error('binary missing'));
    expect((await startup.controller.start()).name).toBe('offline');
    expect(startup.controller.read().reasonCode).toBe('STARTUP_FAILED');

    const compatibility = harness();
    compatibility.connectAcp.mockRejectedValueOnce(
      new ShellCompatibilityError({
        compatible: false,
        code: 'CORE_MISMATCH',
        expected: '0.1.0',
        actual: '0.2.0',
      })
    );
    expect((await compatibility.controller.start()).name).toBe('incompatible');
    expect(compatibility.cleanups[0]).toHaveBeenCalledOnce();
  });

  it('retries recoverable failures only after cleanup with fresh generation and secret', async () => {
    const value = harness();
    value.connectAcp.mockRejectedValueOnce(new Error('transport offline'));
    await value.controller.start();
    expect(value.controller.read()).toMatchObject({ generation: 1, name: 'offline' });
    await value.controller.retry(1);
    expect(value.cleanups[0]).toHaveBeenCalledOnce();
    expect(value.createHost.mock.calls[1][0].serverSecret).toBe('secret-2');
    expect(value.controller.read()).toMatchObject({ generation: 2, name: 'ready' });
  });

  it('ignores stale stop/retry and deduplicates concurrent start/stop', async () => {
    const value = harness();
    const host = deferred<never>();
    value.createHost.mockReturnValueOnce(host.promise);
    const first = value.controller.start();
    const second = value.controller.start();
    expect(first).toBe(second);
    await value.controller.stop(2);
    expect(value.controller.read().name).toBe('booting');
    host.reject(new Error('failed'));
    await first;
    const stopping = value.controller.stop(1);
    expect(value.controller.stop(1)).toBe(stopping);
    await stopping;
    expect(value.controller.read().name).toBe('stopped');
  });

  it('distinguishes unexpected backend exit from an expected stop', async () => {
    const crashed = harness();
    await crashed.controller.start();
    crashed.processes[0].emit('exit', 17, null);
    expect(crashed.controller.read()).toMatchObject({
      generation: 1,
      name: 'offline',
      reasonCode: 'BACKEND_EXITED',
    });
    expect(crashed.connections[0].close).toHaveBeenCalledOnce();

    const stopped = harness();
    await stopped.controller.start();
    await stopped.controller.stop(1);
    stopped.processes[0].emit('exit', 0, null);
    expect(stopped.controller.read().name).toBe('stopped');
  });

  it('cleans a live child when ACP transport closes unexpectedly', async () => {
    const value = harness();
    const transport = deferred<void>();
    value.connectAcp.mockImplementationOnce(async () => {
      const connection = {
        client: {},
        initializeResponse: { protocolVersion: 1 },
        provisioning: {
          provisioning: { schemaVersion: 1, identity: product },
          validation: { valid: true },
        },
        compatibility: { compatible: true as const },
        prepareHandoff: vi.fn(),
        closed: transport.promise,
        close: vi.fn(),
      };
      value.connections.push(connection);
      return connection as never;
    });
    await value.controller.start();
    transport.resolve();
    await vi.waitFor(() => expect(value.controller.read().name).toBe('offline'));
    expect(value.cleanups[0]).toHaveBeenCalledOnce();
  });

  it('ignores stale exit events after a successful retry', async () => {
    const value = harness();
    value.connectAcp.mockRejectedValueOnce(new Error('offline'));
    await value.controller.start();
    await value.controller.retry(1);
    value.processes[0].emit('exit', 1, null);
    expect(value.controller.read()).toMatchObject({ generation: 2, name: 'ready' });
  });
});
