import { randomBytes } from 'node:crypto';
import type { DomainStatusNotification_unstable } from '@repo-makeover/gosling-sdk';
import type { GoslingServeStartupDiagnostics } from '../startupDiagnostics';
import type { MinimalShellHostOptions, MinimalShellHostRuntime } from '../shellHost';
import { createMinimalShellHost } from '../shellHost';
import {
  connectShellAcp,
  provisioningIssueSummaries,
  ShellCompatibilityError,
  type ShellAcpConnection,
  type ShellProvisioningIssueSummary,
} from './acpRuntime';
import {
  initialShellLifecycle,
  transitionShellLifecycle,
  type ShellLifecycleState,
  type ShellLifecycleStateName,
} from './lifecycle';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';
import {
  createShellSessionController,
  type ShellSessionController,
  type ShellSessionUpdate,
} from './sessionController';
import {
  createShellInteractionController,
  type ShellInteraction,
  type ShellInteractionController,
} from './interactionController';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';

interface BackendProcessEvents {
  once(event: 'exit', listener: (code: number | null, signal: string | null) => void): unknown;
  off(event: 'exit', listener: (code: number | null, signal: string | null) => void): unknown;
}

export interface ShellRuntimeControllerDependencies {
  createHost(options: MinimalShellHostOptions): Promise<MinimalShellHostRuntime>;
  connectAcp(input: Parameters<typeof connectShellAcp>[0]): Promise<ShellAcpConnection>;
  generateSecret(): string;
  now(): string;
}

export interface ShellRuntimeControllerOptions {
  profile: ResolvedShellProductProfile;
  manifest: ShellBuildManifest;
  provisioningPath: string;
  diagnosticsDir: string;
  processRegistryPath: string;
  workingDir: string;
  isPackaged: boolean;
  resourcesPath?: string;
  preloadPath: string;
  sessionPartition: string;
  clientName: string;
  clientVersion: string;
}

export interface ShellRuntimeController {
  read(): ShellRuntimeSnapshot;
  start(): Promise<ShellLifecycleState>;
  retry(expectedGeneration: number): Promise<ShellLifecycleState>;
  stop(expectedGeneration: number): Promise<ShellLifecycleState>;
  onChanged(listener: (state: ShellRuntimeSnapshot) => void): () => void;
  onSessionUpdated(listener: (update: ShellSessionUpdate) => void): () => void;
  onInteractionRequested(listener: (interaction: ShellInteraction) => void): () => void;
  getAcp(): ShellAcpConnection | null;
  getSessionController(): ShellSessionController | null;
  getInteractionController(): ShellInteractionController | null;
  getStartupDiagnostics(): GoslingServeStartupDiagnostics | null;
  getExitDetails(): { code: number | null; signal: string | null } | null;
}

const defaultDependencies: ShellRuntimeControllerDependencies = {
  createHost: createMinimalShellHost,
  connectAcp: connectShellAcp,
  generateSecret: () => randomBytes(32).toString('hex'),
  now: () => new Date().toISOString(),
};

function startupFailureName(error: unknown): ShellLifecycleStateName {
  if (error instanceof ShellCompatibilityError) {
    if (error.result.code !== 'PROVISIONING_INVALID') return 'incompatible';
    return error.provisioningIssues.some((issue) =>
      [
        'missing_credential_profile',
        'credential_profile_unavailable',
        'credential_provider_mismatch',
      ].includes(issue.code)
    )
      ? 'relink_required'
      : 'degraded';
  }
  if (startupFailureCode(error) === 'ADAPTER_DESCRIPTOR_MISMATCH') {
    return 'incompatible';
  }
  return 'offline';
}

function startupFailureCode(error: unknown): string {
  if (error instanceof ShellCompatibilityError) {
    return error.result.code;
  }
  if (error instanceof Error && error.message.includes('ADAPTER_DESCRIPTOR_MISMATCH')) {
    return 'ADAPTER_DESCRIPTOR_MISMATCH';
  }
  return 'STARTUP_FAILED';
}

