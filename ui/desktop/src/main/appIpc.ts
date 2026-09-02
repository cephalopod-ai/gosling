// Owns post-ready app lifecycle and renderer-to-window application IPC registration.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports registerAppIpcHandlers; it re-exports none.

import type { App, BrowserWindow as BrowserWindowType, IpcMain } from 'electron';
import { BrowserWindow, Notification, shell } from 'electron';
import { desktopCommandChannels, rendererEventChannels } from '../ipc/channels';
import { errorMessage } from '../utils/conversionUtils';
import type logger from '../utils/logger';
import { normalizeWebUrl } from '../utils/urlSecurity';

interface CreateChatWindowOptions {
  query?: string;
  dir?: string;
  resumeSessionId?: string;
  viewType?: string;
}

interface CreateChatOptions {
  initialMessage?: string;
  dir?: string;
  resumeSessionId?: string;
  viewType?: string;
}

export interface AppIpcDependencies {
  app: App;
  createNewWindow: () => Promise<BrowserWindowType | undefined>;
  createChat: (options: CreateChatOptions) => Promise<BrowserWindowType | undefined>;
  assertRendererFileAccess: (webContentsId: number, filePath: string) => Promise<string>;
  firstGrantedRecentDirectory: (webContentsId?: number) => string | undefined;
  getConfiguredGoslingLocale: () => string | undefined;
  log: Pick<typeof logger, 'info' | 'warn'>;
}

export const APP_IPC_ON_CHANNELS = [
  desktopCommandChannels.createChatWindow,
  desktopCommandChannels.closeWindow,
  'notify',
  'logInfo',
  desktopCommandChannels.broadcastThemeChange,
  desktopCommandChannels.broadcastWorkspaceChange,
  'reload-app',
  'open-in-chrome',
  'restart-app',
  desktopCommandChannels.getAppVersion,
  desktopCommandChannels.getAppLocale,
] as const;

export const APP_IPC_HANDLE_CHANNELS = ['open-directory-in-explorer'] as const;

