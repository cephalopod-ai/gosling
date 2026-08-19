import { describe, expect, it } from 'vitest';
import { shellUiReducer } from './reducer';
import { initialShellUiState, type ShellUiState } from './types';
import {
  confirmInteraction,
  permissionInteraction,
  settings,
  snapshot,
  update,
} from '../testSupport';

function withSnapshot(overrides = {}): ShellUiState {
  return shellUiReducer(initialShellUiState(), {
    type: 'snapshot/replaced',
    snapshot: snapshot(overrides),
  });
}

describe('generation fencing (R-1)', () => {
  it('ignores a snapshot from an older generation', () => {
    const state = withSnapshot({ generation: 4 });
    const next = shellUiReducer(state, {
      type: 'snapshot/replaced',
      snapshot: snapshot({ generation: 3 }),
    });
    expect(next).toBe(state);
  });

  it('ignores session updates and interactions below the current generation', () => {
    const state = withSnapshot({ generation: 4 });
    expect(
      shellUiReducer(state, { type: 'session/updated', update: update({ generation: 3 }) })
    ).toBe(state);
    expect(
      shellUiReducer(state, {
        type: 'interaction/requested',
        interaction: permissionInteraction({ generation: 3, expiresAtGeneration: 3 }),
      })
    ).toBe(state);
  });

  it('clears fenced state but keeps the draft and settings when the generation advances', () => {
    let state = withSnapshot({ generation: 1 });
    state = shellUiReducer(state, { type: 'settings/replaced', settings: settings() });
    state = shellUiReducer(state, { type: 'draft/changed', draft: 'unsent work' });
    state = shellUiReducer(state, { type: 'session/updated', update: update() });
    state = shellUiReducer(state, {
      type: 'interaction/requested',
      interaction: permissionInteraction(),
    });
    state = shellUiReducer(state, { type: 'sessions/loaded', items: [] });
    expect(state.transcript.updates).toHaveLength(1);
    expect(state.interactions).toHaveLength(1);

    const advanced = shellUiReducer(state, {
      type: 'snapshot/replaced',
      snapshot: snapshot({ generation: 2 }),
    });
    expect(advanced.transcript.updates).toHaveLength(0);
    expect(advanced.interactions).toHaveLength(0);
    expect(advanced.sessions.status).toBe('idle');
    expect(advanced.draft).toBe('unsent work');
    expect(advanced.settings).not.toBeNull();
  });
});

describe('interaction lifetime (R-5)', () => {
  it('keeps a pending interaction across streamed tool progress', () => {
    let state = withSnapshot();
    state = shellUiReducer(state, {
      type: 'interaction/requested',
      interaction: permissionInteraction(),
    });
    state = shellUiReducer(state, {
      type: 'session/updated',
      update: update({
        updateSeq: 2,
        kind: 'stream',
        stream: {
          type: 'tool',
          toolCallId: 't1',
          title: 'write',
          toolKind: 'edit',
          status: 'pending',
        },
      }),
    });
    expect(state.interactions).toHaveLength(1);
  });

  it.each(['completed', 'cancelled', 'failed'] as const)(
    'clears pending interactions on a %s outcome',
    (kind) => {
      let state = withSnapshot();
      state = shellUiReducer(state, {
        type: 'interaction/requested',
        interaction: permissionInteraction(),
      });
      state = shellUiReducer(state, {
        type: 'session/updated',
        update: update({ updateSeq: 3, kind }),
      });
      expect(state.interactions).toHaveLength(0);
    }
  );

  it('never re-adds an interaction that was already answered', () => {
    let state = withSnapshot();
    const interaction = permissionInteraction();
    state = shellUiReducer(state, { type: 'interaction/requested', interaction });
    state = shellUiReducer(state, {
      type: 'interaction/responded',
      actionId: interaction.actionId,
    });
    expect(state.interactions).toHaveLength(0);

    state = shellUiReducer(state, { type: 'interaction/requested', interaction });
    expect(state.interactions).toHaveLength(0);

    state = shellUiReducer(state, {
      type: 'snapshot/replaced',
      snapshot: snapshot({ pendingInteractions: [interaction] }),
    });
    expect(state.interactions).toHaveLength(0);
  });

  it('adds interactions the snapshot reports without pruning event-only ones', () => {
    let state = withSnapshot();
    const fromEvent = permissionInteraction({ actionId: 'from-event' });
    state = shellUiReducer(state, { type: 'interaction/requested', interaction: fromEvent });
    state = shellUiReducer(state, {
      type: 'snapshot/replaced',
      snapshot: snapshot({
        pendingInteractions: [confirmInteraction({ actionId: 'from-snapshot' })],
      }),
    });
    expect(state.interactions.map((entry) => entry.actionId)).toEqual([
      'from-event',
      'from-snapshot',
    ]);
  });

  it('expires an interaction whose expiresAtGeneration has passed', () => {
    let state = withSnapshot({ generation: 1 });
    state = shellUiReducer(state, {
      type: 'interaction/requested',
      interaction: permissionInteraction({ generation: 1, expiresAtGeneration: 1 }),
    });
    state = shellUiReducer(state, {
      type: 'snapshot/replaced',
      snapshot: snapshot({ generation: 2 }),
    });
    expect(state.interactions).toHaveLength(0);
  });

  it('ignores a duplicate request for the same action id', () => {
    let state = withSnapshot();
    state = shellUiReducer(state, {
      type: 'interaction/requested',
      interaction: permissionInteraction(),
    });
    const next = shellUiReducer(state, {
      type: 'interaction/requested',
      interaction: permissionInteraction(),
    });
    expect(next).toBe(state);
  });
});

describe('failures and notices', () => {
  it('records a failure and stops reporting a pending operation', () => {
    let state = withSnapshot();
    state = shellUiReducer(state, { type: 'pending/changed', pending: 'prompt.submit' });
    state = shellUiReducer(state, {
      type: 'failure/raised',
      failure: {
        code: 'SESSION_BUSY',
        message: 'Finish or stop the current task before continuing.',
        retrySafe: false,
        recovery: 'review_session',
        preservesDraft: true,
      },
    });
    expect(state.failure?.code).toBe('SESSION_BUSY');
    expect(state.pending).toBeNull();
  });

  it('treats a cancelled directory chooser as a non-error outcome (R-8)', () => {
    let state = withSnapshot();
    state = shellUiReducer(state, {
      type: 'failure/raised',
      failure: {
        code: 'OPERATION_FAILED',
        message: 'x',
        retrySafe: false,
        recovery: 'none',
        preservesDraft: false,
      },
    });
    state = shellUiReducer(state, { type: 'directory/cancelled' });
    expect(state.failure).toBeNull();
    expect(state.directoryCancelled).toBe(true);
    state = shellUiReducer(state, { type: 'notices/cleared' });
    expect(state.directoryCancelled).toBe(false);
  });

  it('records a session failure update as the active failure', () => {
    let state = withSnapshot();
    state = shellUiReducer(state, {
      type: 'session/updated',
      update: update({
        kind: 'failed',
        failure: {
          code: 'RUNTIME_UNAVAILABLE',
          message: 'The shell backend is not currently available.',
          retrySafe: true,
          recovery: 'retry',
          preservesDraft: true,
        },
      }),
    });
    expect(state.failure?.code).toBe('RUNTIME_UNAVAILABLE');
  });
});
