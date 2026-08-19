import { beforeEach, describe, expect, it, vi } from 'vitest';
import { shellIpcChannels } from './ipc';
import { registerShellIpc } from './ipcMain';
import { decodeShellOperationFailure, type ShellOperationFailure } from './operationFailure';

async function expectShellFailure(
  pending: Promise<unknown>,
  code: ShellOperationFailure['code']
): Promise<void> {
  try {
    await pending;
  } catch (error) {
    expect(decodeShellOperationFailure(error)).toMatchObject({ code });
    return;
  }
  throw new Error(`expected ${code}`);
}

function state(generation = 1): import('./runtimeSnapshot').ShellRuntimeSnapshot {
  return {
    generation,
    name: 'ready' as const,
    lifecycleState: 'ready' as const,
    enteredAt: 'now',
    allowedActions: ['stop' as const],
    identity: null,
    runtimeNamespace: null,
    declaredCapabilities: [],
    compatibility: { status: 'unverified' },
    provisioningIssues: [],
    directory: {
      state: 'unselected' as const,
      path: null,
      label: null,
      reasonCode: null,
      remembered: false,
    },
    settingsRecovery: { status: 'loaded' as const, schemaVersion: 1 },
    credentials: {
      catalogStatus: 'denied' as const,
      profiles: [],
      selectedProfileId: null,
      selectionStatus: 'none' as const,
    },
    modules: [],
    session: null,
    adapter: null,
    pendingInteractions: [],
  };
}

interface TestEvent {
  sender: { id: number };
  senderFrame: unknown;
}
type TestHandler = (event: TestEvent, request?: unknown) => Promise<unknown>;

function harness() {
  const handlers = new Map<string, TestHandler>();
  const ipcMain = {
    handle: vi.fn((channel: string, handler: TestHandler) => handlers.set(channel, handler)),
    removeHandler: vi.fn((channel: string) => handlers.delete(channel)),
  };
  const mainFrame = { url: 'file:///shell/index.html' };
  const renderer = { id: 7, mainFrame, isDestroyed: vi.fn(() => false), send: vi.fn() };
  const operations = {
    runtimeRead: vi.fn(() => state()),
    runtimeRetry: vi.fn((request) => ({ accepted: true, ...request, state: 'booting' as const })),
    runtimeStop: vi.fn((request) => ({ accepted: true, ...request, state: 'stopping' as const })),
    sessionCreate: vi.fn((request) => ({
      sessionId: 'session',
      status: 'active' as const,
      resumeKind: 'fresh' as const,
      resumeIntegrity: 'not_applicable' as const,
      workingDir: '/workspace',
      title: null,
      providerId: null,
      modelId: null,
      promptAttempt: null,
      ...request,
    })),
    sessionList: vi.fn(() => []),
    sessionResume: vi.fn((request) => ({
      sessionId: request.sessionId,
      status: 'active' as const,
      resumeKind: 'resumed' as const,
      resumeIntegrity: 'uncertain' as const,
      workingDir: '/workspace',
      title: null,
      providerId: null,
      modelId: null,
      promptAttempt: null,
    })),
    sessionTranscriptRead: vi.fn((request) => ({
      ...request,
      integrity: 'complete' as const,
      firstSeq: null,
      lastSeq: null,
      truncated: false,
      updates: [],
    })),
    sessionArtifactsRead: vi.fn(() => ({ artifacts: [], totalCount: 0, truncated: false })),
    promptSubmit: vi.fn(() => ({ promptAttemptId: 'attempt' })),
    promptCancel: vi.fn(),
    permissionRespond: vi.fn(),
    elicitationRespond: vi.fn(),
    domainSnapshot: vi.fn(() => ({ domainId: 'neutral-fixture', payload: {}, resources: [] })),
    domainAction: vi.fn(() => ({
      domainId: 'neutral-fixture',
      action: 'inspect',
      payload: {},
      resources: [],
    })),
    confirmationRespond: vi.fn(() => ({ status: 'denied' as const })),
    diagnosticsSave: vi.fn(() => ({ status: 'saved' as const, fileName: 'diagnostics.json' })),
    handoffPrepare: vi.fn((request) => ({
      generation: request.generation,
      handoff: {
        schemaVersion: 1,
        handoffId: 'handoff',
        origin: { id: 'fixture', displayName: 'Fixture', version: '0.0.0-test' },
        sourceSessionId: request.sessionId,
        question: request.question,
        requestedCapability: request.requestedCapability,
      },
    })),
    handoffConfirm: vi.fn(() => ({ opened: true })),
    externalOpen: vi.fn(() => ({ opened: true })),
    directorySelect: vi.fn(() => ({
      status: 'cancelled' as const,
      directory: {
        state: 'unselected' as const,
        path: null,
        label: null,
        reasonCode: null,
        remembered: false,
      },
    })),
    credentialSelect: vi.fn(() => ({
      catalogStatus: 'available' as const,
      profiles: [],
      selectedProfileId: null,
      selectionStatus: 'none' as const,
    })),
    sessionDetach: vi.fn(() => ({ detached: false, sessionId: null })),
    settingsRead: vi.fn(() => ({
      appearance: { theme: 'system' as const, textScale: 1 },
      recovery: { status: 'loaded' as const, schemaVersion: 1 },
    })),
    settingsAppearanceUpdate: vi.fn((request) => ({
      appearance: { theme: request.theme ?? 'system', textScale: request.textScale ?? 1 },
      recovery: { status: 'loaded' as const, schemaVersion: 1 },
    })),
    settingsReset: vi.fn(() => ({
      appearance: { theme: 'system' as const, textScale: 1 },
      recovery: { status: 'loaded' as const, schemaVersion: 1 },
    })),
  };
  const registration = registerShellIpc({
    ipcMain,
    renderer,
    operations,
    allowedExternalOrigins: new Set(['https://support.example.test']),
  });
  const event = { sender: renderer, senderFrame: mainFrame };
  const invoke = (channel: string, request?: unknown) => handlers.get(channel)!(event, request);
  return { event, handlers, invoke, ipcMain, operations, registration, renderer };
}

