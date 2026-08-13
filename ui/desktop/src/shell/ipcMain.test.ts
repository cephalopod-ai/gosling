import { beforeEach, describe, expect, it, vi } from 'vitest';
import { shellIpcChannels } from './ipc';
import { registerShellIpc } from './ipcMain';

function state(generation = 1): import('./lifecycle').ShellLifecycleState {
  return {
    generation,
    name: 'ready' as const,
    enteredAt: 'now',
    allowedActions: ['stop' as const],
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
  const renderer = { id: 7, mainFrame, send: vi.fn() };
  const operations = {
    runtimeRead: vi.fn(() => state()),
    runtimeRetry: vi.fn((request) => ({ accepted: true, ...request, state: 'booting' as const })),
    runtimeStop: vi.fn((request) => ({ accepted: true, ...request, state: 'stopping' as const })),
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
        .sort()
    );
    registration.dispose();
    expect(ipcMain.removeHandler.mock.calls.map(([channel]) => channel).sort()).toEqual(
      Object.values(shellIpcChannels)
        .filter((channel) => channel !== shellIpcChannels.runtimeChanged)
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
    await expect(invoke(shellIpcChannels.runtimeRead, {})).rejects.toThrow('does not accept');
    await expect(invoke(shellIpcChannels.runtimeRetry, { generation: 0 })).rejects.toThrow(
      'positive integer'
    );
    await expect(
      invoke(shellIpcChannels.runtimeStop, { generation: 1, extra: true })
    ).rejects.toThrow('unsupported fields');
    await expect(
      invoke(shellIpcChannels.diagnosticsSave, { generation: 1, userGesture: false })
    ).rejects.toThrow('explicit user gesture');
    expect(operations.runtimeRetry).not.toHaveBeenCalled();
    expect(operations.runtimeStop).not.toHaveBeenCalled();
    expect(operations.diagnosticsSave).not.toHaveBeenCalled();
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
    await expect(
      invoke(shellIpcChannels.handoffPrepare, { ...valid, secret: 'forbidden' })
    ).rejects.toThrow('unsupported fields');
    await expect(
      invoke(shellIpcChannels.handoffPrepare, { ...valid, question: 'q'.repeat(65 * 1024) })
    ).rejects.toThrow('size limit');
    await expect(
      invoke(shellIpcChannels.handoffConfirm, { generation: 3, handoffId: '' })
    ).rejects.toThrow('non-empty bounded string');
  });

  it('opens only bounded credential-free URLs at configured HTTP(S) origins', async () => {
    const { invoke, operations } = harness();
    await invoke(shellIpcChannels.externalOpen, 'https://support.example.test/help');
    expect(operations.externalOpen).toHaveBeenCalledWith('https://support.example.test/help');
    await expect(
      invoke(shellIpcChannels.externalOpen, 'https://other.example.test/help')
    ).rejects.toThrow('not allowlisted');
    await expect(
      invoke(shellIpcChannels.externalOpen, 'https://user:pass@support.example.test/help')
    ).rejects.toThrow('credentials');
    await expect(invoke(shellIpcChannels.externalOpen, 'file:///tmp/secret')).rejects.toThrow(
      'not allowlisted'
    );
  });

  it('rejects oversized operation responses', async () => {
    const value = harness();
    value.operations.runtimeRead.mockReturnValue({ ...state(), reasonCode: 'x'.repeat(65 * 1024) });
    await expect(value.invoke(shellIpcChannels.runtimeRead)).rejects.toThrow(
      'response exceeds the channel size limit'
    );
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
});
