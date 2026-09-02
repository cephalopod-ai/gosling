import { describe, expect, it, vi } from 'vitest';
import { APP_IPC_HANDLE_CHANNELS, APP_IPC_ON_CHANNELS, registerAppIpcHandlers } from './appIpc';

vi.mock('electron', () => ({
  BrowserWindow: {},
  Notification: class {},
  shell: {},
}));

describe('application IPC registration', () => {
  it('registers app activation and every original IPC channel once', () => {
    const appOn = vi.fn();
    const on = vi.fn();
    const handle = vi.fn();

    registerAppIpcHandlers(
      { on, handle },
      {
        app: { on: appOn } as never,
        createNewWindow: vi.fn(),
        createChat: vi.fn(),
        assertRendererFileAccess: vi.fn(),
        firstGrantedRecentDirectory: vi.fn(),
        getConfiguredGoslingLocale: vi.fn(),
        log: { info: vi.fn(), warn: vi.fn() } as never,
      }
    );

    expect(appOn.mock.calls.map(([eventName]) => eventName)).toEqual(['activate']);
    expect(on.mock.calls.map(([channel]) => channel)).toEqual(APP_IPC_ON_CHANNELS);
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(APP_IPC_HANDLE_CHANNELS);
  });
});