describe('shell IPC registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('registers exactly the approved invoke channels and removes all of them', () => {
    const { handlers, ipcMain, registration } = harness();
    expect([...handlers.keys()].sort()).toEqual(
      Object.values(shellIpcChannels)
        .filter((channel) => channel !== shellIpcChannels.runtimeChanged)
        .filter((channel) => channel !== shellIpcChannels.sessionUpdated)
        .filter((channel) => channel !== shellIpcChannels.permissionRequested)
        .filter((channel) => channel !== shellIpcChannels.elicitationRequested)
        .filter((channel) => channel !== shellIpcChannels.confirmationRequested)
        .sort()
    );
    registration.dispose();
    expect(ipcMain.removeHandler.mock.calls.map(([channel]) => channel).sort()).toEqual(
      Object.values(shellIpcChannels)
        .filter((channel) => channel !== shellIpcChannels.runtimeChanged)
        .filter((channel) => channel !== shellIpcChannels.sessionUpdated)
        .filter((channel) => channel !== shellIpcChannels.permissionRequested)
        .filter((channel) => channel !== shellIpcChannels.elicitationRequested)
        .filter((channel) => channel !== shellIpcChannels.confirmationRequested)
        .sort()
    );
  });

  it('rejects a different web contents or subframe sender', async () => {
    const { event, handlers, renderer } = harness();
    const read = handlers.get(shellIpcChannels.runtimeRead)!;
    await expect(read({ ...event, sender: { ...renderer, id: 8 } })).rejects.toThrow(
      'untrusted shell renderer'
    );
    await expect(
      read({ ...event, senderFrame: { url: 'file:///shell/frame.html' } })
    ).rejects.toThrow('untrusted shell renderer');
  });

  it('validates exact generation payloads and user gesture before dispatch', async () => {
    const { invoke, operations } = harness();
    await expectShellFailure(invoke(shellIpcChannels.runtimeRead, {}), 'INVALID_REQUEST');
    await expectShellFailure(
      invoke(shellIpcChannels.runtimeRetry, { generation: 0 }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.runtimeStop, { generation: 1, extra: true }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.diagnosticsSave, { generation: 1, userGesture: false }),
      'INVALID_REQUEST'
    );
    expect(operations.runtimeRetry).not.toHaveBeenCalled();
    expect(operations.runtimeStop).not.toHaveBeenCalled();
    expect(operations.diagnosticsSave).not.toHaveBeenCalled();
  });

  it('rejects session and prompt operations absent from the consumer declaration', async () => {
    const value = harness();
    value.registration.dispose();
    const registration = registerShellIpc({
      ipcMain: value.ipcMain,
      renderer: value.renderer,
      operations: value.operations,
      allowedExternalOrigins: new Set(),
      declaredCapabilities: new Set(['session.create']),
    });
    const create = value.handlers.get(shellIpcChannels.sessionCreate)!;
    const submit = value.handlers.get(shellIpcChannels.promptSubmit)!;
    await expect(create(value.event, { generation: 1 })).resolves.toMatchObject({
      sessionId: 'session',
    });
    await expectShellFailure(
      submit(value.event, { generation: 1, sessionId: 'session', text: 'hello' }),
      'CAPABILITY_UNAVAILABLE'
    );
    registration.dispose();
  });

  it('fences session discovery, transcript repair, and Outputs behind declared capabilities', async () => {
    const value = harness();
    value.registration.dispose();
    const registration = registerShellIpc({
      ipcMain: value.ipcMain,
      renderer: value.renderer,
      operations: value.operations,
      allowedExternalOrigins: new Set(),
      declaredCapabilities: new Set(['session.list']),
    });

    await expect(
      value.handlers.get(shellIpcChannels.sessionList)!(value.event, { generation: 1 })
    ).resolves.toEqual([]);
    await expectShellFailure(
      value.handlers.get(shellIpcChannels.sessionTranscriptRead)!(value.event, {
        generation: 1,
        sessionId: 'session',
      }),
      'CAPABILITY_UNAVAILABLE'
    );
    await expectShellFailure(
      value.handlers.get(shellIpcChannels.sessionArtifactsRead)!(value.event, {
        generation: 1,
        sessionId: 'session',
      }),
      'CAPABILITY_UNAVAILABLE'
    );
    registration.dispose();
  });

  it('never accepts a renderer-supplied path or an unexpected selection field', async () => {
    const { invoke, operations } = harness();

    await expectShellFailure(
      invoke(shellIpcChannels.directorySelect, {
        generation: 1,
        userGesture: true,
        path: '/etc',
      }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.directorySelect, { generation: 1, userGesture: false }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.credentialSelect, { generation: 1, profileId: 'a'.repeat(257) }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.credentialSelect, { generation: 1, profileId: 'work', secret: 'x' }),
      'INVALID_REQUEST'
    );
    expect(operations.directorySelect).not.toHaveBeenCalled();
    expect(operations.credentialSelect).not.toHaveBeenCalled();

    await expect(
      invoke(shellIpcChannels.directorySelect, { generation: 1, userGesture: true })
    ).resolves.toMatchObject({ status: 'cancelled' });
    await expect(
      invoke(shellIpcChannels.credentialSelect, { generation: 1, profileId: null })
    ).resolves.toMatchObject({ selectedProfileId: null });
    await expect(invoke(shellIpcChannels.sessionDetach, { generation: 1 })).resolves.toEqual({
      detached: false,
      sessionId: null,
    });
  });

  it('gates directory, credential, and detach operations on the consumer declaration', async () => {
    const value = harness();
    value.registration.dispose();
    const registration = registerShellIpc({
      ipcMain: value.ipcMain,
      renderer: value.renderer,
      operations: value.operations,
      allowedExternalOrigins: new Set(),
      declaredCapabilities: new Set(['directory.select']),
    });

    await expect(
      value.handlers.get(shellIpcChannels.directorySelect)!(value.event, {
        generation: 1,
        userGesture: true,
      })
    ).resolves.toMatchObject({ status: 'cancelled' });
    await expectShellFailure(
      value.handlers.get(shellIpcChannels.credentialSelect)!(value.event, {
        generation: 1,
        profileId: null,
      }),
      'CAPABILITY_UNAVAILABLE'
    );
    await expectShellFailure(
      value.handlers.get(shellIpcChannels.sessionDetach)!(value.event, { generation: 1 }),
      'CAPABILITY_UNAVAILABLE'
    );
    registration.dispose();
  });

  it('requires a session-bound shape before dispatching interaction responses', async () => {
    const { invoke, operations } = harness();
    const permission = {
      generation: 1,
      sessionId: 'session-a',
      actionId: 'permission-a',
      allowOnce: true,
    };
    const elicitation = {
      generation: 1,
      sessionId: 'session-a',
      actionId: 'elicitation-a',
      action: 'cancel',
    } as const;

    await invoke(shellIpcChannels.permissionRespond, permission);
    await invoke(shellIpcChannels.elicitationRespond, elicitation);
    expect(operations.permissionRespond).toHaveBeenCalledWith(permission);
    expect(operations.elicitationRespond).toHaveBeenCalledWith(elicitation);
    await expectShellFailure(
      invoke(shellIpcChannels.permissionRespond, {
        generation: 1,
        actionId: 'permission-a',
        allowOnce: true,
      }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.elicitationRespond, {
        generation: 1,
        actionId: 'elicitation-a',
        action: 'cancel',
      }),
      'INVALID_REQUEST'
    );
  });

  it('bounds, capability-gates, and relays domain requests through the main process', async () => {
    const value = harness();
    const snapshot = { generation: 3, input: { scope: 'neutral' } };
    const action = {
      generation: 3,
      sessionId: 'session-a',
      action: 'inspect',
      input: { id: 'one' },
    };
    const confirmation = {
      generation: 3,
      sessionId: 'session-a',
      actionId: 'confirm-a',
      approve: false,
    };

    await value.invoke(shellIpcChannels.domainSnapshot, snapshot);
    await value.invoke(shellIpcChannels.domainAction, action);
    await value.invoke(shellIpcChannels.confirmationRespond, confirmation);
    expect(value.operations.domainSnapshot).toHaveBeenCalledWith(snapshot);
    expect(value.operations.domainAction).toHaveBeenCalledWith(action);
    expect(value.operations.confirmationRespond).toHaveBeenCalledWith(confirmation);
    await expectShellFailure(
      value.invoke(shellIpcChannels.domainAction, { ...action, extra: true }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      value.invoke(shellIpcChannels.confirmationRespond, { ...confirmation, approve: 'yes' }),
      'INVALID_REQUEST'
    );

    value.registration.dispose();
    const registration = registerShellIpc({
      ipcMain: value.ipcMain,
      renderer: value.renderer,
      operations: value.operations,
      allowedExternalOrigins: new Set(),
      declaredCapabilities: new Set(['domain.snapshot']),
    });
    await expect(value.invoke(shellIpcChannels.domainSnapshot, snapshot)).resolves.toMatchObject({
      domainId: 'neutral-fixture',
    });
    await expectShellFailure(
      value.invoke(shellIpcChannels.domainAction, action),
      'CAPABILITY_UNAVAILABLE'
    );
    await expectShellFailure(
      value.invoke(shellIpcChannels.confirmationRespond, confirmation),
      'CAPABILITY_UNAVAILABLE'
    );
    registration.dispose();
  });

  it('validates handoff shape and enforces the total payload ceiling', async () => {
    const { invoke, operations } = harness();
    const valid = {
      generation: 3,
      sessionId: 'session',
      question: 'question',
      requestedCapability: 'capability',
      references: [{ kind: 'artifact', id: 'one' }],
      allowMutation: false,
    };
    await invoke(shellIpcChannels.handoffPrepare, valid);
    expect(operations.handoffPrepare).toHaveBeenCalledWith(valid);
    await expectShellFailure(
      invoke(shellIpcChannels.handoffPrepare, { ...valid, secret: 'forbidden' }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.handoffPrepare, { ...valid, question: 'q'.repeat(65 * 1024) }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.handoffConfirm, { generation: 3, handoffId: '' }),
      'INVALID_REQUEST'
    );
  });

  it('opens only bounded credential-free URLs at configured HTTP(S) origins', async () => {
    const { invoke, operations } = harness();
    await invoke(shellIpcChannels.externalOpen, 'https://support.example.test/help');
    expect(operations.externalOpen).toHaveBeenCalledWith('https://support.example.test/help');
    await expectShellFailure(
      invoke(shellIpcChannels.externalOpen, 'https://other.example.test/help'),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.externalOpen, 'https://user:pass@support.example.test/help'),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.externalOpen, 'file:///tmp/secret'),
      'INVALID_REQUEST'
    );
  });

  it('reads settings without accepting a request and never gates it on a declared capability', async () => {
    const value = harness();
    value.registration.dispose();
    const registration = registerShellIpc({
      ipcMain: value.ipcMain,
      renderer: value.renderer,
      operations: value.operations,
      allowedExternalOrigins: new Set(),
      declaredCapabilities: new Set(),
    });
    const read = value.handlers.get(shellIpcChannels.settingsRead)!;
    await expectShellFailure(read(value.event, { generation: 1 }), 'INVALID_REQUEST');
    await expect(read(value.event)).resolves.toMatchObject({
      appearance: { theme: 'system', textScale: 1 },
    });
    registration.dispose();
  });

  it('validates settings.appearance.update fields and rejects unsupported ones', async () => {
    const { invoke, operations } = harness();

    await expectShellFailure(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, theme: 'neon' }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, textScale: 0.1 }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, textScale: 0 }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, textScale: 2.1 }),
      'INVALID_REQUEST'
    );
    await expectShellFailure(
      invoke(shellIpcChannels.settingsAppearanceUpdate, {
        generation: 1,
        theme: 'dark',
        credentialProfileId: 'x',
      }),
      'INVALID_REQUEST'
    );
    expect(operations.settingsAppearanceUpdate).not.toHaveBeenCalled();

    await expect(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, theme: 'dark' })
    ).resolves.toMatchObject({ appearance: { theme: 'dark' } });
    await expect(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1 })
    ).resolves.toMatchObject({ appearance: { theme: 'system' } });
    await expect(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, textScale: 0.8 })
    ).resolves.toMatchObject({ appearance: { textScale: 0.8 } });
    await expect(
      invoke(shellIpcChannels.settingsAppearanceUpdate, { generation: 1, textScale: 2 })
    ).resolves.toMatchObject({ appearance: { textScale: 2 } });
  });

  it('requires an explicit user gesture before settings.reset dispatches', async () => {
    const { invoke, operations } = harness();
    await expectShellFailure(
      invoke(shellIpcChannels.settingsReset, { generation: 1, userGesture: false }),
      'INVALID_REQUEST'
    );
    expect(operations.settingsReset).not.toHaveBeenCalled();
    await expect(
      invoke(shellIpcChannels.settingsReset, { generation: 1, userGesture: true })
    ).resolves.toMatchObject({ appearance: { theme: 'system' } });
    expect(operations.settingsReset).toHaveBeenCalledWith({ generation: 1, userGesture: true });
  });

  it('rejects oversized operation responses', async () => {
    const value = harness();
    value.operations.runtimeRead.mockReturnValue({ ...state(), reasonCode: 'x'.repeat(65 * 1024) });
    await expectShellFailure(value.invoke(shellIpcChannels.runtimeRead), 'INVALID_REQUEST');
  });

  it('generation-fences runtime events and sends only the allowlisted event channel', () => {
    const { registration, renderer } = harness();
    expect(registration.publishRuntimeChanged(state(2))).toBe(true);
    expect(registration.publishRuntimeChanged(state(1))).toBe(false);
    expect(registration.publishRuntimeChanged(state(2))).toBe(true);
    expect(renderer.send.mock.calls).toEqual([
      [shellIpcChannels.runtimeChanged, state(2)],
      [shellIpcChannels.runtimeChanged, state(2)],
    ]);
  });

  it('drops lifecycle events when renderer contents are destroyed or close during send', () => {
    const { registration, renderer } = harness();
    renderer.isDestroyed.mockReturnValueOnce(true);
    expect(registration.publishRuntimeChanged(state(2))).toBe(false);
    renderer.send.mockImplementationOnce(() => {
      throw new Error('Object has been destroyed');
    });
    expect(registration.publishRuntimeChanged(state(2))).toBe(false);
    expect(registration.publishRuntimeChanged(state(2))).toBe(true);
    expect(renderer.send).toHaveBeenCalledTimes(2);
  });

  it('routes domain confirmation requests only through the confirmation event channel', () => {
    const { registration, renderer } = harness();
    const interaction = {
      actionId: 'confirm-a',
      generation: 1,
      expiresAtGeneration: 1,
      sessionId: 'session-a',
      promptAttemptId: null,
      kind: 'confirm' as const,
      summary: { action: 'replace', inputFields: ['outputId'] },
    };
    expect(registration.publishInteractionRequested(interaction)).toBe(true);
    expect(renderer.send).toHaveBeenCalledWith(shellIpcChannels.confirmationRequested, interaction);
  });
});
