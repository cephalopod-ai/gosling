// Owns Desktop chrome, notification, wakelock, spellcheck, and focus IPC handlers.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports registerSystemIpcHandlers; it re-exports none.

import type { App, IpcMain } from 'electron';
import { BrowserWindow } from 'electron';
import { execFileSync, spawn } from 'child_process';
import { desktopCommandChannels } from '../ipc/channels';
import type { Settings } from '../utils/settings';

export interface SystemIpcDependencies {
  app: App;
  getSettings: () => Settings;
  updateSettings: (modifier: (settings: Settings) => void) => void;
  createTray: () => void;
  destroyTray: () => void;
  focusWindow: () => void;
  activeWakelockSessionsByWindow: Map<number, Set<string>>;
  syncWindowPowerSaveBlocker: (windowId: number) => void;
  setSessionRecoveryActive: (
    windowId: number,
    sessionId: string,
    workingDir: string,
    active: boolean
  ) => boolean;
}

export const SYSTEM_IPC_CHANNELS = [
  desktopCommandChannels.setMenuBarIcon,
  desktopCommandChannels.getMenuBarIconState,
  desktopCommandChannels.setDockIcon,
  desktopCommandChannels.getDockIconState,
  desktopCommandChannels.openNotificationsSettings,
  desktopCommandChannels.setWakelock,
  desktopCommandChannels.getWakelockState,
  desktopCommandChannels.setWakelockActive,
  desktopCommandChannels.setSessionRecoveryActive,
  desktopCommandChannels.setSpellcheck,
  desktopCommandChannels.getSpellcheckState,
  desktopCommandChannels.isAnyWindowFocused,
  desktopCommandChannels.getIsFullScreen,
] as const;

