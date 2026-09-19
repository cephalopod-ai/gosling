import Electron, { contextBridge, ipcRenderer, webUtils } from 'electron';
import { desktopCommandChannels, rendererEventChannels } from './ipc/channels';
import type {
  AcpEndpoint,
  CreateChatWindowOptions,
  DesktopCommandPayloads,
  DesktopInvokeChannel,
  DesktopSendChannel,
  DesktopSyncChannel,
  McpAppProxyCsp,
  MessageBoxOptions,
  NotificationData,
  RendererEventCallback,
  RendererEventChannel,
  ThemeChangePayload,
  UpdaterEvent,
} from './ipc/channels';
import type { Settings, SettingKey } from './utils/settings';
import { defaultSettings } from './utils/settings';
import type {
  ArtifactRoutingConfig,
  ArtifactSaveRequest,
  ArtifactSaveResponse,
} from './types/artifactRouter';
import type { ResearchLibraryImportResult, ResearchLibraryListing } from './utils/researchLibrary';
import type { ArtifactTrashResult } from './types/artifactTrash';
import type { ArtifactRepositoryClassification } from './utils/artifactRepository';
import type { ArtifactFileTimestampMap } from './types/artifactFileTimestamps';

// Mapping from settings keys to their old localStorage keys for lazy migration
const localStorageKeyMap: Partial<Record<SettingKey, string>> = {
  theme: 'theme',
  useSystemTheme: 'use_system_theme',
  responseStyle: 'response_style',
  showPricing: 'show_pricing',
  seenAnnouncementIds: 'seenAnnouncementIds',
};

// Parse localStorage value based on the setting key
function parseLocalStorageValue<K extends SettingKey>(
  key: K,
  rawValue: string
): Settings[K] | null {
  try {
    switch (key) {
      case 'theme':
        return (rawValue === 'dark' || rawValue === 'light' ? rawValue : null) as Settings[K];
      case 'useSystemTheme':
        return (rawValue === 'true') as unknown as Settings[K];
      case 'responseStyle':
        return rawValue as Settings[K];
      case 'showPricing':
        return (rawValue === 'true') as unknown as Settings[K];
      case 'seenAnnouncementIds':
        return JSON.parse(rawValue) as Settings[K];
      default:
        return null;
    }
  } catch {
    return null;
  }
}

interface MessageBoxResponse {
  response: number;
  checkboxChecked?: boolean;
}

interface FileResponse {
  file: string;
  filePath: string;
  error: string | null;
  found: boolean;
}

export interface ArtifactFileResponse {
  content: string;
  encoding: 'base64' | 'utf8';
  error: string | null;
  filePath: string;
  found: boolean;
  sizeBytes: number;
  truncated: boolean;
}

const config = JSON.parse(process.argv.find((arg) => arg.startsWith('{')) || '{}');

function sendToMain<T extends DesktopSendChannel>(channel: T, ...args: DesktopCommandPayloads[T]) {
  ipcRenderer.send(channel, ...args);
}

function sendSyncToMain<T extends DesktopSyncChannel>(
  channel: T,
  ...args: DesktopCommandPayloads[T]
) {
  return ipcRenderer.sendSync(channel, ...args);
}

function invokeMain<T extends DesktopInvokeChannel>(
  channel: T,
  ...args: DesktopCommandPayloads[T]
) {
  return ipcRenderer.invoke(channel, ...args);
}

