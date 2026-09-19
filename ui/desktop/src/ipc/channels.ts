import type Electron from 'electron';
import type { Settings, SettingKey } from '../utils/settings';
import type { ArtifactRoutingConfig, ArtifactSaveRequest } from '../types/artifactRouter';

export interface ThemeChangePayload {
  mode: string;
  useSystemTheme: boolean;
  theme: string;
  tokensUpdated?: boolean;
}

export interface InitialMessageOptions {
  noAutoSubmit?: boolean;
}

export interface UpdaterEvent {
  event: string;
  data?: unknown;
}

export interface NotificationData {
  title: string;
  body: string;
}

export interface MessageBoxOptions {
  type?: 'none' | 'info' | 'error' | 'question' | 'warning';
  buttons?: string[];
  defaultId?: number;
  title?: string;
  message: string;
  detail?: string;
}

export interface McpAppProxyCsp {
  connectDomains?: string[];
  resourceDomains?: string[];
  frameDomains?: string[];
  baseUriDomains?: string[];
}

/** The ACP secret is transported as a WebSocket subprotocol, never in the URL. */
export interface AcpEndpoint {
  url: string;
  subprotocol: string;
}

export interface CreateChatWindowOptions {
  query?: string;
  dir?: string;
  version?: string;
  resumeSessionId?: string;
  viewType?: string;
}

export const desktopCommandChannels = {
  directoryChooser: 'directory-chooser',
  sessionDirectoryChooser: 'session-directory-chooser',
  getResearchLibraryPath: 'get-research-library-path',
  chooseResearchLibraryPath: 'choose-research-library-path',
  listResearchLibraryFiles: 'list-research-library-files',
  importResearchLibraryFiles: 'import-research-library-files',
  grantSessionDirectories: 'grant-session-directories',
  logInfo: 'logInfo',
  showNotification: 'notify',
  showMessageBox: 'show-message-box',
  saveArtifact: 'save-artifact',
  setArtifactRoutingConfig: 'set-artifact-routing-config',
  openInChrome: 'open-in-chrome',
  reloadApp: 'reload-app',
  checkForOllama: 'check-ollama',
  selectFileOrDirectory: 'select-file-or-directory',
  selectArtifactFile: 'select-artifact-file',
  selectImportSessionFile: 'select-import-session-file',
  readFile: 'read-file',
  readArtifactFile: 'read-artifact-file',
  copyArtifactContents: 'copy-artifact-contents',
  readArtifactTitles: 'read-artifact-titles',
  classifyArtifactRepositories: 'classify-artifact-repositories',
  getArtifactFileTimestamps: 'get-artifact-file-timestamps',
  openArtifactFile: 'open-artifact-file',
  revealArtifactFile: 'reveal-artifact-file',
  writeFile: 'write-file',
  deleteFile: 'delete-file',
  trashArtifactFiles: 'trash-artifact-files',
  ensureDirectory: 'ensure-directory',
  listFiles: 'list-files',
  getAllowedExtensions: 'get-allowed-extensions',
  setMenuBarIcon: 'set-menu-bar-icon',
  getMenuBarIconState: 'get-menu-bar-icon-state',
  setDockIcon: 'set-dock-icon',
  getDockIconState: 'get-dock-icon-state',
  getSetting: 'get-setting',
  getSettings: 'get-settings',
  setSetting: 'set-setting',
  getAcpUrl: 'get-acp-url',
  getMcpAppProxyUrl: 'get-mcp-app-proxy-url',
  setWakelock: 'set-wakelock',
  getWakelockState: 'get-wakelock-state',
  setWakelockActive: 'set-wakelock-active',
  setSessionRecoveryActive: 'set-session-recovery-active',
  setSpellcheck: 'set-spellcheck',
  getSpellcheckState: 'get-spellcheck-state',
  openNotificationsSettings: 'open-notifications-settings',
  isAnyWindowFocused: 'is-any-window-focused',
  getIsFullScreen: 'get-is-fullscreen',
  reactReady: 'react-ready',
  createChatWindow: 'create-chat-window',
  broadcastThemeChange: 'broadcast-theme-change',
  broadcastWorkspaceChange: 'broadcast-workspace-change',
  openExternal: 'open-external',
  checkForUpdates: 'check-for-updates',
  downloadUpdate: 'download-update',
  installUpdate: 'install-update',
  restartApp: 'restart-app',
  getUpdateState: 'get-update-state',
  isUsingGitHubFallback: 'is-using-github-fallback',
  getAutoDownloadDisabled: 'get-auto-download-disabled',
  closeWindow: 'close-window',
  openDirectoryInExplorer: 'open-directory-in-explorer',
  addRecentDir: 'add-recent-dir',
  listRecentDirs: 'list-recent-dirs',
  listGitWorktreeDirs: 'list-git-worktree-dirs',
  getGitBranchInfo: 'get-git-branch-info',
  listGitBranches: 'list-git-branches',
  switchGitBranch: 'switch-git-branch',
  writeClipboardText: 'write-clipboard-text',
  writeClipboardHtml: 'write-clipboard-html',
  getAppVersion: 'get-app-version',
  getAppLocale: 'get-app-locale',
  getCurrentVersion: 'get-current-version',
} as const;

