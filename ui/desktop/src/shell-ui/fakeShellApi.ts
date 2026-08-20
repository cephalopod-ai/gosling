import type { GoslingShellAPI } from '../shell/preloadApi';
import type { ShellInteraction } from '../shell/interactionController';
import type { ShellRuntimeSnapshot } from '../shell/runtimeSnapshot';
import type { ShellSessionUpdate } from '../shell/sessionController';
import type { ShellSettingsSnapshot } from '../shell/ipc';
import { settings as defaultSettings, snapshot as defaultSnapshot } from './testSupport';

export interface FakeShellApi {
  api: GoslingShellAPI;
  calls: Array<{ operation: string; request: unknown }>;
  emitRuntime(snapshot: ShellRuntimeSnapshot): void;
  emitSessionUpdate(update: ShellSessionUpdate): void;
  emitInteraction(interaction: ShellInteraction): void;
  setSnapshot(snapshot: ShellRuntimeSnapshot): void;
  setSettings(settings: ShellSettingsSnapshot): void;
  failNext(operation: string, error: unknown): void;
  listenerCounts(): Record<string, number>;
}

type Listener<T> = (value: T) => void;

/**
 * A typed stand-in for the preload bridge. Components and the store are only ever allowed to reach
 * the shell through this surface, so a test that needs something absent here is testing behaviour
 * the real renderer could not perform either.
 */
