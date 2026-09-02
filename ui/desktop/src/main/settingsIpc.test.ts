import { describe, expect, it, vi } from 'vitest';
import { registerSettingsIpcHandlers, SETTINGS_IPC_CHANNELS } from './settingsIpc';

vi.mock('electron', () => ({ dialog: { showOpenDialog: vi.fn() } }));

describe('settings IPC registration', () => {
  it('registers every original settings and research channel once', () => {
    const handle = vi.fn();
    registerSettingsIpcHandlers(
      { handle },
      {
        app: {} as never,
        getSettings: vi.fn(),
        updateSettings: vi.fn(),
        getExternalBackendSecret: vi.fn(),
        setExternalBackendSecret: vi.fn(),
        updateConfiguredLocale: vi.fn(),
        registerGlobalShortcuts: vi.fn(),
        setAutoDownloadDisabled: vi.fn(),
        rendererDirectoryGrants: {} as never,
      }
    );
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(SETTINGS_IPC_CHANNELS);
  });
});
