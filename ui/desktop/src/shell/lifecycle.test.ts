import { describe, expect, it } from 'vitest';
import { initialShellLifecycle, transitionShellLifecycle } from './lifecycle';

function move(name: Parameters<typeof transitionShellLifecycle>[1]['name'], generation = 1) {
  return { generation, name, at: `2026-08-12T00:00:0${generation}Z` };
}

describe('shell lifecycle', () => {
  it('requires a positive generation and starts in booting', () => {
    expect(initialShellLifecycle(1, 'now')).toEqual({
      generation: 1,
      name: 'booting',
      enteredAt: 'now',
      allowedActions: ['stop'],
    });
    expect(() => initialShellLifecycle(0, 'now')).toThrow(/positive integer/);
  });

  it('executes the primary ready, busy, stop path', () => {
    let state = initialShellLifecycle(1, 'boot');
    for (const name of ['validating', 'ready', 'busy', 'ready', 'stopping', 'stopped'] as const) {
      const result = transitionShellLifecycle(state, move(name));
      expect(result).toMatchObject({ accepted: true, stale: false, illegal: false });
      state = result.state;
    }
    expect(state.name).toBe('stopped');
    expect(state.allowedActions).toEqual([]);
  });

  it.each(['degraded', 'offline'] as const)(
    '%s retries only through stop and a fresh generation',
    (failure) => {
      let state = initialShellLifecycle(1, 'boot');
      state = transitionShellLifecycle(state, move('validating')).state;
      state = transitionShellLifecycle(state, move(failure)).state;
      expect(state.allowedActions).toContain('retry');
      expect(transitionShellLifecycle(state, move('booting', 2))).toMatchObject({ illegal: true });
      state = transitionShellLifecycle(state, move('stopping')).state;
      state = transitionShellLifecycle(state, move('stopped')).state;
      const retry = transitionShellLifecycle(state, move('booting', 2));
      expect(retry).toMatchObject({ accepted: true, stale: false, illegal: false });
      expect(retry.state.generation).toBe(2);
    }
  );

  it.each(['relink_required', 'incompatible'] as const)(
    '%s allows diagnostics and handoff but not retry',
    (failure) => {
      let state = transitionShellLifecycle(
        initialShellLifecycle(1, 'boot'),
        move('validating')
      ).state;
      state = transitionShellLifecycle(state, move(failure)).state;
      expect(state.allowedActions).toEqual(['stop', 'diagnostics', 'handoff']);
    }
  );

  it('ignores stale generation events and rejects illegal same/future-generation events', () => {
    const state = transitionShellLifecycle(
      initialShellLifecycle(2, 'boot'),
      move('validating', 2)
    ).state;
    expect(transitionShellLifecycle(state, move('offline', 1))).toEqual({
      state,
      accepted: false,
      stale: true,
      illegal: false,
    });
    expect(transitionShellLifecycle(state, move('busy', 2))).toMatchObject({ illegal: true });
    expect(transitionShellLifecycle(state, move('ready', 3))).toMatchObject({ illegal: true });
  });

  it('does not retain an earlier reason when a later state has no reason', () => {
    let state = initialShellLifecycle(1, 'boot');
    state = transitionShellLifecycle(state, {
      ...move('validating'),
      reasonCode: 'VALIDATING',
    }).state;
    state = transitionShellLifecycle(state, move('ready')).state;
    expect(state.reasonCode).toBeUndefined();
  });
});