export function createFakeShellApi(
  initial: {
    snapshot?: ShellRuntimeSnapshot;
    settings?: ShellSettingsSnapshot;
  } = {}
): FakeShellApi {
  let snapshot = initial.snapshot ?? defaultSnapshot();
  let settings = initial.settings ?? defaultSettings();
  const calls: Array<{ operation: string; request: unknown }> = [];
  const failures = new Map<string, unknown>();

  const runtimeListeners = new Set<Listener<ShellRuntimeSnapshot>>();
  const sessionListeners = new Set<Listener<ShellSessionUpdate>>();
  const permissionListeners = new Set<Listener<never>>();
  const elicitationListeners = new Set<Listener<never>>();
  const confirmationListeners = new Set<Listener<never>>();

  const record = <T>(operation: string, request: unknown, result: T): Promise<T> => {
    calls.push({ operation, request });
    if (failures.has(operation)) {
      const error = failures.get(operation);
      failures.delete(operation);
      return Promise.reject(error);
    }
    return Promise.resolve(result);
  };

  const subscribe = <T>(set: Set<Listener<T>>, listener: Listener<T>) => {
    set.add(listener);
    return () => set.delete(listener);
  };

  const api: GoslingShellAPI = {
    runtime: {
      read: () => record('runtime.read', undefined, snapshot),
      retry: (request) =>
        record('runtime.retry', request, {
          accepted: true,
          generation: snapshot.generation,
          state: snapshot.lifecycleState,
        }),
      stop: (request) =>
        record('runtime.stop', request, {
          accepted: true,
          generation: snapshot.generation,
          state: snapshot.lifecycleState,
        }),
      onChanged: (listener) => subscribe(runtimeListeners, listener),
    },
    directory: {
      select: (request) =>
        record('directory.select', request, {
          status: 'selected' as const,
          directory: snapshot.directory,
        }),
    },
    credential: {
      select: (request) => record('credential.select', request, snapshot.credentials),
    },
    session: {
      create: (request) =>
        record('session.create', request, snapshot.session ?? defaultSnapshot().session!),
      list: (request) => record('session.list', request, []),
      resume: (request) =>
        record('session.resume', request, snapshot.session ?? defaultSnapshot().session!),
      readTranscript: (request) =>
        record('session.transcript.read', request, {
          generation: snapshot.generation,
          sessionId: request.sessionId,
          integrity: 'complete' as const,
          firstSeq: null,
          lastSeq: null,
          truncated: false,
          updates: [],
        }),
      readArtifacts: (request) =>
        record('session.artifacts.read', request, {
          artifacts: [{ name: 'summary.md', kind: 'text' as const, relation: 'created' as const }],
          totalCount: 1,
          truncated: false,
        }),
      detach: (request) => record('session.detach', request, { detached: true, sessionId: null }),
      onUpdated: (listener) => subscribe(sessionListeners, listener),
    },
    library: {
      list: (request) => record('session.library.read', request, { items: [] }),
      addText: (request) =>
        record('session.library.write', request, {
          item: {
            id: 'lib-text',
            name: request.name,
            kind: 'text' as const,
            scope: request.scope,
            status: 'available' as const,
            mimeType: 'text/plain',
            sizeBytes: request.text.length,
          },
        }),
      addImage: (request) =>
        record('session.library.write', request, {
          item: {
            id: 'lib-image',
            name: request.name,
            kind: 'image' as const,
            scope: request.scope,
            status: 'available' as const,
            mimeType: request.mimeType,
            sizeBytes: request.data.length,
          },
        }),
      linkFile: (request) =>
        record('session.library.write', request, { status: 'canceled' as const }),
      remove: (request) => record('session.library.write', request, { removed: true }),
    },
    extensions: {
      listAvailable: (request) => record('session.extensions.read', request, { extensions: [] }),
      listForSession: (request) => record('session.extensions.read', request, { extensions: [] }),
      add: (request) => record('session.extensions.write', request, { extensions: [] }),
      remove: (request) => record('session.extensions.write', request, { extensions: [] }),
    },
    prompt: {
      submit: (request) => record('prompt.submit', request, { promptAttemptId: 'attempt-1' }),
      cancel: (request) => record('prompt.cancel', request, undefined),
    },
    permission: {
      respond: (request) => record('permission.respond', request, undefined),
      onRequested: (listener) =>
        subscribe(permissionListeners as Set<Listener<never>>, listener as Listener<never>),
    },
    elicitation: {
      respond: (request) => record('elicitation.respond', request, undefined),
      onRequested: (listener) =>
        subscribe(elicitationListeners as Set<Listener<never>>, listener as Listener<never>),
    },
    domain: {
      snapshot: (request) => record('domain.snapshot', request, { domainId: 'example' }),
      action: (request) =>
        record('domain.action', request, { domainId: 'example', action: request.action }),
      confirm: (request) =>
        record('confirmation.respond', request, {
          status: request.approve ? ('approved' as const) : ('denied' as const),
        }),
      onConfirmationRequested: (listener) =>
        subscribe(confirmationListeners as Set<Listener<never>>, listener as Listener<never>),
    },
    diagnostics: {
      save: (request) =>
        record('diagnostics.save', request, { status: 'saved' as const, fileName: 'diag.json' }),
    },
    handoff: {
      prepare: (request) =>
        record('handoff.prepare', request, {
          generation: snapshot.generation,
          handoff: {
            schemaVersion: 1,
            handoffId: 'handoff-1',
            origin: { id: 'template', displayName: 'Default Shell Template', version: '0.0.0' },
            sourceSessionId: request.sessionId,
            question: request.question,
            requestedCapability: request.requestedCapability,
          },
        }),
      confirm: (request) => record('handoff.confirm', request, { opened: true }),
    },
    external: {
      open: (url) => record('external.open', url, { opened: true }),
    },
    settings: {
      read: () => record('settings.read', undefined, settings),
      updateAppearance: (request) => {
        const next: ShellSettingsSnapshot = {
          appearance: {
            theme: request.theme ?? settings.appearance.theme,
            textScale: request.textScale ?? settings.appearance.textScale,
          },
          recovery: settings.recovery,
        };
        settings = next;
        return record('settings.appearance.update', request, next);
      },
      selectModel: (request) => record('settings.model.select', request, settings),
      reset: (request) => record('settings.reset', request, settings),
    },
  };

  return {
    api,
    calls,
    emitRuntime(next) {
      snapshot = next;
      for (const listener of runtimeListeners) listener(next);
    },
    emitSessionUpdate(next) {
      for (const listener of sessionListeners) listener(next);
    },
    emitInteraction(interaction) {
      const target =
        interaction.kind === 'permission'
          ? permissionListeners
          : interaction.kind === 'elicitation'
            ? elicitationListeners
            : confirmationListeners;
      for (const listener of target) (listener as (value: ShellInteraction) => void)(interaction);
    },
    setSnapshot(next) {
      snapshot = next;
    },
    setSettings(next) {
      settings = next;
    },
    failNext(operation, error) {
      failures.set(operation, error);
    },
    listenerCounts() {
      return {
        runtime: runtimeListeners.size,
        session: sessionListeners.size,
        permission: permissionListeners.size,
        elicitation: elicitationListeners.size,
        confirmation: confirmationListeners.size,
      };
    },
  };
}