export type DesktopCommandChannel =
  (typeof desktopCommandChannels)[keyof typeof desktopCommandChannels];

export const desktopSendChannels = [
  desktopCommandChannels.reactReady,
  desktopCommandChannels.createChatWindow,
  desktopCommandChannels.logInfo,
  desktopCommandChannels.showNotification,
  desktopCommandChannels.openInChrome,
  desktopCommandChannels.reloadApp,
  desktopCommandChannels.broadcastThemeChange,
  desktopCommandChannels.broadcastWorkspaceChange,
  desktopCommandChannels.restartApp,
  desktopCommandChannels.closeWindow,
] as const;

export const desktopSyncChannels = [
  desktopCommandChannels.getAppVersion,
  desktopCommandChannels.getAppLocale,
] as const;

export const desktopInvokeChannels = [
  desktopCommandChannels.directoryChooser,
  desktopCommandChannels.sessionDirectoryChooser,
  desktopCommandChannels.getResearchLibraryPath,
  desktopCommandChannels.chooseResearchLibraryPath,
  desktopCommandChannels.listResearchLibraryFiles,
  desktopCommandChannels.importResearchLibraryFiles,
  desktopCommandChannels.grantSessionDirectories,
  desktopCommandChannels.showMessageBox,
  desktopCommandChannels.saveArtifact,
  desktopCommandChannels.setArtifactRoutingConfig,
  desktopCommandChannels.checkForOllama,
  desktopCommandChannels.selectFileOrDirectory,
  desktopCommandChannels.selectArtifactFile,
  desktopCommandChannels.selectImportSessionFile,
  desktopCommandChannels.readFile,
  desktopCommandChannels.readArtifactFile,
  desktopCommandChannels.copyArtifactContents,
  desktopCommandChannels.readArtifactTitles,
  desktopCommandChannels.classifyArtifactRepositories,
  desktopCommandChannels.getArtifactFileTimestamps,
  desktopCommandChannels.openArtifactFile,
  desktopCommandChannels.revealArtifactFile,
  desktopCommandChannels.writeFile,
  desktopCommandChannels.deleteFile,
  desktopCommandChannels.trashArtifactFiles,
  desktopCommandChannels.ensureDirectory,
  desktopCommandChannels.listFiles,
  desktopCommandChannels.getAllowedExtensions,
  desktopCommandChannels.setMenuBarIcon,
  desktopCommandChannels.getMenuBarIconState,
  desktopCommandChannels.setDockIcon,
  desktopCommandChannels.getDockIconState,
  desktopCommandChannels.getSetting,
  desktopCommandChannels.getSettings,
  desktopCommandChannels.setSetting,
  desktopCommandChannels.getAcpUrl,
  desktopCommandChannels.getMcpAppProxyUrl,
  desktopCommandChannels.setWakelock,
  desktopCommandChannels.getWakelockState,
  desktopCommandChannels.setWakelockActive,
  desktopCommandChannels.setSessionRecoveryActive,
  desktopCommandChannels.setSpellcheck,
  desktopCommandChannels.getSpellcheckState,
  desktopCommandChannels.openNotificationsSettings,
  desktopCommandChannels.isAnyWindowFocused,
  desktopCommandChannels.getIsFullScreen,
  desktopCommandChannels.openExternal,
  desktopCommandChannels.checkForUpdates,
  desktopCommandChannels.downloadUpdate,
  desktopCommandChannels.installUpdate,
  desktopCommandChannels.getUpdateState,
  desktopCommandChannels.isUsingGitHubFallback,
  desktopCommandChannels.getAutoDownloadDisabled,
  desktopCommandChannels.openDirectoryInExplorer,
  desktopCommandChannels.addRecentDir,
  desktopCommandChannels.listRecentDirs,
  desktopCommandChannels.listGitWorktreeDirs,
  desktopCommandChannels.getGitBranchInfo,
  desktopCommandChannels.listGitBranches,
  desktopCommandChannels.switchGitBranch,
  desktopCommandChannels.writeClipboardText,
  desktopCommandChannels.writeClipboardHtml,
] as const;

