import type { BrowserWindowConstructorOptions } from 'electron';
import path from 'node:path';
import { createMinimalShellWindowOptions } from '../shellHost';
import { applyShellAppIdentity, type ShellAppIdentityAdapter } from './appIdentity';
import {
  buildShellDiagnostics,
  serializeShellDiagnostics,
  writeShellDiagnostics,
} from './diagnostics';
import { ShellHandoffStore } from './handoff';
import { registerShellIpc, type RegisteredShellIpc, type ShellIpcMainAdapter } from './ipcMain';
import type { ShellLifecycleState } from './lifecycle';
import { loadShellResources, type ShellResourceFiles } from './resources';
import { createShellRuntimeController } from './runtimeController';

interface ShellWebContents {
  id: number;
  mainFrame: unknown;
  send(channel: string, ...args: unknown[]): void;
  setWindowOpenHandler(handler: (details: { url: string }) => { action: 'deny' }): void;
  on(
    event: 'will-navigate',
    listener: (event: { preventDefault(): void }, url: string) => void
  ): void;
}

interface ShellWindow {
  webContents: ShellWebContents;
  once(event: 'ready-to-show', listener: () => void): void;
  on(event: 'closed', listener: () => void): void;
  isDestroyed(): boolean;
  isMinimized(): boolean;
  restore(): void;
  show(): void;
  focus(): void;
  loadURL(url: string): Promise<void> | void;
}

export interface ShellBootstrapAdapter {
  app: ShellAppIdentityAdapter & {
    isPackaged: boolean;
    whenReady(): Promise<void>;
    requestSingleInstanceLock(): boolean;
    setAsDefaultProtocolClient(scheme: string): boolean;
    on(event: 'second-instance', listener: () => void): void;
    on(event: 'before-quit', listener: (event: { preventDefault(): void }) => void): void;
    quit(): void;
    exit(code?: number): void;
  };
  ipcMain: ShellIpcMainAdapter;
  createWindow(options: BrowserWindowConstructorOptions): ShellWindow;
  showSaveDialog(options: {
    title: string;
    defaultPath: string;
    buttonLabel: string;
    message: string;
  }): Promise<{ canceled: boolean; filePath?: string }>;
  openExternal(url: string): Promise<void>;
  resourcesPath: string;
  preloadPath: string;
  rendererUrl: string;
  workingDir: string;
  shellResources: ShellResourceFiles;
  allowedExternalOrigins?: ReadonlySet<string>;
  now?: () => string;
}

export interface ShellBootstrap {
  started: boolean;
  stop(): Promise<void>;
}

function actionResult(state: ShellLifecycleState, accepted: boolean) {
  return { accepted, generation: state.generation, state: state.name };
}

