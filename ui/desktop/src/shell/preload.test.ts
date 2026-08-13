import { beforeEach, describe, expect, it, vi } from 'vitest';
import { shellIpcChannels } from './ipc';

const electron = vi.hoisted(() => ({
  exposeInMainWorld: vi.fn(),
  invoke: vi.fn(),
  on: vi.fn(),
  removeListener: vi.fn(),
}));

vi.mock('electron', () => ({
  contextBridge: { exposeInMainWorld: electron.exposeInMainWorld },
  ipcRenderer: {
    invoke: electron.invoke,
    on: electron.on,
    removeListener: electron.removeListener,
  },
}));

describe('shell preload surface', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('exposes one frozen API with exactly the approved operations', async () => {
    vi.resetModules();
    const { goslingShellAPI } = await import('./preload');

    expect(electron.exposeInMainWorld).toHaveBeenCalledWith('goslingShell', goslingShellAPI);
    expect(Object.keys(goslingShellAPI).sort()).toEqual([
      'diagnostics',
      'external',
      'handoff',
      'runtime',
    ]);
    expect(Object.keys(goslingShellAPI.runtime).sort()).toEqual([
      'onChanged',
      'read',
      'retry',
      'stop',
    ]);
    expect(Object.keys(goslingShellAPI.diagnostics)).toEqual(['save']);
    expect(Object.keys(goslingShellAPI.handoff).sort()).toEqual(['confirm', 'prepare']);
    expect(Object.keys(goslingShellAPI.external)).toEqual(['open']);
    expect(Object.isFrozen(goslingShellAPI)).toBe(true);
    expect(Object.values(goslingShellAPI).every(Object.isFrozen)).toBe(true);
    expect(JSON.stringify(goslingShellAPI)).not.toMatch(
      /acp|secret|provisioning|namespace|filesystem|updater|release/i
    );
  });

  it('routes every invoke through only the typed shell channels', async () => {
    vi.resetModules();
    const { goslingShellAPI } = await import('./preload');
    const generation = { generation: 4 };
    const diagnostics = { ...generation, userGesture: true as const };
    const prepare = {
      ...generation,
      sessionId: 'session',
      question: 'question',
      requestedCapability: 'capability',
    };
    const confirm = { ...generation, handoffId: 'handoff' };

    await goslingShellAPI.runtime.read();
    await goslingShellAPI.runtime.retry(generation);
    await goslingShellAPI.runtime.stop(generation);
    await goslingShellAPI.diagnostics.save(diagnostics);
    await goslingShellAPI.handoff.prepare(prepare);
    await goslingShellAPI.handoff.confirm(confirm);
    await goslingShellAPI.external.open('https://support.example.test/help');

    expect(electron.invoke.mock.calls).toEqual([
      [shellIpcChannels.runtimeRead],
      [shellIpcChannels.runtimeRetry, generation],
      [shellIpcChannels.runtimeStop, generation],
      [shellIpcChannels.diagnosticsSave, diagnostics],
      [shellIpcChannels.handoffPrepare, prepare],
      [shellIpcChannels.handoffConfirm, confirm],
      [shellIpcChannels.externalOpen, 'https://support.example.test/help'],
    ]);
  });

  it('subscribes only to runtime.changed and removes its exact wrapper', async () => {
    vi.resetModules();
    const { goslingShellAPI } = await import('./preload');
    const listener = vi.fn();
    const dispose = goslingShellAPI.runtime.onChanged(listener);
    const [channel, wrapped] = electron.on.mock.calls[0];
    const state = {
      generation: 1,
      name: 'ready' as const,
      enteredAt: 'now',
      allowedActions: ['stop' as const],
    };

    expect(channel).toBe(shellIpcChannels.runtimeChanged);
    wrapped({}, state);
    expect(listener).toHaveBeenCalledWith(state);
    dispose();
    expect(electron.removeListener).toHaveBeenCalledWith(shellIpcChannels.runtimeChanged, wrapped);
  });
});
