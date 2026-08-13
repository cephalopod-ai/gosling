import { randomBytes } from 'node:crypto';
import type { MinimalShellHostOptions, MinimalShellHostRuntime } from '../shellHost';
import { createMinimalShellHost } from '../shellHost';
import { connectShellAcp, ShellCompatibilityError, type ShellAcpConnection } from './acpRuntime';
import {
  initialShellLifecycle,
  transitionShellLifecycle,
  type ShellLifecycleState,
  type ShellLifecycleStateName,
} from './lifecycle';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

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
  clientName: string;
  clientVersion: string;
}

export interface ShellRuntimeController {
  read(): ShellLifecycleState;
  start(): Promise<ShellLifecycleState>;
  retry(expectedGeneration: number): Promise<ShellLifecycleState>;
  stop(expectedGeneration: number): Promise<ShellLifecycleState>;
  onChanged(listener: (state: ShellLifecycleState) => void): () => void;
  getAcp(): ShellAcpConnection | null;
}

const defaultDependencies: ShellRuntimeControllerDependencies = {
  createHost: createMinimalShellHost,
  connectAcp: connectShellAcp,
  generateSecret: () => randomBytes(32).toString('hex'),
  now: () => new Date().toISOString(),
};

function startupFailureName(error: unknown): ShellLifecycleStateName {
  if (error instanceof ShellCompatibilityError) {
    return error.result.code === 'PROVISIONING_INVALID' ? 'degraded' : 'incompatible';
  }
  return 'offline';
}

function startupFailureCode(error: unknown): string {
  if (error instanceof ShellCompatibilityError) {
    return error.result.code;
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
  let acp: ShellAcpConnection | null = null;
  let startPromise: Promise<ShellLifecycleState> | null = null;
  let stopPromise: Promise<ShellLifecycleState> | null = null;
  let expectedExit = false;
  let exitListener: ((code: number | null, signal: string | null) => void) | null = null;
  const listeners = new Set<(state: ShellLifecycleState) => void>();

  const stateName = (): ShellLifecycleStateName => state.name;

  const publish = (next: ShellLifecycleState) => {
    state = next;
    for (const listener of listeners) {
      listener(state);
    }
  };

  const transition = (name: ShellLifecycleStateName, reasonCode?: string) => {
    const result = transitionShellLifecycle(state, {
      generation,
      name,
      at: dependencies.now(),
      ...(reasonCode ? { reasonCode } : {}),
    });
    if (!result.accepted) {
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
    acp?.close();
    acp = null;
    const failedHost = host;
    detachExitListener();
    transition('offline', 'BACKEND_EXITED');
    if (failedHost) {
      void failedHost.backend.cleanup().finally(() => {
        if (host === failedHost) {
          host = null;
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
        });
        if (eventGeneration !== generation || state.name !== 'booting') {
          await runtime.backend.cleanup();
          return state;
        }
        host = runtime;
        exitListener = () => handleUnexpectedExit(eventGeneration);
        (runtime.backend.process as BackendProcessEvents).once('exit', exitListener);
        if (!transition('validating')) {
          await clearRuntime();
          return state;
        }
        acp = await dependencies.connectAcp({
          acpUrl: runtime.backend.acpUrl,
          profile: options.profile,
          manifest: options.manifest,
          clientName: options.clientName,
          clientVersion: options.clientVersion,
        });
        if (eventGeneration !== generation || stateName() !== 'validating') {
          await clearRuntime();
          return state;
        }
        void acp.closed
          .then(() => handleUnexpectedExit(eventGeneration))
          .catch(() => {
            handleUnexpectedExit(eventGeneration);
          });
        transition('ready');
      } catch (error) {
        await clearRuntime();
        if (
          eventGeneration === generation &&
          state.name !== 'stopping' &&
          state.name !== 'stopped'
        ) {
          transition(startupFailureName(error), startupFailureCode(error));
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
      await clearRuntime();
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
    read: () => state,
    start,
    retry,
    stop,
    onChanged(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getAcp: () => acp,
  };
}