export function registerSystemIpcHandlers(
  targetIpcMain: Pick<IpcMain, 'handle'>,
  dependencies: SystemIpcDependencies
): void {
  const {
    app,
    getSettings,
    updateSettings,
    createTray,
    destroyTray,
    focusWindow,
    activeWakelockSessionsByWindow,
    syncWindowPowerSaveBlocker,
    setSessionRecoveryActive,
  } = dependencies;

  // Handle menu bar icon visibility
  targetIpcMain.handle(desktopCommandChannels.setMenuBarIcon, async (_event, show: boolean) => {
    updateSettings((settings) => {
      settings.showMenuBarIcon = show;
    });
    if (show) createTray();
    else destroyTray();
    return true;
  });
  targetIpcMain.handle(desktopCommandChannels.getMenuBarIconState, () => {
    try {
      return getSettings().showMenuBarIcon ?? true;
    } catch (error) {
      console.error('Error getting menu bar icon state:', error);
      return true;
    }
  });

  // Handle dock icon visibility (macOS only)
  targetIpcMain.handle(desktopCommandChannels.setDockIcon, async (_event, show: boolean) => {
    if (process.platform !== 'darwin') return false;
    const settings = getSettings();
    updateSettings((nextSettings) => {
      nextSettings.showDockIcon = show;
    });
    if (show) {
      app.dock?.show();
    } else if (settings.showMenuBarIcon) {
      // Only hide the dock if we have a menu bar icon to maintain accessibility
      app.dock?.hide();
      setTimeout(() => focusWindow(), 50);
    }
    return true;
  });
  targetIpcMain.handle(desktopCommandChannels.getDockIconState, () => {
    try {
      if (process.platform !== 'darwin') return true;
      return getSettings().showDockIcon ?? true;
    } catch (error) {
      console.error('Error getting dock icon state:', error);
      return true;
    }
  });

  // Handle opening system notifications preferences
  targetIpcMain.handle(desktopCommandChannels.openNotificationsSettings, async () => {
    try {
      if (process.platform === 'darwin') {
        spawn('open', ['x-apple.systempreferences:com.apple.preference.notifications']);
        return true;
      } else if (process.platform === 'win32') {
        // Windows: Open notification settings in Settings app
        spawn('ms-settings:notifications', { shell: true });
        return true;
      } else if (process.platform === 'linux') {
        // Linux: Try different desktop environments
        function canSpawn(cmd: string): boolean {
          try {
            execFileSync('which', [cmd], { stdio: 'ignore' });
            return true;
          } catch {
            return false;
          }
        }
        // GNOME
        if (canSpawn('gnome-control-center')) {
          spawn('gnome-control-center', ['notifications']);
          return true;
        }
        // KDE Plasma
        if (canSpawn('systemsettings5')) {
          spawn('systemsettings5', ['kcm_notifications']);
          return true;
        }
        // XFCE
        if (canSpawn('xfce4-settings-manager')) {
          spawn('xfce4-settings-manager', ['--socket-id=notifications']);
          return true;
        }
        console.warn('Could not find a suitable settings application for Linux');
        return false;
      } else {
        console.warn(
          `Opening notification settings is not supported on platform: ${process.platform}`
        );
        return false;
      }
    } catch (error) {
      console.error('Error opening notification settings:', error);
      return false;
    }
  });

  // Handle wakelock setting
  targetIpcMain.handle(desktopCommandChannels.setWakelock, async (_event, enable: boolean) => {
    updateSettings((settings) => {
      settings.enableWakelock = enable;
    });
    for (const windowId of activeWakelockSessionsByWindow.keys()) {
      syncWindowPowerSaveBlocker(windowId);
    }
    return true;
  });
  targetIpcMain.handle(desktopCommandChannels.getWakelockState, () => {
    try {
      return getSettings().enableWakelock ?? false;
    } catch (error) {
      console.error('Error getting wakelock state:', error);
      return false;
    }
  });
  targetIpcMain.handle(
    desktopCommandChannels.setWakelockActive,
    (event, sessionId: string, active: boolean): boolean => {
      const windowId = BrowserWindow.fromWebContents(event.sender)?.id;
      if (!windowId || !sessionId.trim()) return false;
      const activeSessions = activeWakelockSessionsByWindow.get(windowId) ?? new Set<string>();
      if (active) {
        activeSessions.add(sessionId);
        activeWakelockSessionsByWindow.set(windowId, activeSessions);
      } else {
        activeSessions.delete(sessionId);
        if (activeSessions.size === 0) activeWakelockSessionsByWindow.delete(windowId);
      }
      syncWindowPowerSaveBlocker(windowId);
      return true;
    }
  );
  targetIpcMain.handle(
    desktopCommandChannels.setSessionRecoveryActive,
    (event, sessionId: unknown, workingDir: unknown, active: unknown): boolean => {
      const windowId = BrowserWindow.fromWebContents(event.sender)?.id;
      if (
        !windowId ||
        typeof sessionId !== 'string' ||
        typeof workingDir !== 'string' ||
        typeof active !== 'boolean'
      ) {
        return false;
      }
      return setSessionRecoveryActive(windowId, sessionId, workingDir, active);
    }
  );
  targetIpcMain.handle(desktopCommandChannels.setSpellcheck, async (_event, enable: boolean) => {
    updateSettings((settings) => {
      settings.spellcheckEnabled = enable;
    });
    return true;
  });
  targetIpcMain.handle(desktopCommandChannels.getSpellcheckState, () => {
    try {
      return getSettings().spellcheckEnabled ?? true;
    } catch (error) {
      console.error('Error getting spellcheck state:', error);
      return true;
    }
  });
  targetIpcMain.handle(
    desktopCommandChannels.isAnyWindowFocused,
    () => BrowserWindow.getFocusedWindow() !== null
  );
  targetIpcMain.handle(
    desktopCommandChannels.getIsFullScreen,
    (event) => BrowserWindow.fromWebContents(event.sender)?.isFullScreen() ?? false
  );
}
