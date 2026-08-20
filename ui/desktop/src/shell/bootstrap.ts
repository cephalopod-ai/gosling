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
import type { ShellModelOption, ShellModelSelection, ShellSettingsSnapshot } from './ipc';
import type { ShellLifecycleState } from './lifecycle';
import { loadShellResources, type ShellResourceFiles } from './resources';
import { createShellRuntimeController, type ShellRuntimeController } from './runtimeController';
import { createShellSettingsStore } from './localSettings';
import { createShellDirectoryController } from './directoryController';
import { createShellCredentialController } from './credentialController';

interface ShellWebContents {
  id: number;
  mainFrame: unknown;
  isDestroyed(): boolean;
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
    properties: ReadonlyArray<'openFile' | 'openDirectory' | 'createDirectory' | 'dontAddToRecent'>;
    filters?: ReadonlyArray<{ name: string; extensions: string[] }>;
  }): Promise<{ canceled: boolean; filePaths: string[] }>;
  showConfirmDialog(options: {
    title: string;
    message: string;
    detail: string;
    confirmLabel: string;
    cancelLabel: string;
  }): Promise<{ confirmed: boolean }>;
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
  const declaredCapabilities = new Set(loaded.manifest.consumer?.declaredCapabilities ?? []);
  const readModelSelection = async (): Promise<ShellModelSelection> => {
    try {
      const client = requireAcp().client.gosling;
      if (!client.defaultsRead_unstable || !client.providersList_unstable) {
        throw new Error('model selection is unavailable');
      }
      const [defaults, providers] = await Promise.all([
        client.defaultsRead_unstable({}),
        client.providersList_unstable({}),
      ]);
      const options = providers.entries
        .filter((provider) => provider.configured)
        .flatMap((provider): ShellModelOption[] =>
          provider.models.slice(0, 128).map((model) => ({
            providerId: provider.providerId,
            providerName: provider.providerName,
            modelId: model.id,
            modelName: model.name ?? model.id,
          }))
        )
        .filter(
          (option) =>
            option.providerId.length > 0 &&
            option.providerId.length <= 256 &&
            option.modelId.length > 0 &&
            option.modelId.length <= 512
        )
        .sort((left, right) =>
          `${left.providerName}/${left.modelName}`.localeCompare(
            `${right.providerName}/${right.modelName}`
          )
        )
        .slice(0, 128);
      return {
        status: 'available',
        providerId: defaults.providerId ?? null,
        modelId: defaults.modelId ?? null,
        options,
      };
    } catch {
      return { status: 'unavailable', providerId: null, modelId: null, options: [] };
    }
  };
  const readSettings = async (): Promise<ShellSettingsSnapshot> => ({
    appearance: settings.read().appearance,
    recovery: settings.recovery(),
    modelSelection: await readModelSelection(),
  });
  const assertActiveLibrarySession = (generation: number, sessionId: string): void => {
    if (generation !== controller.read().generation) {
      throw new Error('library request generation is stale');
    }
    const session = controller.getSessionController()?.read();
    if (session?.status !== 'active' || session.sessionId !== sessionId) {
      throw new Error('library access is limited to the active session');
    }
  };
  let ipc: RegisteredShellIpc;
  ipc = registerShellIpc({
    ipcMain: adapter.ipcMain,
    renderer: window.webContents as never,
    allowedExternalOrigins: adapter.allowedExternalOrigins ?? new Set(),
    declaredCapabilities,
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
      directorySelect: async (request) => {
        const result = await directory.select(request.generation);
        if (result.status === 'selected') {
          // Project-local extensions and skills are per directory, so the inventory the renderer
          // holds must be re-resolved against the directory just accepted.
          await controller.refreshModules();
        }
        return result;
      },
      credentialSelect: (request) => credentials.select(request.generation, request.profileId),
      sessionCreate: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.create(request.generation);
      },
      sessionList: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('session list generation is stale');
        }
        const accepted = directory.accepted();
        if (!accepted) throw new Error('no working directory is selected');
        return requireAcp().listSessions(accepted);
      },
      sessionResume: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.resume(request.generation, request.sessionId);
      },
      sessionTranscriptRead: (request) => {
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        return sessions.readTranscript(request.generation, request.sessionId);
      },
      sessionArtifactsRead: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('artifact inventory generation is stale');
        }
        const sessions = controller.getSessionController();
        if (!sessions) throw new Error('session runtime is unavailable');
        const session = sessions.read();
        if (session.status !== 'active' || session.sessionId !== request.sessionId) {
          throw new Error('artifact inventory is limited to the active session');
        }
        return requireAcp().listArtifacts(request.sessionId);
      },
      sessionLibraryRead: (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        return requireAcp().listLibrary(request.sessionId);
      },
      sessionLibraryAddText: (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        return requireAcp().addLibraryText({
          sessionId: request.sessionId,
          scope: request.scope,
          name: request.name,
          text: request.text,
        });
      },
      sessionLibraryAddImage: (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        return requireAcp().addLibraryImage({
          sessionId: request.sessionId,
          scope: request.scope,
          name: request.name,
          mimeType: request.mimeType,
          data: request.data,
        });
      },
      sessionLibraryLinkFile: async (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        const selected = await adapter.showOpenDialog({
          title: 'Add a file to the library',
          buttonLabel: 'Add file',
          message: 'The file stays linked in place. Its path is never exposed to the shell UI.',
          properties: ['openFile', 'dontAddToRecent'],
          filters: [
            {
              name: 'Supported files',
              extensions: [
                'pdf',
                'png',
                'jpg',
                'jpeg',
                'webp',
                'gif',
                'txt',
                'md',
                'csv',
                'tsv',
                'json',
                'rs',
                'js',
                'ts',
                'tsx',
                'py',
              ],
            },
          ],
        });
        assertActiveLibrarySession(request.generation, request.sessionId);
        if (selected.canceled || selected.filePaths.length !== 1) return { status: 'canceled' };
        const response = await requireAcp().linkLibraryFile({
          sessionId: request.sessionId,
          scope: request.scope,
          path: selected.filePaths[0],
        });
        return { status: 'added', item: response.item };
      },
      sessionLibraryRemove: (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        return requireAcp().removeLibraryItem(request.sessionId, request.itemId);
      },
      extensionsAvailableRead: (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('extension inventory generation is stale');
        }
        return requireAcp().listAvailableExtensions();
      },
      sessionExtensionsRead: (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        return requireAcp().listSessionExtensions(request.sessionId);
      },
      sessionExtensionsAdd: async (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        const acp = requireAcp();
        await acp.addSessionExtension(request.sessionId, request.extension);
        return acp.listSessionExtensions(request.sessionId);
      },
      sessionExtensionsRemove: async (request) => {
        assertActiveLibrarySession(request.generation, request.sessionId);
        const acp = requireAcp();
        await acp.removeSessionExtension(request.sessionId, request.name);
        return acp.listSessionExtensions(request.sessionId);
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
        if (session.status !== 'active') {
          // A create or resume awaiting the backend would finish after this reset and reinstate an
          // active session, leaving a second server session reachable by nobody.
          throw new Error('cannot detach while a session is still opening');
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
        if (
          (request.libraryItemIds?.length ?? 0) > 0 &&
          !declaredCapabilities.has('session.library.read')
        ) {
          throw new Error('library references require session.library.read');
        }
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
        return acp
          .domainAction({
            sessionId: request.sessionId,
            generation: request.generation,
            action: request.action,
            input: request.input ?? null,
          })
          .then((response) => {
            if (response.confirmationActionId) {
              const interactions = controller.getInteractionController();
              if (!interactions) throw new Error('interaction runtime is unavailable');
              interactions.requestConfirmation({
                actionId: response.confirmationActionId,
                generation: request.generation,
                sessionId: request.sessionId,
                action: request.action,
                actionInput: request.input,
              });
            }
            return response;
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
        const interactions = controller.getInteractionController();
        const pending = interactions
          ?.read()
          .some(
            (interaction) =>
              interaction.kind === 'confirm' &&
              interaction.actionId === request.actionId &&
              interaction.generation === request.generation &&
              interaction.sessionId === request.sessionId
          );
        if (!interactions || !pending) throw new Error('confirmation action is stale');
        const acp = controller.getAcp();
        if (!acp || !acp.domainAdapter) throw new Error('domain adapter is unavailable');
        return acp
          .confirmDomainAction({
            sessionId: request.sessionId,
            generation: request.generation,
            actionId: request.actionId,
            approve: request.approve,
          })
          .finally(() => {
            interactions.respondConfirmation(request);
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
      settingsRead: readSettings,
      settingsAppearanceUpdate: async (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('settings request generation is stale');
        }
        const { theme, textScale } = request;
        const updated = settings.setAppearance({ theme, textScale });
        return { ...(await readSettings()), appearance: updated.appearance };
      },
      settingsModelSelect: async (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('settings request generation is stale');
        }
        const selection = await readModelSelection();
        const selected = selection.options.find(
          (option) => option.providerId === request.providerId && option.modelId === request.modelId
        );
        if (!selected) throw new Error('selected model is not configured in Gosling');
        const sessions = controller.getSessionController();
        const session = sessions?.read();
        if (session?.status === 'active') {
          if (!sessions) throw new Error('session runtime is unavailable');
          await sessions.setProviderModel({
            generation: request.generation,
            providerId: selected.providerId,
            modelId: selected.modelId,
          });
        } else {
          const gosling = requireAcp().client.gosling;
          if (!gosling.defaultsSave_unstable) throw new Error('model selection is unavailable');
          await gosling.defaultsSave_unstable({
            providerId: selected.providerId,
            modelId: selected.modelId,
          });
        }
        return readSettings();
      },
      settingsReset: async (request) => {
        if (request.generation !== controller.read().generation) {
          throw new Error('settings request generation is stale');
        }
        const confirmation = await adapter.showConfirmDialog({
          title: 'Reset local settings',
          message: `Reset ${loaded.profile.product.displayName} settings?`,
          detail:
            'This clears the theme, text size, remembered directory, and preferred credential ' +
            'for this app only. Your Gosling credentials and other apps are not affected.',
          confirmLabel: 'Reset',
          cancelLabel: 'Cancel',
        });
        // Re-check after the await: the runtime may have torn down or retried while the native
        // dialog was open, the same hazard directoryController.ts guards for around its own
        // showOpenDialog await.
        if (request.generation !== controller.read().generation) {
          throw new Error('settings request generation is stale');
        }
        if (!confirmation.confirmed) {
          return readSettings();
        }
        const reset = settings.reset();
        // The settings file is now cleared, but directory/credentials keep their own in-memory
        // selection for session creation (runtimeController.ts reads `.accepted()`/`.selected()`,
        // not the settings file) — without this, a session created before the next restart would
        // still use the directory/credential the operator just reset. clear() is the same
        // teardown call runtimeController.ts already uses when the backend itself is lost, and it
        // publishes through the existing onChanged wiring so the renderer's snapshot updates too.
        directory.clear();
        credentials.clear();
        return { ...(await readSettings()), appearance: reset.appearance };
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
