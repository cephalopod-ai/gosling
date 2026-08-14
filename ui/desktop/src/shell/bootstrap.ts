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
import { createShellRuntimeController, type ShellRuntimeController } from './runtimeController';
import { createShellSettingsStore } from './localSettings';
import { createShellDirectoryController } from './directoryController';
import { createShellCredentialController } from './credentialController';

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
  showOpenDialog(options: {
    title: string;
    buttonLabel: string;
    message: string;
    properties: ReadonlyArray<'openDirectory' | 'createDirectory' | 'dontAddToRecent'>;
  }): Promise<{ canceled: boolean; filePaths: string[] }>;
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

  const settings = createShellSettingsStore(identity.paths.localSettings);
  let controller: ShellRuntimeController;
  const requireAcp = () => {
    const acp = controller.getAcp();
    if (!acp) throw new Error('shell runtime is unavailable');
    return acp;
  };
  const directory = createShellDirectoryController({
    settings,
    showOpenDialog: (options) =>
      adapter.showOpenDialog({
        ...options,
        properties: ['openDirectory', 'dontAddToRecent'],
      }),
    validate: (candidate) => requireAcp().validateDirectory(candidate),
    generation: () => controller.read().generation,
    isSelectable: () => {
      const snapshot = controller.read();
      if (snapshot.lifecycleState !== 'ready') {
        return { allowed: false, reasonCode: 'shell runtime is not ready' };
      }
      if (snapshot.session?.promptAttempt || snapshot.pendingInteractions.length > 0) {
        return { allowed: false, reasonCode: 'an interaction is in progress' };
      }
      if (snapshot.session) {
        return { allowed: false, reasonCode: 'detach the current session before switching' };
      }
      return { allowed: true };
    },
  });
  const credentials = createShellCredentialController({
    settings,
    list: () => requireAcp().listCredentials(),
    generation: () => controller.read().generation,
  });
  controller = createShellRuntimeController({
    profile: loaded.profile,
    manifest: loaded.manifest,
    provisioningPath: loaded.provisioningPath,
    diagnosticsDir: identity.paths.diagnostics,
    processRegistryPath: identity.paths.processRegistry,
    workingDir: adapter.workingDir,
    directory,
    credentials,
    settings,
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
    declaredCapabilities: new Set(loaded.manifest.consumer?.declaredCapabilities ?? []),
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
      directorySelect: (request) => directory.select(request.generation),
      credentialSelect: (request) => credentials.select(request.generation, request.profileId),
      sessionCreate: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.create(request.generation);
      },
      sessionResume: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.resume(request.generation, request.sessionId);
      },
      sessionDetach: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('session detach generation is stale');
        }
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        const session = sessions.read();
        if (session.status === 'none') {
          return { detached: false, sessionId: null };
        }
        if (session.promptAttempt) {
          throw new Error('cannot detach while a prompt attempt is streaming');
        }
        if ((controller.getInteractionController()?.read() ?? []).length > 0) {
          throw new Error('cannot detach while an interaction is pending');
        }
        sessions.close();
        return { detached: true, sessionId: session.sessionId };
      },
      promptSubmit: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.submit(request);
      },
      promptCancel: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.cancel(request);
      },
      permissionRespond: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('permission action generation is stale');
        }
        const interactions = controller.getInteractionController();
        if (!interactions) throw new Error('interaction runtime is unavailable');
        interactions.respondPermission(request);
      },
      elicitationRespond: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('elicitation action generation is stale');
        }
        const interactions = controller.getInteractionController();
        if (!interactions) throw new Error('interaction runtime is unavailable');
        interactions.respondElicitation(request);
      },
      domainSnapshot: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('domain snapshot generation is stale');
        }
        const acp = controller.getAcp();
        if (!acp || !acp.domainAdapter) throw new Error('domain adapter is unavailable');
        return acp.domainSnapshot({ input: request.input ?? null });
      },
      domainAction: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('domain action generation is stale');
        }
        const session = controller.getSessionController()?.read();
        if (session?.status !== 'active' || session.sessionId !== request.sessionId) {
          throw new Error('domain action session is stale');
        }
        const acp = controller.getAcp();
        if (!acp || !acp.domainAdapter) throw new Error('domain adapter is unavailable');
        return acp.domainAction({
          sessionId: request.sessionId,
          generation: request.generation,
          action: request.action,
          input: request.input ?? null,
        });
      },
      confirmationRespond: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('confirmation action generation is stale');
        }
        const session = controller.getSessionController()?.read();
        if (session?.status !== 'active' || session.sessionId !== request.sessionId) {
          throw new Error('confirmation action session is stale');
        }
        const acp = controller.getAcp();
        if (!acp || !acp.domainAdapter) throw new Error('domain adapter is unavailable');
        return acp.confirmDomainAction({
          sessionId: request.sessionId,
          generation: request.generation,
          actionId: request.actionId,
          approve: request.approve,
        });
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
  const removeSessionListener = controller.onSessionUpdated((update) =>
    ipc.publishSessionUpdated(update)
  );
  const removeInteractionListener = controller.onInteractionRequested((interaction) =>
    ipc.publishInteractionRequested(interaction)
  );

  let stopPromise: Promise<void> | null = null;
  const stop = () => {
    stopPromise ??= (async () => {
      handoffs.clear();
      const current = controller.read();
      await controller.stop(current.generation);
      removeChangedListener();
      removeSessionListener();
      removeInteractionListener();
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
