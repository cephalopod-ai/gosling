import { beforeEach, describe, expect, it, vi } from 'vitest';
import { registerSystemIpcHandlers, SYSTEM_IPC_CHANNELS } from './systemIpc';

const electronMocks = vi.hoisted(() => ({ fromWebContents: vi.fn() }));

vi.mock('electron', () => ({
  BrowserWindow: { fromWebContents: electronMocks.fromWebContents },
}));

describe('system IPC registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('registers every system channel once', () => {
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
        setSessionRecoveryActive: vi.fn(),
      }
    );
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(SYSTEM_IPC_CHANNELS);
  });

  it('binds a valid recovery update to the sending window', () => {
    const handlers = new Map<string, (...args: unknown[]) => unknown>();
    const setSessionRecoveryActive = vi.fn(() => true);
    registerSystemIpcHandlers(
      {
        handle: vi.fn((channel: string, handler: (...args: unknown[]) => unknown) => {
          handlers.set(channel, handler);
        }),
      },
      {
        app: {} as never,
        getSettings: vi.fn(),
        updateSettings: vi.fn(),
        createTray: vi.fn(),
        destroyTray: vi.fn(),
        focusWindow: vi.fn(),
        activeWakelockSessionsByWindow: new Map(),
        syncWindowPowerSaveBlocker: vi.fn(),
        setSessionRecoveryActive,
      }
    );
    const sender = {};
    electronMocks.fromWebContents.mockReturnValue({ id: 7 });
    const handler = handlers.get('set-session-recovery-active');

    expect(handler?.({ sender }, 'session-1', '/workspace', true)).toBe(true);
    expect(setSessionRecoveryActive).toHaveBeenCalledWith(7, 'session-1', '/workspace', true);

    expect(handler?.({ sender }, 'session-1', '/workspace', 'yes')).toBe(false);
    expect(setSessionRecoveryActive).toHaveBeenCalledTimes(1);
  });
});