export function registerAppIpcHandlers(
  targetIpcMain: Pick<IpcMain, 'on' | 'handle'>,
  dependencies: AppIpcDependencies
): void {
  const {
    app,
    createNewWindow,
    createChat,
    assertRendererFileAccess,
    firstGrantedRecentDirectory,
    getConfiguredGoslingLocale,
    log,
  } = dependencies;

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      void createNewWindow();
    }
  });

  targetIpcMain.on(
    desktopCommandChannels.createChatWindow,
    (event, options: CreateChatWindowOptions = {}) => {
      void (async () => {
        const { query, dir, resumeSessionId, viewType } = options;
        const resolvedDir =
          typeof dir === 'string' && dir.trim()
            ? await assertRendererFileAccess(event.sender.id, dir)
            : firstGrantedRecentDirectory(event.sender.id);

        const isFromLauncher = query && !resumeSessionId && !viewType;

        if (isFromLauncher) {
          const senderWindow = BrowserWindow.fromWebContents(event.sender);
          const launcherWindowId = senderWindow?.id;
          const allWindows = BrowserWindow.getAllWindows();

          const existingWindows = allWindows.filter(
            (window) => !window.isDestroyed() && window.id !== launcherWindowId
          );

          if (existingWindows.length > 0) {
            const targetWindow = existingWindows[0];
            targetWindow.show();
            targetWindow.focus();
            targetWindow.webContents.send(rendererEventChannels.setInitialMessage, query);
            return;
          }
        }

        await createChat({
          initialMessage: query,
          dir: resolvedDir,
          resumeSessionId,
          viewType,
        });
      })().catch((error) => {
        log.warn('[Main] Rejected create-chat request:', errorMessage(error));
        const senderWindow = BrowserWindow.fromWebContents(event.sender);
        if (senderWindow && !senderWindow.isDestroyed()) {
          senderWindow.webContents.send(
            rendererEventChannels.fatalError,
            'The selected working directory must be chosen or relinked before starting a chat.'
          );
        }
      });
    }
  );

  targetIpcMain.on(desktopCommandChannels.closeWindow, (event) => {
    const window = BrowserWindow.fromWebContents(event.sender);
    if (window && !window.isDestroyed()) {
      window.close();
    }
  });

  targetIpcMain.on('notify', (event, data) => {
    try {
      // Validate notification data
      if (!data || typeof data !== 'object') {
        console.error('Invalid notification data');
        return;
      }

      // Validate title and body
      if (typeof data.title !== 'string' || typeof data.body !== 'string') {
        console.error('Invalid notification title or body');
        return;
      }

      // Limit the length of title and body
      const MAX_LENGTH = 1000;
      if (data.title.length > MAX_LENGTH || data.body.length > MAX_LENGTH) {
        console.error('Notification title or body too long');
        return;
      }

      // Remove any HTML tags for security
      const sanitizeText = (text: string) => text.replace(/<[^>]*>/g, '');

      const notification = new Notification({
        title: sanitizeText(data.title),
        body: sanitizeText(data.body),
      });

      // Add click handler to focus the window
      notification.on('click', () => {
        const window = BrowserWindow.fromWebContents(event.sender);
        if (window) {
          if (window.isMinimized()) {
            window.restore();
          }
          window.show();
          window.focus();
        }
      });

      notification.show();
    } catch (error) {
      console.error('Error showing notification:', error);
    }
  });

  targetIpcMain.on('logInfo', (_event, info) => {
    try {
      // Validate log info
      if (info === undefined || info === null) {
        console.error('Invalid log info: undefined or null');
        return;
      }

      // Convert to string if not already
      const logMessage = String(info);

      // Limit log message length
      const MAX_LENGTH = 10000; // 10KB limit
      if (logMessage.length > MAX_LENGTH) {
        console.error('Log message too long');
        return;
      }

      // Log the sanitized message
      log.info('from renderer:', logMessage);
    } catch (error) {
      console.error('Error logging info:', error);
    }
  });

  targetIpcMain.on(desktopCommandChannels.broadcastThemeChange, (event, themeData) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    const allWindows = BrowserWindow.getAllWindows();

    allWindows.forEach((window) => {
      if (window.id !== senderWindow?.id) {
        window.webContents.send(rendererEventChannels.themeChanged, themeData);
      }
    });
  });

  targetIpcMain.on(desktopCommandChannels.broadcastWorkspaceChange, (event) => {
    const senderWindow = BrowserWindow.fromWebContents(event.sender);
    BrowserWindow.getAllWindows().forEach((window) => {
      if (window.id !== senderWindow?.id) {
        window.webContents.send(rendererEventChannels.workspacesChanged);
      }
    });
  });

  targetIpcMain.on('reload-app', (event) => {
    // Get the window that sent the event
    const window = BrowserWindow.fromWebContents(event.sender);
    if (window) {
      window.reload();
    }
  });

  targetIpcMain.on('open-in-chrome', async (_event, url: unknown) => {
    try {
      const webUrl = normalizeWebUrl(url);
      if (!webUrl) {
        console.error('Invalid URL protocol. Only HTTP and HTTPS are allowed.');
        return;
      }

      await shell.openExternal(webUrl);
    } catch (error) {
      console.error('Error opening URL in browser:', error);
    }
  });

  // Handle app restart
  targetIpcMain.on('restart-app', () => {
    app.relaunch();
    app.quit();
  });

  // Handler for getting app version
  targetIpcMain.on(desktopCommandChannels.getAppVersion, (event) => {
    event.returnValue = app.getVersion();
  });

  targetIpcMain.on(desktopCommandChannels.getAppLocale, (event) => {
    event.returnValue = getConfiguredGoslingLocale();
  });

  targetIpcMain.handle('open-directory-in-explorer', async (event, directoryPath: string) => {
    try {
      const resolvedPath = await assertRendererFileAccess(event.sender.id, directoryPath);
      const errorMessage = await shell.openPath(resolvedPath);
      return errorMessage === '';
    } catch (error) {
      console.error('Error opening directory in explorer:', error);
      return false;
    }
  });
}