export const desktopMainOnlyChannels = [desktopCommandChannels.getCurrentVersion] as const;

export const desktopPreloadCommandChannels = [
  ...desktopSendChannels,
  ...desktopSyncChannels,
  ...desktopInvokeChannels,
] as const;

export type DesktopSendChannel = (typeof desktopSendChannels)[number];
export type DesktopSyncChannel = (typeof desktopSyncChannels)[number];
export type DesktopInvokeChannel = (typeof desktopInvokeChannels)[number];

/** Argument tuples for every renderer-to-main command, shared by both sides of the bridge. */
export interface DesktopCommandPayloads {
  [desktopCommandChannels.directoryChooser]: [];
  [desktopCommandChannels.sessionDirectoryChooser]: [];
  [desktopCommandChannels.getResearchLibraryPath]: [];
  [desktopCommandChannels.chooseResearchLibraryPath]: [];
  [desktopCommandChannels.listResearchLibraryFiles]: [];
  [desktopCommandChannels.importResearchLibraryFiles]: [];
  [desktopCommandChannels.grantSessionDirectories]: [directories: string[]];
  [desktopCommandChannels.logInfo]: [text: string];
  [desktopCommandChannels.showNotification]: [data: NotificationData];
  [desktopCommandChannels.showMessageBox]: [options: MessageBoxOptions];
  [desktopCommandChannels.saveArtifact]: [request: ArtifactSaveRequest];
  [desktopCommandChannels.setArtifactRoutingConfig]: [config: ArtifactRoutingConfig | null];
  [desktopCommandChannels.openInChrome]: [url: string];
  [desktopCommandChannels.reloadApp]: [];
  [desktopCommandChannels.checkForOllama]: [];
  [desktopCommandChannels.selectFileOrDirectory]: [defaultPath?: string];
  [desktopCommandChannels.selectArtifactFile]: [defaultPath?: string];
  [desktopCommandChannels.selectImportSessionFile]: [];
  [desktopCommandChannels.readFile]: [filePath: string];
  [desktopCommandChannels.readArtifactFile]: [filePath: string, baseDirectory?: string];
  [desktopCommandChannels.copyArtifactContents]: [filePath: string, baseDirectory?: string];
  [desktopCommandChannels.readArtifactTitles]: [
    requests: Array<{ filePath: string; baseDirectory?: string }>,
  ];
  [desktopCommandChannels.classifyArtifactRepositories]: [filePaths: string[]];
  [desktopCommandChannels.getArtifactFileTimestamps]: [filePaths: string[]];
  [desktopCommandChannels.openArtifactFile]: [filePath: string, baseDirectory?: string];
  [desktopCommandChannels.revealArtifactFile]: [filePath: string, baseDirectory?: string];
  [desktopCommandChannels.writeFile]: [filePath: string, content: string];
  [desktopCommandChannels.deleteFile]: [filePath: string];
  [desktopCommandChannels.trashArtifactFiles]: [paths: string[]];
  [desktopCommandChannels.ensureDirectory]: [dirPath: string];
  [desktopCommandChannels.listFiles]: [dirPath: string];
  [desktopCommandChannels.getAllowedExtensions]: [];
  [desktopCommandChannels.setMenuBarIcon]: [show: boolean];
  [desktopCommandChannels.getMenuBarIconState]: [];
  [desktopCommandChannels.setDockIcon]: [show: boolean];
  [desktopCommandChannels.getDockIconState]: [];
  [desktopCommandChannels.getSetting]: [key: SettingKey];
  [desktopCommandChannels.getSettings]: [keys: SettingKey[]];
  [desktopCommandChannels.setSetting]: [key: SettingKey, value: Settings[SettingKey]];
  [desktopCommandChannels.getAcpUrl]: [];
  [desktopCommandChannels.getMcpAppProxyUrl]: [csp?: McpAppProxyCsp | null];
  [desktopCommandChannels.setWakelock]: [enable: boolean];
  [desktopCommandChannels.getWakelockState]: [];
  [desktopCommandChannels.setWakelockActive]: [sessionId: string, active: boolean];
  [desktopCommandChannels.setSessionRecoveryActive]: [
    sessionId: string,
    workingDir: string,
    active: boolean,
  ];
  [desktopCommandChannels.setSpellcheck]: [enable: boolean];
  [desktopCommandChannels.getSpellcheckState]: [];
  [desktopCommandChannels.openNotificationsSettings]: [];
  [desktopCommandChannels.isAnyWindowFocused]: [];
  [desktopCommandChannels.getIsFullScreen]: [];
  [desktopCommandChannels.reactReady]: [];
  [desktopCommandChannels.createChatWindow]: [options?: CreateChatWindowOptions];
  [desktopCommandChannels.broadcastThemeChange]: [themeData: ThemeChangePayload];
  [desktopCommandChannels.broadcastWorkspaceChange]: [];
  [desktopCommandChannels.openExternal]: [url: string];
  [desktopCommandChannels.checkForUpdates]: [];
  [desktopCommandChannels.downloadUpdate]: [];
  [desktopCommandChannels.installUpdate]: [];
  [desktopCommandChannels.restartApp]: [];
  [desktopCommandChannels.getUpdateState]: [];
  [desktopCommandChannels.isUsingGitHubFallback]: [];
  [desktopCommandChannels.getAutoDownloadDisabled]: [];
  [desktopCommandChannels.closeWindow]: [];
  [desktopCommandChannels.openDirectoryInExplorer]: [directoryPath: string];
  [desktopCommandChannels.addRecentDir]: [dir: string];
  [desktopCommandChannels.listRecentDirs]: [];
  [desktopCommandChannels.listGitWorktreeDirs]: [dir: string];
  [desktopCommandChannels.getGitBranchInfo]: [dir: string];
  [desktopCommandChannels.listGitBranches]: [dir: string];
  [desktopCommandChannels.switchGitBranch]: [dir: string, branch: string];
  [desktopCommandChannels.writeClipboardText]: [text: string];
  [desktopCommandChannels.writeClipboardHtml]: [html: string, text: string];
  [desktopCommandChannels.getAppVersion]: [];
  [desktopCommandChannels.getAppLocale]: [];
  [desktopCommandChannels.getCurrentVersion]: [];
}

