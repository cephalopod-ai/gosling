import { describe, expect, it, vi } from 'vitest';
import { createWindowChrome } from './windowChrome';

vi.mock('electron', () => ({
  BrowserWindow: {},
  dialog: {},
  screen: {},
  Tray: class {},
}));
vi.mock('../utils/autoUpdater', () => ({
  getUpdateAvailable: vi.fn(),
  setTrayRef: vi.fn(),
  updateTrayMenu: vi.fn(),
}));
vi.mock('../utils/recentDirs', () => ({ addRecentDir: vi.fn(), loadRecentDirs: vi.fn() }));

describe('window chrome ownership', () => {
  it('starts without a tray and exposes every facade callback', () => {
    const windowChrome = createWindowChrome({
      app: {} as never,
      appConfig: {},
      getConfiguredGoslingLocale: vi.fn(),
      getAppUrl: vi.fn(),
      reactReadyWindows: new Set(),
      updateSettings: vi.fn(),
      createChat: vi.fn(),
      firstGrantedRecentDirectory: vi.fn(),
      rendererDirectoryGrants: {} as never,
      log: { info: vi.fn() } as never,
    });

    expect(windowChrome.hasTray()).toBe(false);
    expect(Object.keys(windowChrome)).toEqual([
      'createLauncher',
      'destroyTray',
      'createTray',
      'buildRecentFilesMenu',
      'openDirectoryDialog',
      'hasTray',
    ]);
  });
});