export async function bootstrapShell(adapter: ShellBootstrapAdapter): Promise<ShellBootstrap> {
  const loaded = loadShellResources({
    isPackaged: adapter.app.isPackaged,
    resourcesPath: adapter.resourcesPath,
    files: adapter.shellResources,
  });
  const identity = applyShellAppIdentity(adapter.app, loaded.profile.product);
  if (!adapter.app.requestSingleInstanceLock()) {
    adapter.app.quit();
    return { started: false, stop: async () => {} };
  }
  adapter.app.setAsDefaultProtocolClient(identity.protocolScheme);
  await adapter.app.whenReady();

  const controller = createShellRuntimeController({
    profile: loaded.profile,
    manifest: loaded.manifest,
    provisioningPath: loaded.provisioningPath,
    diagnosticsDir: identity.paths.diagnostics,
    processRegistryPath: identity.paths.processRegistry,
    workingDir: adapter.workingDir,
    isPackaged: adapter.app.isPackaged,
    resourcesPath: adapter.resourcesPath,
    preloadPath: adapter.preloadPath,
    sessionPartition: identity.sessionPartition,
    clientName: loaded.profile.product.id,
    clientVersion: loaded.profile.product.version,
  });
  const window = adapter.createWindow(
    createMinimalShellWindowOptions({
      profile: {
        id: loaded.profile.product.id,
        displayName: loaded.profile.product.displayName,
      },
      preloadPath: adapter.preloadPath,
      sessionPartition: identity.sessionPartition,
    })
  );
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event, url) => {
    if (url !== adapter.rendererUrl) {
      event.preventDefault();
    }
  });

  const handoffs = new ShellHandoffStore(
    {
      id: loaded.profile.product.id,
      displayName: loaded.profile.product.displayName,
      version: loaded.profile.product.version,
    },
    loaded.manifest.compatibility.handoffSchemaVersion
  );
  const now = adapter.now ?? (() => new Date().toISOString());
  let ipc: RegisteredShellIpc;
  ipc = registerShellIpc({
    ipcMain: adapter.ipcMain,
    renderer: window.webContents as never,
    allowedExternalOrigins: adapter.allowedExternalOrigins ?? new Set(),
    operations: {
      runtimeRead: () => controller.read(),
      runtimeRetry: async (request) => {
        handoffs.clear();
        const prior = controller.read();
        const state = await controller.retry(request.generation);
        return actionResult(state, state !== prior);
      },
      runtimeStop: async (request) => {
        handoffs.clear();
        const prior = controller.read();
        const state = await controller.stop(request.generation);
        return actionResult(state, state !== prior || state.name === 'stopped');
      },
      diagnosticsSave: async (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('diagnostics request generation is stale');
        }
        const selected = await adapter.showSaveDialog({
          title: 'Save redacted shell diagnostics',
          defaultPath: `${loaded.profile.product.id}-diagnostics.json`,
          buttonLabel: 'Save',
          message: 'Inspect this redacted report before sharing it.',
        });
        if (selected.canceled || !selected.filePath) {
          return { status: 'canceled' };
        }
        const bundle = buildShellDiagnostics({
          generatedAt: now(),
          manifest: loaded.manifest,
          lifecycle: controller.read(),
          startup: controller.getStartupDiagnostics(),
          exitDetails: controller.getExitDetails(),
          processRegistryPath: identity.paths.processRegistry,
        });
        writeShellDiagnostics(selected.filePath, serializeShellDiagnostics(bundle));
        return { status: 'saved', fileName: path.basename(selected.filePath) };
      },
      handoffPrepare: async (request) => {
        if (request.generation !== controller.read().generation || !controller.getAcp()) {
          throw new Error('handoff request generation is stale or unavailable');
        }
        const { generation, ...params } = request;
        const prepared = await controller.getAcp()!.prepareHandoff(params);
        return { generation, handoff: handoffs.prepare(generation, prepared.handoff) };
      },
      handoffConfirm: async (request) => {
        const uri = handoffs.confirm(request.generation, request.handoffId);
        await adapter.openExternal(uri);
        return { opened: true };
      },
      externalOpen: async (url) => {
        await adapter.openExternal(url);
        return { opened: true };
      },
    },
  });
  const removeChangedListener = controller.onChanged((state) => ipc.publishRuntimeChanged(state));

  let stopPromise: Promise<void> | null = null;
  const stop = () => {
    stopPromise ??= (async () => {
      handoffs.clear();
      const current = controller.read();
      await controller.stop(current.generation);
      removeChangedListener();
      ipc.dispose();
    })();
    return stopPromise;
  };

  let quitAfterCleanup = false;
  adapter.app.on('before-quit', (event) => {
    if (quitAfterCleanup) {
      return;
    }
    event.preventDefault();
    void stop().finally(() => {
      quitAfterCleanup = true;
      adapter.app.quit();
    });
  });
  adapter.app.on('second-instance', () => {
    if (window.isMinimized()) {
      window.restore();
    }
    window.show();
    window.focus();
  });
  window.on('closed', () => {
    adapter.app.quit();
  });
  window.once('ready-to-show', () => {
    if (!window.isDestroyed()) {
      window.show();
    }
  });
  await window.loadURL(adapter.rendererUrl);
  void controller.start();
  return { started: true, stop };
}