// Define the API types in a single place
type ElectronAPI = {
  platform: string;
  arch: string;
  reactReady: () => void;
  getConfig: () => Record<string, unknown>;
  directoryChooser: () => Promise<Electron.OpenDialogReturnValue>;
  sessionDirectoryChooser: () => Promise<Electron.OpenDialogReturnValue>;
  getResearchLibraryPath: () => Promise<string>;
  chooseResearchLibraryPath: () => Promise<string | null>;
  listResearchLibraryFiles: () => Promise<ResearchLibraryListing>;
  importResearchLibraryFiles: () => Promise<ResearchLibraryImportResult>;
  grantSessionDirectories: (directories: string[]) => Promise<string[]>;
  createChatWindow: (options?: CreateChatWindowOptions) => void;
  logInfo: (txt: string) => void;
  showNotification: (data: NotificationData) => void;
  showMessageBox: (options: MessageBoxOptions) => Promise<MessageBoxResponse>;
  saveArtifact: (request: ArtifactSaveRequest) => Promise<ArtifactSaveResponse>;
  setArtifactRoutingConfig: (config: ArtifactRoutingConfig | null) => Promise<boolean>;
  openInChrome: (url: string) => void;
  reloadApp: () => void;
  checkForOllama: () => Promise<boolean>;
  selectFileOrDirectory: (defaultPath?: string) => Promise<string | null>;
  selectArtifactFile: (defaultPath?: string) => Promise<string | null>;
  selectImportSessionFile: () => Promise<{
    filePath: string;
    contents: string;
    error?: string;
  } | null>;
  readFile: (directory: string) => Promise<FileResponse>;
  readArtifactFile: (filePath: string, baseDirectory?: string) => Promise<ArtifactFileResponse>;
  copyArtifactContents: (filePath: string, baseDirectory?: string) => Promise<void>;
  readArtifactTitles: (
    requests: Array<{ filePath: string; baseDirectory?: string }>
  ) => Promise<Record<string, string>>;
  classifyArtifactRepositories: (filePaths: string[]) => Promise<ArtifactRepositoryClassification>;
  getArtifactFileTimestamps: (filePaths: string[]) => Promise<ArtifactFileTimestampMap>;
  openArtifactFile: (filePath: string, baseDirectory?: string) => Promise<boolean>;
  revealArtifactFile: (filePath: string, baseDirectory?: string) => Promise<void>;
  writeFile: (directory: string, content: string) => Promise<boolean>;
  deleteFile: (filePath: string) => Promise<boolean>;
  trashArtifactFiles: (paths: string[]) => Promise<ArtifactTrashResult[]>;
  ensureDirectory: (dirPath: string) => Promise<boolean>;
  listFiles: (dirPath: string, extension?: string) => Promise<string[]>;
  getAllowedExtensions: () => Promise<string[]>;
  getPathForFile: (file: File) => string;
  setMenuBarIcon: (show: boolean) => Promise<boolean>;
  getMenuBarIconState: () => Promise<boolean>;
  setDockIcon: (show: boolean) => Promise<boolean>;
  getDockIconState: () => Promise<boolean>;
  getSetting: <K extends SettingKey>(key: K) => Promise<Settings[K]>;
  getSettings: <K extends SettingKey>(keys: K[]) => Promise<Pick<Settings, K>>;
  setSetting: <K extends SettingKey>(key: K, value: Settings[K]) => Promise<void>;
  getAcpUrl: () => Promise<AcpEndpoint | null>;
  getMcpAppProxyUrl: (csp?: McpAppProxyCsp | null) => Promise<string | null>;
  setWakelock: (enable: boolean) => Promise<boolean>;
  getWakelockState: () => Promise<boolean>;
  setWakelockActive: (sessionId: string, active: boolean) => Promise<boolean>;
  setSessionRecoveryActive: (
    sessionId: string,
    workingDir: string,
    active: boolean
  ) => Promise<boolean>;
  setSpellcheck: (enable: boolean) => Promise<boolean>;
  getSpellcheckState: () => Promise<boolean>;
  openNotificationsSettings: () => Promise<boolean>;
  isAnyWindowFocused: () => Promise<boolean>;
  getIsFullScreen: () => Promise<boolean>;
  onMouseBackButtonClicked: (callback: () => void) => void;
  offMouseBackButtonClicked: (callback: () => void) => void;
  on: <T extends RendererEventChannel>(channel: T, callback: RendererEventCallback<T>) => void;
  off: <T extends RendererEventChannel>(channel: T, callback: RendererEventCallback<T>) => void;
  broadcastThemeChange: (themeData: ThemeChangePayload) => void;
  broadcastWorkspaceChange: () => void;
  openExternal: (url: string) => Promise<void>;
  // Update-related functions
  getVersion: () => string;
  checkForUpdates: () => Promise<{ updateInfo: unknown; error: string | null }>;
  downloadUpdate: () => Promise<{ success: boolean; error: string | null }>;
  installUpdate: () => void;
  restartApp: () => void;
  onUpdaterEvent: (callback: (event: UpdaterEvent) => void) => () => void;
  getUpdateState: () => Promise<{ updateAvailable: boolean; latestVersion?: string } | null>;
  isUsingGitHubFallback: () => Promise<boolean>;
  getAutoDownloadDisabled: () => Promise<boolean>;
  closeWindow: () => void;
  openDirectoryInExplorer: (directoryPath: string) => Promise<boolean>;
  addRecentDir: (dir: string) => Promise<boolean>;
  listRecentDirs: () => Promise<string[]>;
  listGitWorktreeDirs: (dir: string) => Promise<string[]>;
  getGitBranchInfo: (dir: string) => Promise<{ branch: string } | null>;
  listGitBranches: (dir: string) => Promise<string[]>;
  switchGitBranch: (dir: string, branch: string) => Promise<{ success: boolean }>;
  writeClipboardText: (text: string) => Promise<void>;
  writeClipboardHtml: (html: string, text: string) => Promise<void>;
};

