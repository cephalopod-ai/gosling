import { describe, expect, it } from 'vitest';
import { canChangeDirectory, isDeclared, selectRoute } from './route';
import { shellUiReducer } from './reducer';
import { initialShellUiState, type ShellUiState } from './types';
import { activeSession, snapshot } from '../testSupport';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import type { ShellLifecycleStateName } from '../../shell/lifecycle';

function stateWith(overrides: Partial<ShellRuntimeSnapshot>): ShellUiState {
  return shellUiReducer(initialShellUiState(), {
    type: 'snapshot/replaced',
    snapshot: snapshot(overrides),
  });
}

const LIFECYCLE_EXPECTATIONS: Array<[ShellLifecycleStateName, string]> = [
  ['booting', 'S-01'],
  ['validating', 'S-02'],
  ['degraded', 'S-14'],
  ['relink_required', 'S-15'],
  ['incompatible', 'S-16'],
  ['offline', 'S-17'],
  ['fatal', 'S-18'],
  ['stopping', 'S-19'],
  ['stopped', 'S-20'],
];

describe('route derivation', () => {
  it('shows the boot screen before any snapshot exists', () => {
    expect(selectRoute(initialShellUiState())).toBe('S-01');
  });

  it.each(LIFECYCLE_EXPECTATIONS)('maps lifecycle %s to %s', (lifecycleState, route) => {
    expect(selectRoute(stateWith({ lifecycleState }))).toBe(route);
  });

  it('covers every lifecycle state name', () => {
    const covered = new Set<ShellLifecycleStateName>(
      LIFECYCLE_EXPECTATIONS.map(([lifecycleState]) => lifecycleState)
    );
    covered.add('ready');
    covered.add('busy');
    const all: ShellLifecycleStateName[] = [
      'booting',
      'validating',
      'ready',
      'busy',
      'degraded',
      'relink_required',
      'incompatible',
      'offline',
      'stopping',
      'stopped',
      'fatal',
    ];
    expect(all.filter((name) => !covered.has(name))).toEqual([]);
  });

  it('prioritises a missing folder over a session', () => {
    const state = stateWith({
      directory: {
        state: 'missing',
        path: '/gone',
        label: 'gone',
        reasonCode: 'directory_not_found',
        remembered: true,
      },
    });
    expect(selectRoute(state)).toBe('S-23');
  });

  it('asks for a folder when nothing is selected', () => {
    const state = stateWith({
      directory: {
        state: 'unselected',
        path: null,
        label: null,
        reasonCode: null,
        remembered: false,
      },
    });
    expect(selectRoute(state)).toBe('S-03');
  });

  it('asks for an account only when the catalog is readable', () => {
    const selectable = stateWith({
      credentials: {
        catalogStatus: 'available',
        profiles: [],
        selectedProfileId: null,
        selectionStatus: 'none',
      },
    });
    expect(selectRoute(selectable)).toBe('S-04');

    const fixed = stateWith({
      credentials: {
        catalogStatus: 'denied',
        profiles: [],
        selectedProfileId: null,
        selectionStatus: 'none',
      },
      session: null,
    });
    expect(selectRoute(fixed)).toBe('S-05');
  });

  it.each(['relink_required', 'missing'] as const)(
    'routes a %s credential selection to the account problem screen',
    (selectionStatus) => {
      const state = stateWith({
        credentials: {
          catalogStatus: 'available',
          profiles: [],
          selectedProfileId: 'cred-9',
          selectionStatus,
        },
      });
      expect(selectRoute(state)).toBe('S-24');
    }
  );

  it('shows the session picker when no session is active', () => {
    expect(selectRoute(stateWith({ session: null }))).toBe('S-05');
    expect(
      selectRoute(stateWith({ session: activeSession({ status: 'none', sessionId: '' }) }))
    ).toBe('S-05');
  });

  it('shows the conversation once a session is active', () => {
    expect(selectRoute(stateWith({}))).toBe('S-06');
  });

  it('lets the settings view win over the workspace but not over a lifecycle failure', () => {
    const ready = { ...stateWith({}), view: 'settings' as const };
    expect(selectRoute(ready)).toBe('S-21');
    const failed = { ...stateWith({ lifecycleState: 'fatal' }), view: 'settings' as const };
    expect(selectRoute(failed)).toBe('S-18');
  });
});

describe('capability and directory guards', () => {
  it('reports declared capabilities from the snapshot', () => {
    const state = stateWith({ declaredCapabilities: ['prompt.submit'] });
    expect(isDeclared(state, 'prompt.submit')).toBe(true);
    expect(isDeclared(state, 'domain.action')).toBe(false);
  });

  it('reports no capability when the snapshot is absent', () => {
    expect(isDeclared(initialShellUiState(), 'prompt.submit')).toBe(false);
  });

  it('blocks changing folder while a session is held', () => {
    expect(canChangeDirectory(stateWith({}))).toBe(false);
    expect(canChangeDirectory(stateWith({ session: null }))).toBe(true);
  });
});
