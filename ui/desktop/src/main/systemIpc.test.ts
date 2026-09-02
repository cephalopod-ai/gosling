import { describe, expect, it, vi } from 'vitest';
import { registerSystemIpcHandlers, SYSTEM_IPC_CHANNELS } from './systemIpc';

vi.mock('electron', () => ({ BrowserWindow: {} }));

describe('system IPC registration', () => {
  it('registers every original system channel once', () => {
    const handle = vi.fn();
    registerSystemIpcHandlers(
      { handle },
      {
        app: {} as never,
        getSettings: vi.fn(),
        updateSettings: vi.fn(),
        createTray: vi.fn(),
        destroyTray: vi.fn(),
        focusWindow: vi.fn(),
        activeWakelockSessionsByWindow: new Map(),
        syncWindowPowerSaveBlocker: vi.fn(),
      }
    );
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(SYSTEM_IPC_CHANNELS);
  });
});