type AppConfigAPI = {
  get: (key: string) => unknown;
  getAll: () => Record<string, unknown>;
};

const mouseBackButtonListeners = new WeakMap<() => void, () => void>();

const electronAPI: ElectronAPI = {
  platform: process.platform,
  arch: process.arch,
  reactReady: () => sendToMain(desktopCommandChannels.reactReady),
  getConfig: () => {
    if (!config || Object.keys(config).length === 0) {
      console.warn(
        'No config provided by main process. This may indicate an initialization issue.'
      );
    }
    return config;
  },
  directoryChooser: () => invokeMain(desktopCommandChannels.directoryChooser),
  sessionDirectoryChooser: () => invokeMain(desktopCommandChannels.sessionDirectoryChooser),
  getResearchLibraryPath: () => invokeMain(desktopCommandChannels.getResearchLibraryPath),
  chooseResearchLibraryPath: () => invokeMain(desktopCommandChannels.chooseResearchLibraryPath),
  listResearchLibraryFiles: () => invokeMain(desktopCommandChannels.listResearchLibraryFiles),
  importResearchLibraryFiles: () => invokeMain(desktopCommandChannels.importResearchLibraryFiles),
  grantSessionDirectories: (directories: string[]) =>
    invokeMain(desktopCommandChannels.grantSessionDirectories, directories),
  createChatWindow: (options?: CreateChatWindowOptions) =>
    sendToMain(desktopCommandChannels.createChatWindow, options || {}),
  logInfo: (txt: string) => sendToMain(desktopCommandChannels.logInfo, txt),
  showNotification: (data: NotificationData) =>
    sendToMain(desktopCommandChannels.showNotification, data),
  showMessageBox: (options: MessageBoxOptions) =>
    invokeMain(desktopCommandChannels.showMessageBox, options),
  saveArtifact: (request: ArtifactSaveRequest) =>
    invokeMain(desktopCommandChannels.saveArtifact, request),
  setArtifactRoutingConfig: (routingConfig: ArtifactRoutingConfig | null) =>
    invokeMain(desktopCommandChannels.setArtifactRoutingConfig, routingConfig),
  openInChrome: (url: string) => sendToMain(desktopCommandChannels.openInChrome, url),
  reloadApp: () => sendToMain(desktopCommandChannels.reloadApp),
  checkForOllama: () => invokeMain(desktopCommandChannels.checkForOllama),

  selectFileOrDirectory: (defaultPath?: string) =>
    invokeMain(desktopCommandChannels.selectFileOrDirectory, defaultPath),
  selectArtifactFile: (defaultPath?: string) =>
    invokeMain(desktopCommandChannels.selectArtifactFile, defaultPath),
  selectImportSessionFile: () => invokeMain(desktopCommandChannels.selectImportSessionFile),
  readFile: (filePath: string) => invokeMain(desktopCommandChannels.readFile, filePath),
  readArtifactFile: (filePath: string, baseDirectory?: string) =>
    invokeMain(desktopCommandChannels.readArtifactFile, filePath, baseDirectory),
  copyArtifactContents: (filePath: string, baseDirectory?: string) =>
    invokeMain(desktopCommandChannels.copyArtifactContents, filePath, baseDirectory),
  readArtifactTitles: (requests: Array<{ filePath: string; baseDirectory?: string }>) =>
    invokeMain(desktopCommandChannels.readArtifactTitles, requests),
  classifyArtifactRepositories: (filePaths: string[]) =>
    invokeMain(desktopCommandChannels.classifyArtifactRepositories, filePaths),
  getArtifactFileTimestamps: (filePaths: string[]) =>
    invokeMain(desktopCommandChannels.getArtifactFileTimestamps, filePaths),
  openArtifactFile: (filePath: string, baseDirectory?: string) =>
    invokeMain(desktopCommandChannels.openArtifactFile, filePath, baseDirectory),
  revealArtifactFile: (filePath: string, baseDirectory?: string) =>
    invokeMain(desktopCommandChannels.revealArtifactFile, filePath, baseDirectory),
  writeFile: (filePath: string, content: string) =>
    invokeMain(desktopCommandChannels.writeFile, filePath, content),
  deleteFile: (filePath: string) => invokeMain(desktopCommandChannels.deleteFile, filePath),
  trashArtifactFiles: (paths: string[]) =>
    invokeMain(desktopCommandChannels.trashArtifactFiles, paths),
  ensureDirectory: (dirPath: string) => invokeMain(desktopCommandChannels.ensureDirectory, dirPath),
  listFiles: (dirPath: string, extension?: string) =>
    invokeMain(desktopCommandChannels.listFiles, dirPath, extension),
  getPathForFile: (file: File) => webUtils.getPathForFile(file),
  getAllowedExtensions: () => invokeMain(desktopCommandChannels.getAllowedExtensions),
  setMenuBarIcon: (show: boolean) => invokeMain(desktopCommandChannels.setMenuBarIcon, show),
  getMenuBarIconState: () => invokeMain(desktopCommandChannels.getMenuBarIconState),
  setDockIcon: (show: boolean) => invokeMain(desktopCommandChannels.setDockIcon, show),
  getDockIconState: () => invokeMain(desktopCommandChannels.getDockIconState),
  getSetting: async <K extends SettingKey>(key: K): Promise<Settings[K]> => {
    try {
      // Check for localStorage value first (lazy migration)
      const localStorageKey = localStorageKeyMap[key];
      if (localStorageKey) {
        const rawValue = localStorage.getItem(localStorageKey);
        if (rawValue !== null) {
          const parsed = parseLocalStorageValue(key, rawValue);
          if (parsed !== null) {
            return parsed;
          }
        }
      }
      return await invokeMain(desktopCommandChannels.getSetting, key);
    } catch (error) {
      console.error(`Failed to get setting '${key}', using default`, error);
      return defaultSettings[key];
    }
  },
  getSettings: async <K extends SettingKey>(keys: K[]): Promise<Pick<Settings, K>> => {
    const values: Partial<Pick<Settings, K>> = {};
    const ipcKeys: K[] = [];

    for (const key of keys) {
      const localStorageKey = localStorageKeyMap[key];
      if (localStorageKey) {
        const rawValue = localStorage.getItem(localStorageKey);
        if (rawValue !== null) {
          const parsed = parseLocalStorageValue(key, rawValue);
          if (parsed !== null) {
            values[key] = parsed;
            continue;
          }
        }
      }
      ipcKeys.push(key);
    }

    if (ipcKeys.length > 0) {
      try {
        Object.assign(values, await invokeMain(desktopCommandChannels.getSettings, ipcKeys));
      } catch (error) {
        console.error(`Failed to get settings '${ipcKeys.join(', ')}', using defaults`, error);
        for (const key of ipcKeys) {
          values[key] = defaultSettings[key];
        }
      }
    }

    return values as Pick<Settings, K>;
  },
  setSetting: async <K extends SettingKey>(key: K, value: Settings[K]): Promise<void> => {
    // Clear any localStorage version when writing
    const localStorageKey = localStorageKeyMap[key];
    if (localStorageKey) {
      localStorage.removeItem(localStorageKey);
    }
    return invokeMain(desktopCommandChannels.setSetting, key, value);
  },
  getAcpUrl: () => invokeMain(desktopCommandChannels.getAcpUrl),
  getMcpAppProxyUrl: (csp?: McpAppProxyCsp | null) =>
    invokeMain(desktopCommandChannels.getMcpAppProxyUrl, csp),
  setWakelock: (enable: boolean) => invokeMain(desktopCommandChannels.setWakelock, enable),
  getWakelockState: () => invokeMain(desktopCommandChannels.getWakelockState),
  setWakelockActive: (sessionId: string, active: boolean) =>
    invokeMain(desktopCommandChannels.setWakelockActive, sessionId, active),
  setSessionRecoveryActive: (sessionId: string, workingDir: string, active: boolean) =>
    invokeMain(desktopCommandChannels.setSessionRecoveryActive, sessionId, workingDir, active),
  setSpellcheck: (enable: boolean) => invokeMain(desktopCommandChannels.setSpellcheck, enable),
  getSpellcheckState: () => invokeMain(desktopCommandChannels.getSpellcheckState),
  openNotificationsSettings: () => invokeMain(desktopCommandChannels.openNotificationsSettings),
  isAnyWindowFocused: () => invokeMain(desktopCommandChannels.isAnyWindowFocused),
  getIsFullScreen: () => invokeMain(desktopCommandChannels.getIsFullScreen),
  onMouseBackButtonClicked: (callback: () => void) => {
    const wrappedCallback = () => callback();
    mouseBackButtonListeners.set(callback, wrappedCallback);
    ipcRenderer.on(rendererEventChannels.mouseBackButtonClicked, wrappedCallback);
  },
  offMouseBackButtonClicked: (callback: () => void) => {
    const wrappedCallback = mouseBackButtonListeners.get(callback);
    if (wrappedCallback) {
      ipcRenderer.removeListener(rendererEventChannels.mouseBackButtonClicked, wrappedCallback);
      mouseBackButtonListeners.delete(callback);
    }
  },
  on: <T extends RendererEventChannel>(channel: T, callback: RendererEventCallback<T>) => {
    ipcRenderer.on(channel, callback);
  },
  off: <T extends RendererEventChannel>(channel: T, callback: RendererEventCallback<T>) => {
    ipcRenderer.off(channel, callback);
  },
  broadcastThemeChange: (themeData: ThemeChangePayload) => {
    sendToMain(desktopCommandChannels.broadcastThemeChange, themeData);
  },
  broadcastWorkspaceChange: () => {
    sendToMain(desktopCommandChannels.broadcastWorkspaceChange);
  },
  openExternal: (url: string): Promise<void> => {
    return invokeMain(desktopCommandChannels.openExternal, url);
  },
  getVersion: (): string => {
    return config.GOSLING_VERSION || sendSyncToMain(desktopCommandChannels.getAppVersion) || '';
  },
  checkForUpdates: (): Promise<{ updateInfo: unknown; error: string | null }> => {
    return invokeMain(desktopCommandChannels.checkForUpdates);
  },
  downloadUpdate: (): Promise<{ success: boolean; error: string | null }> => {
    return invokeMain(desktopCommandChannels.downloadUpdate);
  },
  installUpdate: (): void => {
    void invokeMain(desktopCommandChannels.installUpdate);
  },
  restartApp: (): void => {
    sendToMain(desktopCommandChannels.restartApp);
  },
  onUpdaterEvent: (callback: (event: UpdaterEvent) => void): (() => void) => {
    const handler = (_event: Electron.IpcRendererEvent, data: UpdaterEvent) => callback(data);
    ipcRenderer.on(rendererEventChannels.updaterEvent, handler);
    return () => ipcRenderer.removeListener(rendererEventChannels.updaterEvent, handler);
  },
  getUpdateState: (): Promise<{ updateAvailable: boolean; latestVersion?: string } | null> => {
    return invokeMain(desktopCommandChannels.getUpdateState);
  },
  isUsingGitHubFallback: (): Promise<boolean> => {
    return invokeMain(desktopCommandChannels.isUsingGitHubFallback);
  },
  getAutoDownloadDisabled: (): Promise<boolean> => {
    return invokeMain(desktopCommandChannels.getAutoDownloadDisabled);
  },
  closeWindow: () => sendToMain(desktopCommandChannels.closeWindow),
  openDirectoryInExplorer: (directoryPath: string) =>
    invokeMain(desktopCommandChannels.openDirectoryInExplorer, directoryPath),
  addRecentDir: (dir: string) => invokeMain(desktopCommandChannels.addRecentDir, dir),
  listRecentDirs: () => invokeMain(desktopCommandChannels.listRecentDirs),
  listGitWorktreeDirs: (dir: string) => invokeMain(desktopCommandChannels.listGitWorktreeDirs, dir),
  getGitBranchInfo: (dir: string) => invokeMain(desktopCommandChannels.getGitBranchInfo, dir),
  listGitBranches: (dir: string) => invokeMain(desktopCommandChannels.listGitBranches, dir),
  switchGitBranch: (dir: string, branch: string) =>
    invokeMain(desktopCommandChannels.switchGitBranch, dir, branch),
  writeClipboardText: async (text: string): Promise<void> => {
    await invokeMain(desktopCommandChannels.writeClipboardText, text);
  },
  writeClipboardHtml: async (html: string, text: string): Promise<void> => {
    await invokeMain(desktopCommandChannels.writeClipboardHtml, html, text);
  },
};

function getAppLocale(): unknown {
  try {
    return sendSyncToMain(desktopCommandChannels.getAppLocale) ?? config.GOSLING_LOCALE;
  } catch {
    return config.GOSLING_LOCALE;
  }
}

const appConfigAPI: AppConfigAPI = {
  get: (key: string) => (key === 'GOSLING_LOCALE' ? getAppLocale() : config[key]),
  getAll: () => ({ ...config, GOSLING_LOCALE: getAppLocale() }),
};

// Expose the APIs
contextBridge.exposeInMainWorld('electron', electronAPI);
contextBridge.exposeInMainWorld('appConfig', appConfigAPI);

// Type declaration for TypeScript
declare global {
  interface Window {
    electron: ElectronAPI;
    appConfig: AppConfigAPI;
  }
}
