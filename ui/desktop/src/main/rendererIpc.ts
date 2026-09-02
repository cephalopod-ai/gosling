// Owns renderer readiness, directory, Git, ACP endpoint, and MCP proxy IPC registration.
// Extracted from ui/desktop/src/main.ts in a behavior-preserving modularization.
// The compatibility facade imports registerRendererIpcHandlers; it re-exports none.

import type { IpcMain } from 'electron';
import { BrowserWindow, dialog } from 'electron';
import os from 'node:os';
import { URLSearchParams } from 'node:url';
import { acpTokenSubprotocol } from '../goslingServe';
import type { GoslingServeLeaseRegistry } from '../goslingServeLeaseRegistry';
import { desktopCommandChannels, rendererEventChannels } from '../ipc/channels';
import type { RendererDirectoryGrantRegistry } from '../utils/rendererDirectoryGrants';
import { addRecentDir, loadRecentDirs } from '../utils/recentDirs';
import { registerGitIpcHandlers } from './gitIpc';

export type McpAppProxyCsp = {
  connectDomains?: string[];
  resourceDomains?: string[];
  frameDomains?: string[];
  baseUriDomains?: string[];
};

interface RendererIpcLogger {
  info: (...args: unknown[]) => void;
  error: (...args: unknown[]) => void;
}

export interface RendererIpcDependencies {
  log: RendererIpcLogger;
  pendingInitialMessages: Map<number, string>;
  pendingInitialMessageNoAutoSubmit: Set<number>;
  pendingDeepLinks: Map<number, string>;
  reactReadyWindows: Set<number>;
  sendOpenSharedSession: (window: BrowserWindow, url: string) => void;
  openExternalIfSafe: (url: string) => Promise<void>;
  rendererDirectoryGrants: RendererDirectoryGrantRegistry;
  assertRendererFileAccess: (webContentsId: number, filePath: string) => Promise<string>;
  goslingServeLeases: Pick<GoslingServeLeaseRegistry, 'getAcpUrl' | 'getSecretKey'>;
}

export function loopbackHttpBaseFromAcpUrl(acpUrl: string): string | null {
  try {
    const parsed = new URL(acpUrl);
    if (!['ws:', 'wss:'].includes(parsed.protocol)) return null;
    if (!['127.0.0.1', 'localhost', '::1', '[::1]'].includes(parsed.hostname)) return null;
    parsed.protocol = parsed.protocol === 'wss:' ? 'https:' : 'http:';
    parsed.pathname = '';
    parsed.search = '';
    parsed.hash = '';
    return parsed.toString().replace(/\/+$/, '');
  } catch {
    return null;
  }
}

function appendDomainParams(proxyUrl: URL, csp?: McpAppProxyCsp | null): void {
  if (csp?.connectDomains?.length)
    proxyUrl.searchParams.set('connect_domains', csp.connectDomains.join(','));
  if (csp?.resourceDomains?.length)
    proxyUrl.searchParams.set('resource_domains', csp.resourceDomains.join(','));
  if (csp?.frameDomains?.length)
    proxyUrl.searchParams.set('frame_domains', csp.frameDomains.join(','));
  if (csp?.baseUriDomains?.length)
    proxyUrl.searchParams.set('base_uri_domains', csp.baseUriDomains.join(','));
}

export function registerRendererIpcHandlers(
  targetIpcMain: Pick<IpcMain, 'on' | 'handle'>,
  dependencies: RendererIpcDependencies
): void {
  const {
    log,
    pendingInitialMessages,
    pendingInitialMessageNoAutoSubmit,
    pendingDeepLinks,
    reactReadyWindows,
    sendOpenSharedSession,
    openExternalIfSafe,
    rendererDirectoryGrants,
    assertRendererFileAccess,
    goslingServeLeases,
  } = dependencies;

  targetIpcMain.on(desktopCommandChannels.reactReady, (event) => {
    log.info('React ready event received');
    // Get the window that sent the react-ready event
    const window = BrowserWindow.fromWebContents(event.sender);
    const windowId = window?.id;
    if (windowId !== undefined) reactReadyWindows.add(windowId);
    // Send any pending initial message for this window
    if (windowId && pendingInitialMessages.has(windowId)) {
      const initialMessage = pendingInitialMessages.get(windowId)!;
      const noAutoSubmit = pendingInitialMessageNoAutoSubmit.has(windowId);
      log.info('Sending pending initial message to window');
      window.webContents.send(rendererEventChannels.setInitialMessage, initialMessage, {
        noAutoSubmit,
      });
      pendingInitialMessages.delete(windowId);
      pendingInitialMessageNoAutoSubmit.delete(windowId);
    }
    if (windowId && pendingDeepLinks.has(windowId) && window) {
      const deepLinkUrl = pendingDeepLinks.get(windowId)!;
      pendingDeepLinks.delete(windowId);
      log.info('Processing pending deep link for window:', windowId);
      try {
        const parsedUrl = new URL(deepLinkUrl);
        if (parsedUrl.hostname === 'extension')
          window.webContents.send(rendererEventChannels.addExtension, deepLinkUrl);
        else if (parsedUrl.hostname === 'sessions') sendOpenSharedSession(window, deepLinkUrl);
      } catch (error) {
        log.error('Error processing pending deep link:', error);
      }
    }
  });

  targetIpcMain.handle(desktopCommandChannels.openExternal, async (_event, url: string) => {
    await openExternalIfSafe(url);
  });
  targetIpcMain.handle('directory-chooser', async (event) => {
    const result = await dialog.showOpenDialog({
      properties: ['openDirectory', 'createDirectory'],
      defaultPath: os.homedir(),
    });
    if (!result.canceled && result.filePaths[0])
      rendererDirectoryGrants.grantSelectedPath(event.sender.id, result.filePaths[0]);
    return result;
  });
  targetIpcMain.handle('session-directory-chooser', () =>
    dialog.showOpenDialog({
      properties: ['openDirectory', 'createDirectory'],
      defaultPath: os.homedir(),
      title: 'Add directory to this session',
    })
  );
  targetIpcMain.handle('add-recent-dir', (_event, dir: string) => {
    if (dir) addRecentDir(dir);
  });
  targetIpcMain.handle('list-recent-dirs', () => loadRecentDirs());
  registerGitIpcHandlers(targetIpcMain, assertRendererFileAccess);
  targetIpcMain.handle('get-acp-url', async (event) => {
    const windowId = BrowserWindow.fromWebContents(event.sender)?.id;
    if (!windowId) return null;
    const url = goslingServeLeases.getAcpUrl(windowId);
    const secretKey = goslingServeLeases.getSecretKey(windowId);
    if (!url || !secretKey) return null;
    // The secret travels in the WebSocket subprotocol, not the URL (SEC-GOS-001).
    return { url, subprotocol: acpTokenSubprotocol(secretKey) };
  });
  targetIpcMain.handle('get-mcp-app-proxy-url', async (event, csp?: McpAppProxyCsp | null) => {
    const windowId = BrowserWindow.fromWebContents(event.sender)?.id;
    if (!windowId) return null;
    const acpUrl = goslingServeLeases.getAcpUrl(windowId);
    const secretKey = goslingServeLeases.getSecretKey(windowId);
    if (!acpUrl || !secretKey) return null;
    const httpBase = loopbackHttpBaseFromAcpUrl(acpUrl);
    if (!httpBase) return null;
    const proxyUrl = new URL(`${httpBase}/mcp-app-proxy`);
    appendDomainParams(proxyUrl, csp);
    proxyUrl.hash = new URLSearchParams({ secret: secretKey }).toString();
    return proxyUrl.toString();
  });
}
