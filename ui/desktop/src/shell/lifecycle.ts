export type ShellLifecycleStateName =
  | 'booting'
  | 'validating'
  | 'ready'
  | 'busy'
  | 'degraded'
  | 'relink_required'
  | 'incompatible'
  | 'offline'
  | 'stopping'
  | 'stopped'
  | 'fatal';

export type ShellLifecycleAction = 'retry' | 'stop' | 'diagnostics' | 'handoff';

export interface ShellLifecycleState {
  generation: number;
  name: ShellLifecycleStateName;
  enteredAt: string;
  reasonCode?: string;
  allowedActions: ShellLifecycleAction[];
}

export interface ShellLifecycleEvent {
  generation: number;
  name: ShellLifecycleStateName;
  at: string;
  reasonCode?: string;
}

export interface ShellLifecycleResult {
  state: ShellLifecycleState;
  accepted: boolean;
  stale: boolean;
  illegal: boolean;
}

const transitions: Record<ShellLifecycleStateName, ReadonlySet<ShellLifecycleStateName>> = {
  booting: new Set(['validating', 'offline', 'stopping', 'fatal']),
  validating: new Set([
    'ready',
    'degraded',
    'relink_required',
    'incompatible',
    'offline',
    'stopping',
    'fatal',
  ]),
  ready: new Set([
    'busy',
    'degraded',
    'relink_required',
    'incompatible',
    'offline',
    'stopping',
    'fatal',
  ]),
  busy: new Set([
    'ready',
    'degraded',
    'relink_required',
    'incompatible',
    'offline',
    'stopping',
    'fatal',
  ]),
  degraded: new Set(['stopping', 'fatal']),
  relink_required: new Set(['stopping', 'fatal']),
  incompatible: new Set(['stopping', 'fatal']),
  offline: new Set(['stopping', 'fatal']),
  stopping: new Set(['stopped', 'fatal']),
  stopped: new Set(['booting']),
  fatal: new Set(['stopping']),
};

const actions: Record<ShellLifecycleStateName, ShellLifecycleAction[]> = {
  booting: ['stop'],
  validating: ['stop', 'diagnostics'],
  ready: ['stop', 'diagnostics', 'handoff'],
  busy: ['stop', 'diagnostics'],
  degraded: ['retry', 'stop', 'diagnostics', 'handoff'],
  relink_required: ['stop', 'diagnostics', 'handoff'],
  incompatible: ['stop', 'diagnostics', 'handoff'],
  offline: ['retry', 'stop', 'diagnostics'],
  stopping: ['diagnostics'],
  stopped: [],
  fatal: ['stop', 'diagnostics'],
};

export function initialShellLifecycle(generation: number, at: string): ShellLifecycleState {
  if (!Number.isSafeInteger(generation) || generation < 1) {
    throw new Error('shell lifecycle generation must be a positive integer');
  }
  return { generation, name: 'booting', enteredAt: at, allowedActions: [...actions.booting] };
}

export function transitionShellLifecycle(
  state: ShellLifecycleState,
  event: ShellLifecycleEvent
): ShellLifecycleResult {
  if (event.generation < state.generation) {
    return { state, accepted: false, stale: true, illegal: false };
  }
  const freshGeneration = event.generation > state.generation;
  const legalFreshGeneration =
    freshGeneration && state.name === 'stopped' && event.name === 'booting';
  if (
    (!freshGeneration && !transitions[state.name].has(event.name)) ||
    (freshGeneration && !legalFreshGeneration)
  ) {
    return { state, accepted: false, stale: false, illegal: true };
  }
  return {
    state: {
      generation: event.generation,
      name: event.name,
      enteredAt: event.at,
      ...(event.reasonCode ? { reasonCode: event.reasonCode } : {}),
      allowedActions: [...actions[event.name]],
    },
    accepted: true,
    stale: false,
    illegal: false,
  };
}
