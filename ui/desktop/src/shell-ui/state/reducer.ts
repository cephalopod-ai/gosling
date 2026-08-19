import type { ShellUiAction, ShellUiState } from './types';
import { appendTranscriptUpdate, emptyTranscript, transcriptFromSnapshot } from './transcript';

const MAX_RESPONDED_ACTION_IDS = 256;

function generationOf(state: ShellUiState): number | null {
  return state.snapshot?.generation ?? null;
}

/** R-1: anything carrying a generation below the snapshot's is dropped. */
function isStale(state: ShellUiState, generation: number): boolean {
  const current = generationOf(state);
  return current !== null && generation < current;
}

/**
 * R-1: a generation increase invalidates every generation-fenced projection. The draft and the
 * settings document are the only slices that survive, because `runtimeController.retry` restarts
 * the runtime rather than reconnecting it.
 */
function onGenerationAdvanced(state: ShellUiState): ShellUiState {
  return {
    ...state,
    transcript: emptyTranscript(),
    interactions: [],
    sessions: { status: 'idle', items: [] },
    respondedActionIds: [],
    pending: null,
    handoff: null,
    view: state.view === 'settings' ? 'settings' : 'workspace',
  };
}

function terminalKind(kind: string): boolean {
  return kind === 'completed' || kind === 'cancelled' || kind === 'failed';
}

export function shellUiReducer(state: ShellUiState, action: ShellUiAction): ShellUiState {
  switch (action.type) {
    case 'snapshot/replaced': {
      const previous = generationOf(state);
      const advanced = previous !== null && action.snapshot.generation > previous;
      if (previous !== null && action.snapshot.generation < previous) return state;
      const base = advanced ? onGenerationAdvanced(state) : state;
      const merged = mergeInteractions(
        base,
        action.snapshot.pendingInteractions,
        action.snapshot.generation
      );
      return { ...merged, snapshot: action.snapshot };
    }

    case 'settings/replaced':
      return { ...state, settings: action.settings };

    case 'transcript/loaded':
      if (isStale(state, action.transcript.generation)) return state;
      return { ...state, transcript: transcriptFromSnapshot(action.transcript) };

    case 'session/updated': {
      if (isStale(state, action.update.generation)) return state;
      const transcript = appendTranscriptUpdate(state.transcript, action.update);
      // R-5: only a terminal session outcome may clear pending interactions. A `stream` update
      // must never dismiss a permission, form, or confirmation the user has not answered.
      const interactions = terminalKind(action.update.kind)
        ? state.interactions.filter((entry) => entry.sessionId !== action.update.sessionId)
        : state.interactions;
      const failure =
        action.update.kind === 'failed' && action.update.failure
          ? action.update.failure
          : state.failure;
      return { ...state, transcript, interactions, failure };
    }

    case 'interaction/requested': {
      if (isStale(state, action.interaction.generation)) return state;
      if (state.respondedActionIds.includes(action.interaction.actionId)) return state;
      if (state.interactions.some((entry) => entry.actionId === action.interaction.actionId)) {
        return state;
      }
      return { ...state, interactions: [...state.interactions, action.interaction] };
    }

    case 'interaction/responded':
      return {
        ...state,
        interactions: state.interactions.filter((entry) => entry.actionId !== action.actionId),
        respondedActionIds: [action.actionId, ...state.respondedActionIds].slice(
          0,
          MAX_RESPONDED_ACTION_IDS
        ),
      };

    case 'sessions/loading':
      return { ...state, sessions: { status: 'loading', items: state.sessions.items } };

    case 'sessions/loaded':
      return { ...state, sessions: { status: 'loaded', items: action.items } };

    case 'draft/changed':
      return { ...state, draft: action.draft };

    case 'failure/raised':
      return { ...state, failure: action.failure, pending: null };

    case 'failure/cleared':
      return { ...state, failure: null };

    case 'pending/changed':
      return { ...state, pending: action.pending };

    case 'view/changed':
      return { ...state, view: action.view };

    case 'handoff/prepared':
      return { ...state, handoff: action.handoff, view: 'handoff' };

    case 'handoff/cleared':
      return { ...state, handoff: null, view: 'workspace' };

    case 'diagnostics/saved':
      return { ...state, savedDiagnosticsFile: action.fileName };

    case 'directory/cancelled':
      // R-8: cancel is a successful outcome, never an error.
      return { ...state, directoryCancelled: true, pending: null, failure: null };

    case 'notices/cleared':
      return { ...state, savedDiagnosticsFile: null, directoryCancelled: false };

    default:
      return state;
  }
}

/**
 * The snapshot may only *add* interactions. Pruning by snapshot membership would delete an
 * interaction that arrived on its own event channel before main published the snapshot that
 * contains it. Expiry is driven by `expiresAtGeneration`, which main stamps for exactly this.
 */
function mergeInteractions(
  state: ShellUiState,
  pending: ShellUiState['interactions'],
  generation: number
): ShellUiState {
  const responded = new Set(state.respondedActionIds);
  const retained = state.interactions.filter(
    (entry) => entry.expiresAtGeneration >= generation && !responded.has(entry.actionId)
  );
  const known = new Set(retained.map((entry) => entry.actionId));
  const additions = pending.filter(
    (entry) =>
      !known.has(entry.actionId) &&
      !responded.has(entry.actionId) &&
      entry.expiresAtGeneration >= generation
  );
  if (additions.length === 0 && retained.length === state.interactions.length) return state;
  return { ...state, interactions: [...retained, ...additions] };
}
