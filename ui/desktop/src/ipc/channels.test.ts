// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { APP_IPC_HANDLE_CHANNELS, APP_IPC_ON_CHANNELS } from '../main/appIpc';
import { FILE_IPC_CHANNELS } from '../main/fileIpc';
import { RENDERER_IPC_HANDLE_CHANNELS, RENDERER_IPC_ON_CHANNELS } from '../main/rendererIpc';
import { SETTINGS_IPC_CHANNELS } from '../main/settingsIpc';
import { SYSTEM_IPC_CHANNELS } from '../main/systemIpc';
import { registerUpdateIpcHandlers, UPDATE_IPC_CHANNELS } from '../utils/autoUpdater';
import {
  desktopCommandChannels,
  desktopInvokeChannels,
  desktopMainOnlyChannels,
  desktopPreloadCommandChannels,
  desktopSendChannels,
  desktopSyncChannels,
} from './channels';
import type { DesktopCommandChannel, DesktopCommandPayloads } from './channels';

vi.mock('electron', () => ({
  app: { getPath: vi.fn(() => '/tmp') },
  BrowserWindow: {},
  clipboard: {},
  dialog: {},
  ipcMain: {},
  Menu: {},
  nativeImage: {},
  Notification: class {},
  shell: {},
  Tray: class {},
}));

vi.mock('electron-updater', () => ({ autoUpdater: {} }));
vi.mock('../utils/logger', () => ({ default: { info: vi.fn() } }));

type PayloadContractComplete =
  Exclude<DesktopCommandChannel, keyof DesktopCommandPayloads> extends never
    ? Exclude<keyof DesktopCommandPayloads, DesktopCommandChannel> extends never
      ? true
      : false
    : false;

const payloadContractComplete: PayloadContractComplete = true;

function sorted(channels: readonly string[]): string[] {
  return [...channels].sort();
}

describe('Desktop IPC contract', () => {
  it('defines one payload tuple for every unique command channel', () => {
    expect(payloadContractComplete).toBe(true);
    expect(new Set(Object.values(desktopCommandChannels)).size).toBe(
      Object.values(desktopCommandChannels).length
    );
    expect(new Set(desktopPreloadCommandChannels).size).toBe(desktopPreloadCommandChannels.length);
    expect(sorted(Object.values(desktopCommandChannels))).toEqual(
      sorted([...desktopPreloadCommandChannels, ...desktopMainOnlyChannels])
    );
  });

  it('matches every preload command kind to exactly one main registration', () => {
    const mainOnChannels = [...APP_IPC_ON_CHANNELS, ...RENDERER_IPC_ON_CHANNELS];
    const mainHandleChannels = [
      ...APP_IPC_HANDLE_CHANNELS,
      ...FILE_IPC_CHANNELS,
      ...RENDERER_IPC_HANDLE_CHANNELS,
      ...SETTINGS_IPC_CHANNELS,
      ...SYSTEM_IPC_CHANNELS,
      ...UPDATE_IPC_CHANNELS,
    ];

    expect(new Set(mainOnChannels).size).toBe(mainOnChannels.length);
    expect(new Set(mainHandleChannels).size).toBe(mainHandleChannels.length);
    expect(sorted(mainOnChannels)).toEqual(
      sorted([...desktopSendChannels, ...desktopSyncChannels])
    );
    expect(sorted(mainHandleChannels)).toEqual(
      sorted([...desktopInvokeChannels, ...desktopMainOnlyChannels])
    );
  });

  it('pins the updater declaration to its real registration function', () => {
    const handle = vi.fn();
    registerUpdateIpcHandlers({ handle });
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(UPDATE_IPC_CHANNELS);
  });
});
