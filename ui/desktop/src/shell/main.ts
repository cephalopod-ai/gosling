import { app, BrowserWindow, dialog, ipcMain, shell } from 'electron';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { bootstrapShell } from './bootstrap';
import type { ShellResourceFiles } from './resources';

declare const __GOSLING_SHELL_RESOURCE_FILES__: ShellResourceFiles;
declare const SHELL_WINDOW_VITE_DEV_SERVER_URL: string;
declare const SHELL_WINDOW_VITE_NAME: string;

const playwrightPathRoot = process.env.GOSLING_PLAYWRIGHT_USER_DATA_DIR?.trim();
if (process.env.ENABLE_PLAYWRIGHT === 'true' && playwrightPathRoot) {
  const root = path.resolve(playwrightPathRoot);
  app.setPath('appData', path.join(root, 'app-data'));
  app.setPath('sessionData', path.join(root, 'session-data'));
  app.setPath('temp', path.join(root, 'temp'));
}

function rendererUrl(): string {
  return SHELL_WINDOW_VITE_DEV_SERVER_URL
    ? new URL('shell.html', SHELL_WINDOW_VITE_DEV_SERVER_URL).toString()
    : pathToFileURL(
        path.join(__dirname, `../renderer/${SHELL_WINDOW_VITE_NAME}/shell.html`)
      ).toString();
}

void bootstrapShell({
  app,
  ipcMain,
  createWindow: (options) => new BrowserWindow(options),
  showSaveDialog: (options) => dialog.showSaveDialog(options),
  showOpenDialog: ({ filters, ...options }) =>
    dialog.showOpenDialog({
      ...options,
      properties: [...options.properties],
      ...(filters ? { filters: filters.map((filter) => ({ ...filter })) } : {}),
    }),
  showConfirmDialog: async (options) => {
    const result = await dialog.showMessageBox({
      type: 'warning',
      title: options.title,
      message: options.message,
      detail: options.detail,
      buttons: [options.cancelLabel, options.confirmLabel],
      defaultId: 0,
      cancelId: 0,
    });
    return { confirmed: result.response === 1 };
  },
  openExternal: (url) => shell.openExternal(url),
  resourcesPath: process.resourcesPath,
  preloadPath: path.join(__dirname, 'shell-preload.js'),
  rendererUrl: rendererUrl(),
  workingDir: process.cwd(),
  shellResources: __GOSLING_SHELL_RESOURCE_FILES__,
}).catch(async (error: unknown) => {
  const message = error instanceof Error ? error.message : 'Unknown shell startup failure';
  await app.whenReady();
  dialog.showErrorBox('Shell failed to start', message.slice(0, 2048));
  app.exit(1);
});