export const rendererEventChannels = {
  addExtension: 'add-extension',
  artifactDownloadUnrouted: 'artifact-download-unrouted',
  fatalError: 'fatal-error',
  findCommand: 'find-command',
  findNext: 'find-next',
  findPrevious: 'find-previous',
  focusInput: 'focus-input',
  fullscreenChange: 'fullscreen-change',
  mouseBackButtonClicked: 'mouse-back-button-clicked',
  newChat: 'new-chat',
  openSharedSession: 'open-shared-session',
  setInitialMessage: 'set-initial-message',
  setView: 'set-view',
  themeChanged: 'theme-changed',
  toggleNavigation: 'toggle-navigation',
  updaterEvent: 'updater-event',
  useSelectionFind: 'use-selection-find',
  workspacesChanged: 'workspaces-changed',
} as const;

export type RendererEventChannel =
  (typeof rendererEventChannels)[keyof typeof rendererEventChannels];

export interface RendererEventPayloads {
  [rendererEventChannels.addExtension]: [url: string];
  [rendererEventChannels.artifactDownloadUnrouted]: [fileName: string];
  [rendererEventChannels.fatalError]: [message: string];
  [rendererEventChannels.findCommand]: [];
  [rendererEventChannels.findNext]: [];
  [rendererEventChannels.findPrevious]: [];
  [rendererEventChannels.focusInput]: [];
  [rendererEventChannels.fullscreenChange]: [isFullScreen: boolean];
  [rendererEventChannels.mouseBackButtonClicked]: [];
  [rendererEventChannels.newChat]: [];
  [rendererEventChannels.openSharedSession]: [url: string];
  [rendererEventChannels.setInitialMessage]: [
    initialMessage: string,
    options?: InitialMessageOptions,
  ];
  [rendererEventChannels.setView]: [view: string, section?: string];
  [rendererEventChannels.themeChanged]: [themeData: ThemeChangePayload];
  [rendererEventChannels.toggleNavigation]: [];
  [rendererEventChannels.updaterEvent]: [event: UpdaterEvent];
  [rendererEventChannels.useSelectionFind]: [];
  [rendererEventChannels.workspacesChanged]: [];
}

export type RendererEventCallback<T extends RendererEventChannel> = (
  event: Electron.IpcRendererEvent,
  ...args: RendererEventPayloads[T]
) => void;
