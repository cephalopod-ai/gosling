import type { ShellUiState } from './types';

/**
 * Screen identifiers from `docs/build/shell-productization/gui/gate-1-product-workflow-design.md`
 * section 4. The route is always derived from the snapshot; it is never assigned independently.
 */
export type ShellRoute =
  | 'S-01'
  | 'S-02'
  | 'S-03'
  | 'S-04'
  | 'S-05'
  | 'S-06'
  | 'S-14'
  | 'S-15'
  | 'S-16'
  | 'S-17'
  | 'S-18'
  | 'S-19'
  | 'S-20'
  | 'S-21'
  | 'S-23'
  | 'S-24'
  | 'S-28';

const LIFECYCLE_ROUTES: Partial<Record<string, ShellRoute>> = {
  booting: 'S-01',
  validating: 'S-02',
  degraded: 'S-14',
  relink_required: 'S-15',
  incompatible: 'S-16',
  offline: 'S-17',
  stopping: 'S-19',
  stopped: 'S-20',
  fatal: 'S-18',
};

export function selectRoute(state: ShellUiState): ShellRoute {
  const snapshot = state.snapshot;
  if (!snapshot) return 'S-01';

  const lifecycleRoute = LIFECYCLE_ROUTES[snapshot.lifecycleState];
  if (lifecycleRoute) return lifecycleRoute;

  if (state.view === 'handoff' && state.handoff) return 'S-28';
  if (state.view === 'settings') return 'S-21';

  const directory = snapshot.directory;
  if (directory.state === 'missing' || directory.state === 'invalid') return 'S-23';
  if (directory.state === 'unselected') return 'S-03';

  const credentials = snapshot.credentials;
  if (
    credentials.selectionStatus === 'missing' ||
    credentials.selectionStatus === 'relink_required'
  ) {
    return 'S-24';
  }
  if (credentials.selectionStatus === 'none' && credentials.catalogStatus === 'available') {
    return 'S-04';
  }

  const session = snapshot.session;
  if (!session || session.status === 'none') return 'S-05';
  if (state.view === 'sessions') return 'S-05';
  return 'S-06';
}

export function canChangeDirectory(state: ShellUiState): boolean {
  const session = state.snapshot?.session;
  return !session || session.status === 'none';
}

export function isDeclared(state: ShellUiState, capability: string): boolean {
  return state.snapshot?.declaredCapabilities.includes(capability) ?? false;
}

/**
 * `handoff.prepare` needs a live ACP connection and a non-empty session id: main rejects an empty
 * one, and the envelope is server-prepared. `identity` is populated only while the ACP connection
 * exists, so it is the renderer's faithful proxy for that. The host still lists `handoff` in
 * `allowedActions` for `relink_required` and `incompatible`, where both preconditions are absent —
 * see SHP-DEF-055 — so the UI must not render a control that cannot work.
 */
export function canHandOff(state: ShellUiState): boolean {
  const snapshot = state.snapshot;
  if (!snapshot) return false;
  return (
    snapshot.allowedActions.includes('handoff') &&
    snapshot.identity !== null &&
    (snapshot.session?.sessionId ?? '').length > 0
  );
}

export function lifecycleAllows(
  state: ShellUiState,
  action: 'retry' | 'stop' | 'diagnostics' | 'handoff'
): boolean {
  return state.snapshot?.allowedActions.includes(action) ?? false;
}
