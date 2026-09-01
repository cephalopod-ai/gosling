// Owns the quick launcher, tray, window reveal, recent-directory menu, and directory chooser.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports createWindowChrome; it re-exports none.

import type { App, BrowserWindow as BrowserWindowType, OpenDialogReturnValue } from 'electron';
import { BrowserWindow, dialog, screen, Tray } from 'electron';
import fsSync from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { format as formatUrl } from 'node:url';
import type logger from '../utils/logger';
import { addRecentDir, loadRecentDirs } from '../utils/recentDirs';
import type { RendererDirectoryGrantRegistry } from '../utils/rendererDirectoryGrants';
import type { Settings } from '../utils/settings';
import { getUpdateAvailable, setTrayRef, updateTrayMenu } from '../utils/autoUpdater';

interface LauncherConfig {
  GOSLING_LOCALE?: string;
  [key: string]: unknown;
}

interface CreateChatOptions {
  dir?: string;
}

export interface WindowChromeDependencies {
  app: App;
  appConfig: LauncherConfig;
  getConfiguredGoslingLocale: () => string | undefined;
  getAppUrl: () => URL;
  reactReadyWindows: Set<number>;
  updateSettings: (modifier: (settings: Settings) => void) => void;
  createChat: (app: App, options: CreateChatOptions) => Promise<BrowserWindowType | undefined>;
  firstGrantedRecentDirectory: () => string | undefined;
  rendererDirectoryGrants: RendererDirectoryGrantRegistry;
  log: Pick<typeof logger, 'info'>;
}

export interface WindowChrome {
  createLauncher: () => BrowserWindowType;
  destroyTray: () => void;
  createTray: () => void;
  buildRecentFilesMenu: () => Array<{ label: string; click: () => Promise<void> }>;
  openDirectoryDialog: () => Promise<OpenDialogReturnValue>;
  hasTray: () => boolean;
}

