// @vitest-environment node
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { APP_IPC_HANDLE_CHANNELS, APP_IPC_ON_CHANNELS, registerAppIpcHandlers } from './appIpc';
import type { AppIpcDependencies } from './appIpc';
import { desktopCommandChannels, rendererEventChannels } from '../ipc/channels';
import type { CreateChatWindowOptions } from '../ipc/channels';

const senderWindow = vi.hoisted(() => ({
  id: 1,
  isDestroyed: () => false,
  webContents: { send: vi.fn() },
}));

vi.mock('electron', () => ({
  BrowserWindow: {
    fromWebContents: () => senderWindow,
    getAllWindows: () => [senderWindow],
  },
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

describe('create-chat-window directory authorization', () => {
  let tempRoot: string;
  let grantedDir: string;
  let otherWorkspaceDir: string;
  let fallbackDir: string;
  let createChat: ReturnType<typeof vi.fn<AppIpcDependencies['createChat']>>;
  let createChatWindow: (event: unknown, options?: CreateChatWindowOptions) => void;

  beforeEach(() => {
    tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-app-ipc-'));
    grantedDir = path.join(tempRoot, 'alpha');
    otherWorkspaceDir = path.join(tempRoot, 'beta');
    fallbackDir = path.join(tempRoot, 'recent');
    for (const dir of [grantedDir, otherWorkspaceDir, fallbackDir]) fs.mkdirSync(dir);

    senderWindow.webContents.send.mockClear();
    createChat = vi.fn<AppIpcDependencies['createChat']>(async () => undefined);

    const on = vi.fn();
    registerAppIpcHandlers(
      { on, handle: vi.fn() },
      {
        app: { on: vi.fn() } as never,
        createNewWindow: vi.fn(),
        createChat,
        assertRendererFileAccess: async (_webContentsId: number, filePath: string) => {
          if (filePath === grantedDir) return grantedDir;
          throw new Error('Renderer file access denied for path outside approved roots');
        },
        firstGrantedRecentDirectory: () => fallbackDir,
        getConfiguredGoslingLocale: vi.fn(),
        log: { info: vi.fn(), warn: vi.fn() } as never,
      }
    );
    createChatWindow = on.mock.calls.find(
      ([channel]) => channel === desktopCommandChannels.createChatWindow
    )![1];
  });

  afterEach(() => {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  });

  const openWindow = async (options: CreateChatWindowOptions) => {
    createChatWindow({ sender: { id: 7 } }, options);
    await vi.waitFor(() => {
      expect(
        createChat.mock.calls.length + senderWindow.webContents.send.mock.calls.length
      ).toBeGreaterThan(0);
    });
  };

  const sentChannels = () => senderWindow.webContents.send.mock.calls.map(([channel]) => channel);

  it('opens a granted folder in a new window unchanged', async () => {
    await openWindow({ dir: grantedDir, resumeSessionId: 'alpha-session', viewType: 'pair' });

    expect(createChat).toHaveBeenCalledWith({
      initialMessage: undefined,
      dir: grantedDir,
      resumeSessionId: 'alpha-session',
      viewType: 'pair',
    });
    expect(sentChannels()).toEqual([]);
  });

  it('opens a session from a non-active workspace without granting its folder from the request', async () => {
    await openWindow({
      dir: otherWorkspaceDir,
      resumeSessionId: 'beta-session',
      viewType: 'pair',
    });

    expect(createChat).toHaveBeenCalledWith({
      initialMessage: undefined,
      dir: fallbackDir,
      resumeSessionId: 'beta-session',
      viewType: 'pair',
    });
    expect(sentChannels()).toEqual([]);
  });

  it('still refuses an ungranted folder for a new chat, without a fatal error', async () => {
    await openWindow({ dir: otherWorkspaceDir, query: 'hello' });

    expect(createChat).not.toHaveBeenCalled();
    expect(senderWindow.webContents.send).toHaveBeenCalledWith(
      rendererEventChannels.createChatWindowRefused,
      'unapproved-directory',
      otherWorkspaceDir
    );
    expect(sentChannels()).not.toContain(rendererEventChannels.fatalError);
  });

  it('refuses a missing session folder in the current window, without a fatal error', async () => {
    const missingDir = path.join(tempRoot, 'deleted');

    await openWindow({ dir: missingDir, resumeSessionId: 'moved-session', viewType: 'pair' });

    expect(createChat).not.toHaveBeenCalled();
    expect(senderWindow.webContents.send).toHaveBeenCalledWith(
      rendererEventChannels.createChatWindowRefused,
      'unavailable-directory',
      missingDir
    );
    expect(sentChannels()).not.toContain(rendererEventChannels.fatalError);
  });

  it('reports a window that fails to open without a fatal error', async () => {
    createChat.mockRejectedValueOnce(new Error('backend did not start'));

    createChatWindow({ sender: { id: 7 } }, { dir: grantedDir, resumeSessionId: 'alpha-session' });
    await vi.waitFor(() => expect(senderWindow.webContents.send).toHaveBeenCalled());

    expect(senderWindow.webContents.send).toHaveBeenCalledWith(
      rendererEventChannels.createChatWindowRefused,
      'failed',
      'backend did not start'
    );
    expect(sentChannels()).not.toContain(rendererEventChannels.fatalError);
  });
});
