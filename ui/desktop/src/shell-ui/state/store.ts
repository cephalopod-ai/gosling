import type { GoslingShellAPI } from '../../shell/preloadApi';
import type { ShellTheme } from '../../shell/settingsSchema';
import type { GoslingExtension, ShellLibraryScope } from '@repo-makeover/gosling-sdk';
import { asOperationFailure } from '../api';
import { shellUiReducer } from './reducer';
import { isDeclared } from './route';
import {
  initialShellUiState,
  type ShellUiAction,
  type ShellUiPendingOperation,
  type ShellUiState,
} from './types';

export interface ShellStore {
  getState(): ShellUiState;
  subscribe(listener: () => void): () => void;
  dispatch(action: ShellUiAction): void;
  start(): Promise<void>;
  dispose(): void;
  actions: ShellStoreActions;
}

export interface ShellStoreActions {
  setDraft(draft: string): void;
  setView(view: ShellUiState['view']): void;
  dismissFailure(): void;
  clearNotices(): void;
  refreshRuntime(): Promise<void>;
  selectDirectory(): Promise<void>;
  selectCredential(profileId: string | null): Promise<void>;
  listSessions(): Promise<void>;
  createSession(): Promise<void>;
  resumeSession(sessionId: string): Promise<void>;
  detachSession(): Promise<boolean>;
  repairTranscript(sessionId?: string): Promise<void>;
  readOutputs(): Promise<void>;
  readLibrary(): Promise<void>;
  setLibraryScope(scope: ShellLibraryScope): void;
  toggleLibraryItem(itemId: string): void;
  addLibraryText(name: string, text: string): Promise<void>;
  addLibraryImage(name: string, mimeType: string, data: string): Promise<void>;
  linkLibraryFile(): Promise<void>;
  removeLibraryItem(itemId: string): Promise<void>;
  readExtensions(): Promise<void>;
  addSessionExtension(extension: GoslingExtension): Promise<void>;
  removeSessionExtension(name: string): Promise<void>;
  submitPrompt(): Promise<void>;
  cancelPrompt(): Promise<void>;
  respondPermission(actionId: string, allowOnce: boolean): Promise<void>;
  respondElicitation(
    actionId: string,
    action: 'submit' | 'decline' | 'cancel',
    fields?: Record<string, unknown>
  ): Promise<void>;
  respondConfirmation(actionId: string, approve: boolean): Promise<void>;
  saveDiagnostics(): Promise<void>;
  prepareHandoff(question: string, requestedCapability: string): Promise<void>;
  confirmHandoff(): Promise<void>;
  retryRuntime(): Promise<void>;
  stopRuntime(): Promise<void>;
  updateAppearance(update: { theme?: ShellTheme; textScale?: number }): Promise<void>;
  selectModel(providerId: string, modelId: string): Promise<void>;
  resetSettings(): Promise<void>;
}

