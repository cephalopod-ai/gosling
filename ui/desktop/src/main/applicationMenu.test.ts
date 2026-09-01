import { describe, expect, it, vi } from 'vitest';
import { defaultSettings } from '../utils/settings';
import { installApplicationMenu } from './applicationMenu';

const { getApplicationMenu } = vi.hoisted(() => ({
  getApplicationMenu: vi.fn(() => null),
}));

vi.mock('electron', () => ({
  BrowserWindow: {},
  Menu: {
    getApplicationMenu,
    buildFromTemplate: vi.fn(),
    setApplicationMenu: vi.fn(),
  },
  MenuItem: class {},
}));

describe('application menu installation', () => {
  it('preserves the no-menu startup path', () => {
    installApplicationMenu({
      app: {} as never,
      settings: defaultSettings,
      menuT: (label) => label,
      translateMenuLabels: vi.fn(),
      createNewWindow: vi.fn(),
      openDirectoryDialog: vi.fn(),
      buildRecentFilesMenu: vi.fn(() => []),
      focusWindow: vi.fn(),
      createLauncher: vi.fn(),
    });

    expect(getApplicationMenu).toHaveBeenCalledOnce();
  });
});