export function createWindowChrome(dependencies: WindowChromeDependencies): WindowChrome {
  const {
    app,
    appConfig,
    getConfiguredGoslingLocale,
    getAppUrl,
    reactReadyWindows,
    updateSettings,
    createChat,
    firstGrantedRecentDirectory,
    rendererDirectoryGrants,
    log,
  } = dependencies;

  let activeLauncherWindow: BrowserWindowType | null = null;
  let tray: Tray | null = null;

  function createLauncher(): BrowserWindowType {
    if (activeLauncherWindow && !activeLauncherWindow.isDestroyed()) {
      activeLauncherWindow.focus();
      return activeLauncherWindow;
    }

    const launcherWindow = new BrowserWindow({
      width: 600,
      height: 80,
      frame: false,
      transparent: process.platform === 'darwin',
      backgroundColor: process.platform === 'darwin' ? '#00000000' : '#ffffff',
      webPreferences: {
        preload: path.join(__dirname, 'preload.js'),
        nodeIntegration: false,
        contextIsolation: true,
        // See the main window above. (SECN-GSL-003)
        sandbox: true,
        additionalArguments: [
          JSON.stringify({
            ...appConfig,
            GOSLING_LOCALE: getConfiguredGoslingLocale(),
          }),
        ],
        partition: 'persist:gosling',
      },
      skipTaskbar: true,
      alwaysOnTop: true,
      resizable: false,
      movable: true,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      hasShadow: true,
      vibrancy: process.platform === 'darwin' ? 'window' : undefined,
    });

    // Center on screen
    const primaryDisplay = screen.getPrimaryDisplay();
    const { width, height } = primaryDisplay.workAreaSize;
    const windowBounds = launcherWindow.getBounds();

    launcherWindow.setPosition(
      Math.round(width / 2 - windowBounds.width / 2),
      Math.round(height / 3 - windowBounds.height / 2)
    );

    // Load launcher window content
    const url = getAppUrl();

    url.hash = '/launcher';
    launcherWindow.loadURL(formatUrl(url));
    activeLauncherWindow = launcherWindow;

    launcherWindow.on('closed', () => {
      reactReadyWindows.delete(launcherWindow.id);
      activeLauncherWindow = null;
    });

    // Destroy window when it loses focus
    launcherWindow.on('blur', () => {
      launcherWindow.destroy();
    });

    // Also destroy on escape key
    launcherWindow.webContents.on('before-input-event', (event, input) => {
      if (input.key === 'Escape') {
        launcherWindow.destroy();
        event.preventDefault();
      }
    });

    return launcherWindow;
  }

  function destroyTray(): void {
    if (tray) {
      tray.destroy();
      tray = null;
    }
  }

  function disableTray(): void {
    updateSettings((settings) => {
      settings.showMenuBarIcon = false;
    });
  }

  function createTray(): void {
    destroyTray();

    const possiblePaths = [
      path.join(process.resourcesPath, 'images', 'iconTemplate.png'),
      path.join(process.cwd(), 'src', 'images', 'iconTemplate.png'),
      path.join(__dirname, '..', 'images', 'iconTemplate.png'),
      path.join(__dirname, 'images', 'iconTemplate.png'),
      path.join(process.cwd(), 'images', 'iconTemplate.png'),
    ];

    const iconPath = possiblePaths.find((possiblePath) => fsSync.existsSync(possiblePath));

    if (!iconPath) {
      console.warn('[Main] Tray icon not found. App will continue without system tray.');
      disableTray();
      return;
    }

    try {
      tray = new Tray(iconPath);
      setTrayRef(tray);
      updateTrayMenu(getUpdateAvailable());

      if (process.platform === 'win32') {
        tray.on('click', showWindow);
      }
    } catch (error) {
      console.error('[Main] Tray creation failed. App will continue without system tray.', error);
      disableTray();
      tray = null;
    }
  }

  async function showWindow(): Promise<void> {
    const windows = BrowserWindow.getAllWindows();

    if (windows.length === 0) {
      log.info('No windows are open, creating a new one...');
      await createChat(app, { dir: firstGrantedRecentDirectory() });
      return;
    }

    const initialOffsetX = 30;
    const initialOffsetY = 30;

    // Iterate over all windows
    windows.forEach((window, index) => {
      const currentBounds = window.getBounds();
      const newX = currentBounds.x + initialOffsetX * index;
      const newY = currentBounds.y + initialOffsetY * index;

      window.setBounds({
        x: newX,
        y: newY,
        width: currentBounds.width,
        height: currentBounds.height,
      });

      if (!window.isVisible()) {
        window.show();
      }

      window.focus();
    });
  }

  function buildRecentFilesMenu(): Array<{ label: string; click: () => Promise<void> }> {
    const recentDirs = loadRecentDirs().filter((directory) =>
      rendererDirectoryGrants.isGrantedDirectory(0, directory)
    );
    return recentDirs.map((directory) => ({
      label: directory,
      click: async () => {
        await createChat(app, { dir: directory });
      },
    }));
  }

  async function openDirectoryDialog(): Promise<OpenDialogReturnValue> {
    // Get the current working directory from the focused window
    let defaultPath: string | undefined;
    const currentWindow = BrowserWindow.getFocusedWindow();

    if (currentWindow) {
      try {
        const currentWorkingDir = await currentWindow.webContents.executeJavaScript(
          `window.appConfig ? window.appConfig.get('GOSLING_WORKING_DIR') : null`
        );

        if (currentWorkingDir && typeof currentWorkingDir === 'string') {
          // Verify the directory exists before using it as default
          try {
            const stats = fsSync.lstatSync(currentWorkingDir);
            if (stats.isDirectory()) {
              defaultPath = currentWorkingDir;
            }
          } catch (error) {
            if (error && typeof error === 'object' && 'code' in error) {
              const fsError = error as { code?: string; message?: string };
              if (
                fsError.code === 'ENOENT' ||
                fsError.code === 'EACCES' ||
                fsError.code === 'EPERM'
              ) {
                console.warn(
                  `Current working directory not accessible (${fsError.code}): ${currentWorkingDir}, falling back to home directory`
                );
                defaultPath = os.homedir();
              } else {
                console.warn(
                  `Unexpected filesystem error (${fsError.code}) for directory ${currentWorkingDir}:`,
                  fsError.message
                );
                defaultPath = os.homedir();
              }
            } else {
              console.warn(`Unexpected error checking directory ${currentWorkingDir}:`, error);
              defaultPath = os.homedir();
            }
          }
        }
      } catch (error) {
        console.warn('Failed to get current working directory from window:', error);
      }
    }

    if (!defaultPath) {
      defaultPath = os.homedir();
    }

    const result = (await dialog.showOpenDialog({
      properties: ['openFile', 'openDirectory', 'createDirectory'],
      defaultPath,
    })) as unknown as OpenDialogReturnValue;

    if (!result.canceled && result.filePaths.length > 0) {
      const selectedPath = result.filePaths[0];

      // If a file was selected, use its parent directory
      let dirToAdd = selectedPath;
      try {
        const stats = fsSync.lstatSync(selectedPath);

        // Reject symlinks for security
        if (stats.isSymbolicLink()) {
          console.warn(`Selected path is a symlink, using parent directory for security`);
          dirToAdd = path.dirname(selectedPath);
        } else if (stats.isFile()) {
          dirToAdd = path.dirname(selectedPath);
        }
      } catch {
        console.warn(`Could not stat selected path, using parent directory`);
        dirToAdd = path.dirname(selectedPath); // Fallback to parent directory
      }

      addRecentDir(dirToAdd);
      rendererDirectoryGrants.grantSelectedPath(currentWindow?.webContents.id ?? 0, dirToAdd);

      await createChat(app, { dir: dirToAdd });
    }
    return result;
  }

  return {
    createLauncher,
    destroyTray,
    createTray,
    buildRecentFilesMenu,
    openDirectoryDialog,
    hasTray: () => tray !== null,
  };
}