export function createShellStore(api: GoslingShellAPI): ShellStore {
  let state = initialShellUiState();
  const listeners = new Set<() => void>();
  const unsubscribes: Array<() => void> = [];
  let disposed = false;

  const notify = () => {
    for (const listener of listeners) listener();
  };

  const dispatch = (action: ShellUiAction) => {
    const next = shellUiReducer(state, action);
    if (next === state) return;
    state = next;
    notify();
  };

  const generation = (): number | null => state.snapshot?.generation ?? null;

  /**
   * Every operation is fenced on the generation the renderer currently believes in, reports its own
   * `pending` marker, and converts a rejection into a classified failure. A generation that is not
   * yet known means the runtime snapshot has not arrived, so the operation is simply not issued.
   */
  const run = async <T>(
    pending: ShellUiPendingOperation,
    operation: (currentGeneration: number) => Promise<T>,
    onSuccess?: (result: T) => void
  ): Promise<boolean> => {
    const current = generation();
    if (current === null) return false;
    dispatch({ type: 'pending/changed', pending });
    try {
      const result = await operation(current);
      if (disposed) return false;
      // The failure clears on success rather than on start, so a banner the user still needs is not
      // erased by a background refresh.
      dispatch({ type: 'failure/cleared' });
      onSuccess?.(result);
      dispatch({ type: 'pending/changed', pending: null });
      return true;
    } catch (error) {
      if (disposed) return false;
      dispatch({ type: 'failure/raised', failure: asOperationFailure(error) });
      return false;
    }
  };

  const requireCapability = (capability: string): boolean => isDeclared(state, capability);

  const actions: ShellStoreActions = {
    setDraft(draft) {
      dispatch({ type: 'draft/changed', draft });
    },
    setView(view) {
      dispatch({ type: 'view/changed', view });
    },
    dismissFailure() {
      dispatch({ type: 'failure/cleared' });
    },
    clearNotices() {
      dispatch({ type: 'notices/cleared' });
    },

    async refreshRuntime() {
      try {
        const snapshot = await api.runtime.read();
        if (!disposed) dispatch({ type: 'snapshot/replaced', snapshot });
      } catch (error) {
        if (!disposed) dispatch({ type: 'failure/raised', failure: asOperationFailure(error) });
      }
    },

    async selectDirectory() {
      if (!requireCapability('directory.select')) return;
      await run(
        'directory.select',
        (current) => api.directory.select({ generation: current, userGesture: true }),
        (result) => {
          if (result.status === 'cancelled') dispatch({ type: 'directory/cancelled' });
        }
      );
    },

    async selectCredential(profileId) {
      if (!requireCapability('credential.select')) return;
      await run('credential.select', (current) =>
        api.credential.select({ generation: current, profileId })
      );
    },

    async listSessions() {
      if (!requireCapability('session.list')) return;
      dispatch({ type: 'sessions/loading' });
      await run(
        'session.list',
        (current) => api.session.list({ generation: current }),
        (items) => dispatch({ type: 'sessions/loaded', items })
      );
    },

    async createSession() {
      if (!requireCapability('session.create')) return;
      await run('session.create', (current) => api.session.create({ generation: current }));
    },

    async resumeSession(sessionId) {
      if (!requireCapability('session.resume')) return;
      let resumed: string | null = null;
      await run(
        'session.resume',
        (current) => api.session.resume({ generation: current, sessionId }),
        (record) => {
          resumed = record.sessionId;
          dispatch({ type: 'view/changed', view: 'workspace' });
        }
      );
      // The resumed id comes from the operation result: main publishes the runtime snapshot
      // asynchronously, so reading it back from `state` here would race and skip the replay.
      if (resumed) await actions.repairTranscript(resumed);
    },

    async detachSession() {
      if (!requireCapability('session.detach')) return false;
      const detached = await run('session.detach', (current) =>
        api.session.detach({ generation: current })
      );
      if (!detached) return false;
      await actions.listSessions();
      return true;
    },

    async repairTranscript(explicitSessionId) {
      if (!requireCapability('session.transcript.read')) return;
      const sessionId = explicitSessionId ?? state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.transcript.read',
        (current) => api.session.readTranscript({ generation: current, sessionId }),
        (transcript) => dispatch({ type: 'transcript/loaded', transcript })
      );
    },

    async readOutputs() {
      if (!requireCapability('session.artifacts.read')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      dispatch({ type: 'outputs/loading' });
      await run(
        'session.artifacts.read',
        (current) => api.session.readArtifacts({ generation: current, sessionId }),
        (response) =>
          dispatch({
            type: 'outputs/loaded',
            items: response.artifacts ?? [],
            totalCount: response.totalCount,
            truncated: response.truncated ?? false,
          })
      );
    },

    async readLibrary() {
      if (!requireCapability('session.library.read')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      dispatch({ type: 'library/loading' });
      await run(
        'session.library.read',
        (current) => api.library.list({ generation: current, sessionId }),
        (response) => dispatch({ type: 'library/loaded', items: response.items ?? [] })
      );
    },

    setLibraryScope(scope) {
      dispatch({ type: 'library/scopeChanged', scope });
    },

    toggleLibraryItem(itemId) {
      dispatch({ type: 'library/itemToggled', itemId });
    },

    async addLibraryText(name, text) {
      if (!requireCapability('session.library.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.library.write',
        (current) =>
          api.library.addText({
            generation: current,
            sessionId,
            scope: state.library.addScope,
            name,
            text,
          }),
        (response) => dispatch({ type: 'library/itemAdded', item: response.item })
      );
    },

    async addLibraryImage(name, mimeType, data) {
      if (!requireCapability('session.library.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.library.write',
        (current) =>
          api.library.addImage({
            generation: current,
            sessionId,
            scope: state.library.addScope,
            name,
            mimeType,
            data,
          }),
        (response) => dispatch({ type: 'library/itemAdded', item: response.item })
      );
    },

    async linkLibraryFile() {
      if (!requireCapability('session.library.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.library.write',
        (current) =>
          api.library.linkFile({
            generation: current,
            sessionId,
            scope: state.library.addScope,
            userGesture: true,
          }),
        (response) => {
          if (response.status === 'added') {
            dispatch({ type: 'library/itemAdded', item: response.item });
          }
        }
      );
    },

    async removeLibraryItem(itemId) {
      if (!requireCapability('session.library.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.library.write',
        (current) => api.library.remove({ generation: current, sessionId, itemId }),
        (response) => {
          if (response.removed) dispatch({ type: 'library/itemRemoved', itemId });
        }
      );
    },

    async readExtensions() {
      if (!requireCapability('session.extensions.read')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      dispatch({ type: 'extensions/loading' });
      await run(
        'session.extensions.read',
        async (current) => {
          const [availableResponse, sessionResponse] = await Promise.all([
            api.extensions.listAvailable({ generation: current }),
            api.extensions.listForSession({ generation: current, sessionId }),
          ]);
          return { availableResponse, sessionResponse };
        },
        ({ availableResponse, sessionResponse }) =>
          dispatch({
            type: 'extensions/loaded',
            available: availableResponse.extensions ?? [],
            selected: sessionResponse.extensions ?? [],
          })
      );
    },

    async addSessionExtension(extension) {
      if (!requireCapability('session.extensions.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.extensions.write',
        (current) => api.extensions.add({ generation: current, sessionId, extension }),
        (response) =>
          dispatch({
            type: 'extensions/loaded',
            available: state.extensions.available,
            selected: response.extensions ?? [],
          })
      );
    },

    async removeSessionExtension(name) {
      if (!requireCapability('session.extensions.write')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      if (!sessionId) return;
      await run(
        'session.extensions.write',
        (current) => api.extensions.remove({ generation: current, sessionId, name }),
        (response) =>
          dispatch({
            type: 'extensions/loaded',
            available: state.extensions.available,
            selected: response.extensions ?? [],
          })
      );
    },

    async submitPrompt() {
      if (!requireCapability('prompt.submit')) return;
      const sessionId = state.snapshot?.session?.sessionId;
      const text = state.draft;
      const libraryItemIds = [...state.library.selectedItemIds];
      if (!sessionId || (text.trim().length === 0 && libraryItemIds.length === 0)) return;
      const current = generation();
      if (current === null) return;
      dispatch({ type: 'pending/changed', pending: 'prompt.submit' });
      dispatch({ type: 'draft/changed', draft: '' });
      try {
        await api.prompt.submit({ generation: current, sessionId, text, libraryItemIds });
        if (disposed) return;
        dispatch({ type: 'library/selectionCleared' });
        dispatch({ type: 'failure/cleared' });
        dispatch({ type: 'pending/changed', pending: null });
      } catch (error) {
        if (disposed) return;
        const failure = asOperationFailure(error);
        // R-7: a failure that preserves the draft must put the exact submitted text back.
        if (failure.preservesDraft) dispatch({ type: 'draft/changed', draft: text });
        dispatch({ type: 'failure/raised', failure });
      }
    },

    async cancelPrompt() {
      if (!requireCapability('prompt.cancel')) return;
      const session = state.snapshot?.session;
      const promptAttemptId = session?.promptAttempt?.id;
      if (!session || !promptAttemptId) return;
      await run('prompt.cancel', (current) =>
        api.prompt.cancel({ generation: current, sessionId: session.sessionId, promptAttemptId })
      );
    },

    async respondPermission(actionId, allowOnce) {
      if (!requireCapability('permission.respond')) return;
      const interaction = state.interactions.find((entry) => entry.actionId === actionId);
      if (!interaction) return;
      dispatch({ type: 'interaction/responded', actionId });
      await run('interaction.respond', (current) =>
        api.permission.respond({
          generation: current,
          sessionId: interaction.sessionId,
          actionId,
          allowOnce,
        })
      );
    },

    async respondElicitation(actionId, action, fields) {
      if (!requireCapability('elicitation.respond')) return;
      const interaction = state.interactions.find((entry) => entry.actionId === actionId);
      if (!interaction) return;
      dispatch({ type: 'interaction/responded', actionId });
      await run('interaction.respond', (current) =>
        api.elicitation.respond({
          generation: current,
          sessionId: interaction.sessionId,
          actionId,
          action,
          ...(fields ? { fields } : {}),
        })
      );
    },

    async respondConfirmation(actionId, approve) {
      if (!requireCapability('confirmation.respond')) return;
      const interaction = state.interactions.find((entry) => entry.actionId === actionId);
      if (!interaction) return;
      dispatch({ type: 'interaction/responded', actionId });
      await run('interaction.respond', (current) =>
        api.domain.confirm({
          generation: current,
          sessionId: interaction.sessionId,
          actionId,
          approve,
        })
      );
    },

    async saveDiagnostics() {
      await run(
        'diagnostics.save',
        (current) => api.diagnostics.save({ generation: current, userGesture: true }),
        (result) => {
          if (result.status === 'saved') {
            dispatch({ type: 'diagnostics/saved', fileName: result.fileName });
          }
        }
      );
    },

    async prepareHandoff(question, requestedCapability) {
      const sessionId = state.snapshot?.session?.sessionId ?? '';
      // Main requires a non-empty session id and a live ACP connection. Refusing here — visibly,
      // rather than silently — keeps the shell from issuing a request it knows will be rejected.
      // See SHP-DEF-055.
      if (sessionId.length === 0 || state.snapshot?.identity === null) {
        dispatch({
          type: 'failure/raised',
          failure: {
            code: 'CAPABILITY_UNAVAILABLE',
            message: 'This shell cannot open Gosling without a live session.',
            retrySafe: false,
            recovery: 'none',
            preservesDraft: false,
          },
        });
        return;
      }
      await run(
        'handoff.prepare',
        (current) =>
          api.handoff.prepare({ generation: current, sessionId, question, requestedCapability }),
        (result) => dispatch({ type: 'handoff/prepared', handoff: result.handoff })
      );
    },

    async confirmHandoff() {
      const handoffId = state.handoff?.handoffId;
      if (!handoffId) return;
      await run(
        'handoff.confirm',
        (current) => api.handoff.confirm({ generation: current, handoffId }),
        () => dispatch({ type: 'handoff/cleared' })
      );
    },

    async retryRuntime() {
      await run('runtime.retry', (current) => api.runtime.retry({ generation: current }));
      await actions.refreshRuntime();
    },

    async stopRuntime() {
      await run('runtime.stop', (current) => api.runtime.stop({ generation: current }));
      await actions.refreshRuntime();
    },

    async updateAppearance(update) {
      await run(
        'settings.update',
        (current) => api.settings.updateAppearance({ generation: current, ...update }),
        (settings) => dispatch({ type: 'settings/replaced', settings })
      );
    },

    async selectModel(providerId, modelId) {
      await run(
        'settings.model.select',
        (current) => api.settings.selectModel({ generation: current, providerId, modelId }),
        (settings) => dispatch({ type: 'settings/replaced', settings })
      );
    },

    async resetSettings() {
      await run(
        'settings.reset',
        (current) => api.settings.reset({ generation: current, userGesture: true }),
        (settings) => dispatch({ type: 'settings/replaced', settings })
      );
    },
  };

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    dispatch,
    async start() {
      unsubscribes.push(
        api.runtime.onChanged((snapshot) => dispatch({ type: 'snapshot/replaced', snapshot })),
        api.session.onUpdated((update) => dispatch({ type: 'session/updated', update })),
        api.permission.onRequested((interaction) =>
          dispatch({ type: 'interaction/requested', interaction })
        ),
        api.elicitation.onRequested((interaction) =>
          dispatch({ type: 'interaction/requested', interaction })
        ),
        api.domain.onConfirmationRequested((interaction) =>
          dispatch({ type: 'interaction/requested', interaction })
        )
      );
      const [snapshot, settings] = await Promise.all([
        api.runtime.read().catch(() => null),
        api.settings.read().catch(() => null),
      ]);
      if (disposed) return;
      if (snapshot) dispatch({ type: 'snapshot/replaced', snapshot });
      if (settings) dispatch({ type: 'settings/replaced', settings });
    },
    dispose() {
      disposed = true;
      for (const unsubscribe of unsubscribes) unsubscribe();
      unsubscribes.length = 0;
      listeners.clear();
    },
    actions,
  };
}
