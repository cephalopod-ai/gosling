import { EventEmitter } from 'node:events';
import type {
  ShellCredentialListResponse_unstable,
  ShellDirectoryValidateResponse_unstable,
  ShellModuleListResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import { describe, expect, it, vi } from 'vitest';
import { ShellCompatibilityError } from './acpRuntime';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';
import { createShellRuntimeController, type ShellRuntimeController } from './runtimeController';
import { createShellDirectoryController } from './directoryController';
import { createShellCredentialController } from './credentialController';
import {
  defaultShellLocalSettings,
  type ShellLocalSettings,
  type ShellSettingsStore,
} from './localSettings';

function copySettings(settings: ShellLocalSettings): ShellLocalSettings {
  return {
    schemaVersion: settings.schemaVersion,
    appearance: { ...settings.appearance },
    workspace: { ...settings.workspace },
  };
}

function memorySettingsStore(lastWorkingDirectory: string | null): ShellSettingsStore {
  let settings = {
    ...defaultShellLocalSettings(),
    workspace: {
      lastWorkingDirectory,
      preferredCredentialProfileId: null as string | null,
    },
  };
  return {
    read: () => copySettings(settings),
    recovery: () => ({ status: 'loaded' as const, schemaVersion: 1 }),
    setAppearance: () => copySettings(settings),
    setLastWorkingDirectory(directory) {
      settings = { ...settings, workspace: { ...settings.workspace, lastWorkingDirectory: directory } };
      return copySettings(settings);
    },
    setPreferredCredentialProfileId(profileId) {
      settings = {
        ...settings,
        workspace: { ...settings.workspace, preferredCredentialProfileId: profileId },
      };
      return copySettings(settings);
    },
    reset: () => copySettings(settings),
  };
}

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

interface TestConnection {
  close: ReturnType<typeof vi.fn>;
  closed: Promise<void>;
  validateDirectory(candidate: string): Promise<ShellDirectoryValidateResponse_unstable>;
  listCredentials(): Promise<ShellCredentialListResponse_unstable>;
  listModules(): Promise<ShellModuleListResponse_unstable>;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((onResolve, onReject) => {
    resolve = onResolve;
    reject = onReject;
  });
  return { promise, resolve, reject };
}

function harness(withAdapter = false) {
  let tick = 0;
  const processes: EventEmitter[] = [];
  const cleanups: Array<ReturnType<typeof vi.fn>> = [];
  const connections: TestConnection[] = [];
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
      runtimeNamespace: product.runtimeNamespace,
      domainAdapter: withAdapter
        ? {
            descriptorId: 'neutral-fixture',
            protocolVersion: '1.0.0',
            actions: ['inspect', 'toggle'],
          }
        : null,
      createSession: vi
        .fn()
        .mockResolvedValue({ sessionId: 'session-a', workingDir: '/workspace' }),
      resumeSession: vi.fn(),
      prompt: vi.fn().mockResolvedValue({ stopReason: 'end_turn' }),
      cancel: vi.fn().mockResolvedValue(undefined),
      prepareHandoff: vi.fn(),
      validateDirectory: vi
        .fn()
        .mockImplementation(async (candidate: string) => ({
          status: 'valid' as const,
          canonicalPath: candidate,
        })),
      listCredentials: vi.fn().mockResolvedValue({ status: 'denied' as const, profiles: [] }),
      listModules: vi.fn().mockResolvedValue({ contractVersion: 1, modules: [] }),
      closed: new Promise<void>(() => {}),
      close,
    };
    connections.push(connection as unknown as TestConnection);
    return connection as never;
  });
  let controller: ShellRuntimeController;
  const settings = memorySettingsStore('/workspace');
  const directory = createShellDirectoryController({
    settings,
    showOpenDialog: vi.fn(async () => ({ canceled: true, filePaths: [] })),
    validate: (candidate) => connections[connections.length - 1].validateDirectory(candidate),
    generation: () => controller.read().generation,
    isSelectable: () => ({ allowed: true }),
  });
  const credentials = createShellCredentialController({
    settings,
    list: () => connections[connections.length - 1].listCredentials(),
    generation: () => controller.read().generation,
  });
  controller = createShellRuntimeController(
    {
      profile,
      manifest,
      provisioningPath: '/resources/provisioning.json',
      diagnosticsDir: '/user/diagnostics',
      processRegistryPath: '/user/backend-processes.json',
      workingDir: '/workspace',
      directory,
      credentials,
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
  return { cleanups, connectAcp, connections, controller, createHost, credentials, directory, processes };
}

describe('shell runtime controller', () => {
  it('owns boot, validation, ready, and bounded stop with exact host inputs', async () => {
    const value = harness();
    const observed: string[] = [];
    value.controller.onChanged((state) => {
      const entry = `${state.generation}:${state.name}`;
      if (observed[observed.length - 1] !== entry) observed.push(entry);
    });
    await value.controller.start();
    expect(observed).toEqual(['1:validating', '1:ready']);
    expect(value.controller.read().directory).toEqual({
      state: 'selected',
      path: '/workspace',
      label: 'workspace',
      reasonCode: null,
    });
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

    const adapterMismatch = harness();
    adapterMismatch.createHost.mockRejectedValueOnce(new Error('ADAPTER_DESCRIPTOR_MISMATCH'));
    expect((await adapterMismatch.controller.start()).name).toBe('incompatible');
    expect(adapterMismatch.controller.read().reasonCode).toBe('ADAPTER_DESCRIPTOR_MISMATCH');

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

    const relink = harness();
    relink.connectAcp.mockRejectedValueOnce(
      new ShellCompatibilityError(
        {
          compatible: false,
          code: 'PROVISIONING_INVALID',
          expected: true,
          actual: false,
        },
        [{ code: 'credential_profile_unavailable', path: 'session.credentialProfileId' }]
      )
    );
    expect((await relink.controller.start()).name).toBe('relink_required');
    expect(relink.controller.read().provisioningIssues).toEqual([
      { code: 'credential_profile_unavailable', path: 'session.credentialProfileId' },
    ]);
  });

  it('creates one application session only through the compatible main-owned ACP connection', async () => {
    const value = harness();
    await value.controller.start();
    const sessions = value.controller.getSessionController();
    expect(sessions).not.toBeNull();
    await expect(sessions!.create(1)).resolves.toMatchObject({
      sessionId: 'session-a',
      status: 'active',
    });
    const connection = value.connections[0] as unknown as {
      createSession: ReturnType<typeof vi.fn>;
      prompt: ReturnType<typeof vi.fn>;
    };
    expect(connection.createSession).toHaveBeenCalledOnce();
    const attempt = sessions!.submit({ generation: 1, sessionId: 'session-a', text: 'hello' });
    expect(connection.prompt).toHaveBeenCalledWith({
      sessionId: 'session-a',
      text: 'hello',
      messageId: attempt.promptAttemptId,
    });
    await value.controller.stop(1);
    expect(value.controller.getSessionController()).toBeNull();
  });

  it('projects ACP updates only through the active main-owned session', async () => {
    const value = harness();
    const updates: unknown[] = [];
    value.controller.onSessionUpdated((update) => updates.push(update));
    await value.controller.start();
    const sessions = value.controller.getSessionController()!;
    await sessions.create(1);
    sessions.submit({ generation: 1, sessionId: 'session-a', text: 'hello' });

    const connectCalls = value.connectAcp.mock.calls as unknown as Array<
      [{ callbacks: () => { sessionUpdate(notification: unknown): Promise<void> } }]
    >;
    const callbacks = connectCalls[0]![0].callbacks();
    await callbacks.sessionUpdate({
      sessionId: 'session-a',
      update: {
        sessionUpdate: 'agent_message_chunk',
        messageId: 'message-a',
        content: { type: 'text', text: 'response' },
        _meta: { private: 'discarded' },
      },
    } as never);
    await callbacks.sessionUpdate({
      sessionId: 'other',
      update: { sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: 'ignored' } },
    } as never);

    expect(updates.slice(0, 2)).toEqual([
      expect.objectContaining({ kind: 'started', updateSeq: 1 }),
      expect.objectContaining({
        kind: 'stream',
        updateSeq: 2,
        stream: { type: 'content', role: 'assistant', messageId: 'message-a', text: 'response' },
      }),
    ]);
  });

  it('maps prompt activity to a production busy-to-ready lifecycle transition', async () => {
    const value = harness();
    const observed: string[] = [];
    value.controller.onChanged((state) => observed.push(state.name));
    await value.controller.start();
    const sessions = value.controller.getSessionController()!;
    await sessions.create(1);
    sessions.submit({ generation: 1, sessionId: 'session-a', text: 'hello' });
    await vi.waitFor(() => expect(value.controller.read().name).toBe('ready'));
    expect(observed).toContain('busy');
    expect(observed[observed.length - 1]).toBe('ready');
  });

  it('returns a safe verified snapshot with active session and pending interaction facts', async () => {
    const value = harness();
    await value.controller.start();
    expect(value.controller.read()).toMatchObject({
      lifecycleState: 'ready',
      identity: { id: product.id, displayName: product.displayName, version: product.version },
      runtimeNamespace: product.runtimeNamespace,
      compatibility: { status: 'compatible' },
      session: null,
      adapter: null,
      pendingInteractions: [],
    });
    await value.controller.getSessionController()!.create(1);
    const connectCalls = value.connectAcp.mock.calls as unknown as Array<
      [{ callbacks: () => { requestPermission(request: unknown): Promise<unknown> } }]
    >;
    const callbacks = connectCalls[0]![0].callbacks();
    const pending = callbacks.requestPermission({
      sessionId: 'session-a',
      toolCall: { toolCallId: 'tool-a', title: 'Read source', kind: 'read', status: 'pending' },
      options: [
        { optionId: 'allow', name: 'Allow once', kind: 'allow_once' },
        { optionId: 'deny', name: 'Deny', kind: 'reject_once' },
      ],
    });
    expect(value.controller.read()).toMatchObject({
      session: { sessionId: 'session-a', status: 'active' },
      pendingInteractions: [
        {
          kind: 'permission',
          generation: 1,
          expiresAtGeneration: 1,
          sessionId: 'session-a',
          summary: { toolTitle: 'Read source', allowOnce: true, deny: true },
        },
      ],
    });
    const actionId = value.controller.read().pendingInteractions[0]!.actionId;
    value.controller
      .getInteractionController()!
      .respondPermission({ actionId, generation: 1, sessionId: 'session-a', allowOnce: false });
    await expect(pending).resolves.toEqual({ outcome: { outcome: 'selected', optionId: 'deny' } });
    expect(value.controller.read().pendingInteractions).toEqual([]);
    await expect(
      callbacks.requestPermission({
        sessionId: 'foreign-session',
        toolCall: { toolCallId: 'tool-b', title: 'Foreign', kind: 'read', status: 'pending' },
        options: [],
      })
    ).resolves.toEqual({ outcome: { outcome: 'cancelled' } });
    expect(value.controller.read().pendingInteractions).toEqual([]);
  });

  it('projects server-owned adapter status changes without exposing adapter authority', async () => {
    const value = harness(true);
    const observed: unknown[] = [];
    value.controller.onChanged((state) => observed.push(state.adapter));
    await value.controller.start();
    const connectCalls = value.connectAcp.mock.calls as unknown as Array<
      [
        {
          callbacks: () => {
            unstable_shellDomainStatus(notification: { status: 'crashed' }): Promise<void>;
          };
        },
      ]
    >;
    await connectCalls[0]![0].callbacks().unstable_shellDomainStatus({ status: 'crashed' });

    expect(value.controller.read().adapter).toEqual({
      descriptorId: 'neutral-fixture',
      protocolVersion: '1.0.0',
      actions: ['inspect', 'toggle'],
      status: 'crashed',
    });
    expect(observed).not.toContainEqual(expect.objectContaining({ transport: expect.anything() }));
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

  it('enters fatal when owned backend cleanup fails', async () => {
    const value = harness();
    await value.controller.start();
    value.cleanups[0].mockRejectedValueOnce(new Error('cleanup failed'));
    await value.controller.stop(1);
    expect(value.controller.read()).toMatchObject({ name: 'fatal', reasonCode: 'CLEANUP_FAILED' });
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
        runtimeNamespace: product.runtimeNamespace,
        prepareHandoff: vi.fn(),
        validateDirectory: vi.fn(async (candidate: string) => ({
          status: 'valid' as const,
          canonicalPath: candidate,
        })),
        listCredentials: vi.fn(async () => ({ status: 'denied' as const, profiles: [] })),
        listModules: vi.fn(async () => ({ contractVersion: 1, modules: [] })),
        closed: transport.promise,
        close: vi.fn(),
      };
      value.connections.push(connection as unknown as TestConnection);
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
