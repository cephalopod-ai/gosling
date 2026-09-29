// @vitest-environment node
import { EventEmitter } from 'node:events';
import { describe, expect, it, vi } from 'vitest';

const electron = vi.hoisted(() => ({
  exposeInMainWorld: vi.fn(),
  ipc: null as EventEmitter | null,
}));

vi.mock('electron', async () => {
  const { EventEmitter: Emitter } = await import('node:events');
  electron.ipc = new Emitter();
  return {
    default: {},
    contextBridge: { exposeInMainWorld: electron.exposeInMainWorld },
    ipcRenderer: electron.ipc,
    webUtils: {},
  };
});

type ExposedApi = {
  on: (channel: string, callback: (...args: unknown[]) => void) => unknown;
  off?: (channel: string, callback: (...args: unknown[]) => void) => void;
};

/** contextBridge passes the preload world a fresh proxy of a renderer function on every call. */
const acrossBridge =
  (callback: (...args: unknown[]) => void) =>
  (...args: unknown[]) =>
    callback(...args);

async function exposedElectronApi(): Promise<ExposedApi> {
  vi.resetModules();
  await import('./preload');
  const call = electron.exposeInMainWorld.mock.calls.find(([name]) => name === 'electron');
  return call![1] as ExposedApi;
}

describe('preload renderer-event listeners', () => {
  it('removes the registered listener through the function on() returns, across the bridge', async () => {
    const api = await exposedElectronApi();
    const ipc = electron.ipc!;
    const handleSetView = vi.fn();

    for (let chat = 0; chat < 12; chat += 1) {
      const unsubscribe = api.on('set-view', acrossBridge(handleSetView));
      expect(ipc.listenerCount('set-view')).toBe(1);
      (unsubscribe as () => void)();
      api.off?.('set-view', acrossBridge(handleSetView));
    }

    expect(ipc.listenerCount('set-view')).toBe(0);

    const unsubscribe = api.on('set-view', acrossBridge(handleSetView)) as () => void;
    ipc.emit('set-view', {}, 'settings');
    expect(handleSetView).toHaveBeenCalledTimes(1);
    unsubscribe();
    ipc.emit('set-view', {}, 'settings');
    expect(handleSetView).toHaveBeenCalledTimes(1);
  });
});