export function createShellRuntimeController(
  options: ShellRuntimeControllerOptions,
  dependencies: ShellRuntimeControllerDependencies = defaultDependencies
): ShellRuntimeController {
  let generation = 1;
  let state = initialShellLifecycle(generation, dependencies.now());
  let host: MinimalShellHostRuntime | null = null;
  let latestStartupDiagnostics: GoslingServeStartupDiagnostics | null = null;
  let latestExitDetails: { code: number | null; signal: string | null } | null = null;
  let provisioningIssues: ShellProvisioningIssueSummary[] = [];
  let acp: ShellAcpConnection | null = null;
  let sessions: ShellSessionController | null = null;
  let interactions: ShellInteractionController | null = null;
  let adapterStatus: NonNullable<ShellRuntimeSnapshot['adapter']>['status'] | null = null;
  let startPromise: Promise<ShellLifecycleState> | null = null;
  let stopPromise: Promise<ShellLifecycleState> | null = null;
  let expectedExit = false;
  let exitListener: ((code: number | null, signal: string | null) => void) | null = null;
  const listeners = new Set<(state: ShellRuntimeSnapshot) => void>();
  const sessionListeners = new Set<(update: ShellSessionUpdate) => void>();
  const interactionListeners = new Set<(interaction: ShellInteraction) => void>();

  const stateName = (): ShellLifecycleStateName => state.name;

  const snapshot = (): ShellRuntimeSnapshot => {
    const session = sessions?.read() ?? null;
    return {
      ...state,
      lifecycleState: state.name,
      identity: acp
        ? {
            id: options.profile.product.id,
            displayName: options.profile.product.displayName,
            version: options.profile.product.version,
          }
        : null,
      runtimeNamespace: acp?.runtimeNamespace ?? null,
      compatibility: {
        status: acp ? 'compatible' : state.name === 'incompatible' ? 'incompatible' : 'unverified',
      },
      provisioningIssues: provisioningIssues.map((issue) => ({ ...issue })),
      session: session?.status === 'none' ? null : session,
      adapter: acp?.domainAdapter
        ? {
            ...acp.domainAdapter,
            status: adapterStatus ?? 'ready',
          }
        : null,
      pendingInteractions: interactions?.read() ?? [],
    };
  };

  const notify = () => {
    for (const listener of listeners) {
      listener(snapshot());
    }
  };

  const publish = (next: ShellLifecycleState) => {
    state = next;
    notify();
  };

  const transition = (name: ShellLifecycleStateName, reasonCode?: string) => {
    const result = transitionShellLifecycle(state, {
      generation,
      name,
      at: dependencies.now(),
      ...(reasonCode ? { reasonCode } : {}),
    });
    if (!result.accepted) {
      host?.backend.recordStartupEvent('shell_lifecycle_transition_rejected', {
        code: result.stale ? 'STALE_LIFECYCLE_EVENT' : 'ILLEGAL_LIFECYCLE_EVENT',
      });
      return false;
    }
    publish(result.state);
    return true;
  };

  const detachExitListener = () => {
    if (host && exitListener) {
      (host.backend.process as BackendProcessEvents).off('exit', exitListener);
    }
    exitListener = null;
  };

  const clearRuntime = async () => {
    detachExitListener();
    interactions?.clear();
    interactions = null;
    sessions?.close();
    sessions = null;
    acp?.close();
    acp = null;
    const currentHost = host;
    host = null;
    if (currentHost) {
      await currentHost.backend.cleanup();
    }
  };

  const handleUnexpectedExit = (eventGeneration: number) => {
    if (expectedExit || eventGeneration !== generation || state.name === 'stopped') {
      return;
    }
    transition('offline', 'BACKEND_EXITED');
    acp?.close();
    acp = null;
    interactions?.clear();
    interactions = null;
    sessions?.close('failed');
    sessions = null;
    const failedHost = host;
    detachExitListener();
    if (failedHost) {
      void failedHost.backend
        .cleanup()
        .finally(() => {
          if (host === failedHost) host = null;
        })
        .catch(() => {
          if (eventGeneration === generation && state.name === 'offline') {
            transition('fatal', 'CLEANUP_FAILED');
          }
        });
    }
  };

  const start = (): Promise<ShellLifecycleState> => {
    if (startPromise) {
      return startPromise;
    }
    if (state.name !== 'booting') {
      return Promise.resolve(state);
    }
    const eventGeneration = generation;
    startPromise = (async () => {
      expectedExit = false;
      provisioningIssues = [];
      adapterStatus = null;
      try {
        const runtime = await dependencies.createHost({
          profile: {
            id: options.profile.product.id,
            displayName: options.profile.product.displayName,
            version: options.profile.product.version,
            runtimeNamespace: options.profile.product.runtimeNamespace,
            provisioningPath: options.provisioningPath,
          },
          serverSecret: dependencies.generateSecret(),
          workingDir: options.workingDir,
          diagnosticsDir: options.diagnosticsDir,
          processRegistryPath: options.processRegistryPath,
          isPackaged: options.isPackaged,
          resourcesPath: options.resourcesPath,
          preloadPath: options.preloadPath,
          sessionPartition: options.sessionPartition,
        });
        if (eventGeneration !== generation || state.name !== 'booting') {
          await runtime.backend.cleanup();
          return state;
        }
        host = runtime;
        latestStartupDiagnostics = runtime.backend.getStartupDiagnostics();
        latestExitDetails = runtime.backend.getExitDetails();
        exitListener = (code, signal) => {
          latestStartupDiagnostics = runtime.backend.getStartupDiagnostics();
          latestExitDetails = { code, signal };
          handleUnexpectedExit(eventGeneration);
        };
        (runtime.backend.process as BackendProcessEvents).once('exit', exitListener);
        if (!transition('validating')) {
          await clearRuntime();
          return state;
        }
        interactions = createShellInteractionController({
          generation: () => generation,
          promptAttemptId: () => sessions?.read().promptAttempt?.id ?? null,
        });
        interactions.onChanged(notify);
        interactions.onRequested((interaction) => {
          for (const listener of interactionListeners) listener(interaction);
        });
        const acceptsInteraction = (sessionId: string) => {
          const session = sessions?.read();
          return session?.status === 'active' && session.sessionId === sessionId;
        };
        acp = await dependencies.connectAcp({
          acpUrl: runtime.backend.acpUrl,
          profile: options.profile,
          manifest: options.manifest,
          workingDir: options.workingDir,
          clientName: options.clientName,
          clientVersion: options.clientVersion,
          callbacks: () => ({
            requestPermission: (request) =>
              acceptsInteraction(request.sessionId) && interactions
                ? interactions.requestPermission(request)
                : Promise.resolve({ outcome: { outcome: 'cancelled' } }),
            unstable_createElicitation: (request) =>
              'sessionId' in request && acceptsInteraction(request.sessionId) && interactions
                ? interactions.requestElicitation(request)
                : Promise.resolve({ action: 'cancel' }),
            sessionUpdate: async (notification) => sessions?.ingestUpdate(notification),
            unstable_shellDomainStatus: async (notification: DomainStatusNotification_unstable) => {
              adapterStatus = notification.status;
              notify();
            },
          }),
        });
        if (eventGeneration !== generation || stateName() !== 'validating') {
          await clearRuntime();
          return state;
        }
        provisioningIssues = provisioningIssueSummaries(acp.provisioning.validation.issues);
        sessions = createShellSessionController({
          transport: acp,
          generation: () => generation,
        });
        sessions.onChanged(notify);
        sessions.onUpdate((update) => {
          if (update.kind === 'started' && state.name === 'ready') transition('busy');
          if (
            (update.kind === 'completed' ||
              update.kind === 'cancelled' ||
              update.kind === 'failed') &&
            state.name === 'busy'
          ) {
            transition('ready');
          }
          if (update.kind !== 'started') interactions?.clearSession(update.sessionId);
          for (const listener of sessionListeners) listener(update);
        });
        void acp.closed
          .then(() => handleUnexpectedExit(eventGeneration))
          .catch(() => {
            handleUnexpectedExit(eventGeneration);
          });
        transition('ready');
      } catch (error) {
        provisioningIssues =
          error instanceof ShellCompatibilityError ? error.provisioningIssues : provisioningIssues;
        const failureCode = startupFailureCode(error);
        host?.backend.recordStartupEvent('shell_preflight_failed', { code: failureCode });
        latestStartupDiagnostics =
          host?.backend.getStartupDiagnostics() ?? latestStartupDiagnostics;
        try {
          await clearRuntime();
        } catch {
          if (eventGeneration === generation && state.name !== 'stopped') {
            transition('fatal', 'CLEANUP_FAILED');
          }
          return state;
        }
        if (
          eventGeneration === generation &&
          state.name !== 'stopping' &&
          state.name !== 'stopped'
        ) {
          transition(startupFailureName(error), failureCode);
        }
      }
      return state;
    })().finally(() => {
      startPromise = null;
    });
    return startPromise;
  };

  const stop = (expectedGeneration: number): Promise<ShellLifecycleState> => {
    if (expectedGeneration !== generation) {
      return Promise.resolve(state);
    }
    if (stopPromise) {
      return stopPromise;
    }
    if (state.name === 'stopped') {
      return Promise.resolve(state);
    }
    stopPromise = (async () => {
      expectedExit = true;
      if (state.name !== 'stopping') {
        transition('stopping');
      }
      try {
        await clearRuntime();
      } catch {
        transition('fatal', 'CLEANUP_FAILED');
        return state;
      }
      transition('stopped');
      return state;
    })().finally(() => {
      stopPromise = null;
    });
    return stopPromise;
  };

  const retry = async (expectedGeneration: number): Promise<ShellLifecycleState> => {
    if (
      expectedGeneration !== generation ||
      !state.allowedActions.includes('retry') ||
      stopPromise ||
      startPromise
    ) {
      return state;
    }
    await stop(expectedGeneration);
    generation += 1;
    const booting = transitionShellLifecycle(state, {
      generation,
      name: 'booting',
      at: dependencies.now(),
    });
    if (!booting.accepted) {
      return state;
    }
    publish(booting.state);
    return start();
  };

  return {
    read: snapshot,
    start,
    retry,
    stop,
    onChanged(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    onSessionUpdated(listener) {
      sessionListeners.add(listener);
      return () => sessionListeners.delete(listener);
    },
    onInteractionRequested(listener) {
      interactionListeners.add(listener);
      return () => interactionListeners.delete(listener);
    },
    getAcp: () => acp,
    getSessionController: () => sessions,
    getInteractionController: () => interactions,
    getStartupDiagnostics: () => host?.backend.getStartupDiagnostics() ?? latestStartupDiagnostics,
    getExitDetails: () => host?.backend.getExitDetails() ?? latestExitDetails,
  };
}
