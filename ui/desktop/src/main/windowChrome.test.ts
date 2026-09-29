import { beforeEach, describe, expect, it, vi } from 'vitest';
import { BrowserWindow, dialog } from 'electron';
import os from 'node:os';
import { createWindowChrome } from './windowChrome';

vi.mock('electron', () => ({
  BrowserWindow: { getFocusedWindow: vi.fn(), getAllWindows: vi.fn(() => []) },
  dialog: { showOpenDialog: vi.fn() },
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
  beforeEach(() => vi.clearAllMocks());

  it('carries an explicit native selection into the new window even for home', async () => {
    const createChat = vi.fn();
    const grantSelectedPath = vi.fn();
    vi.mocked(BrowserWindow.getFocusedWindow).mockReturnValue(null);
    vi.mocked(dialog.showOpenDialog).mockResolvedValue({
      canceled: false,
      filePaths: [os.homedir()],
    });
    const app = {} as never;
    const chrome = createWindowChrome({
      app,
      appConfig: {},
      getConfiguredGoslingLocale: vi.fn(),
      getAppUrl: vi.fn(),
      reactReadyWindows: new Set(),
      updateSettings: vi.fn(),
      createChat,
      firstGrantedRecentDirectory: vi.fn(),
      rendererDirectoryGrants: { grantSelectedPath } as never,
      log: { info: vi.fn() } as never,
    });

    await chrome.openDirectoryDialog();
    expect(createChat).toHaveBeenCalledWith(app, { nativeDirectorySelection: os.homedir() });
    expect(grantSelectedPath).toHaveBeenCalledWith(0, os.homedir());
  });
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
