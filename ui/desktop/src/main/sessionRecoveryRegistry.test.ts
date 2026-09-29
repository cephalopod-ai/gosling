import { describe, expect, it, vi } from 'vitest';
import { defaultSettings, type Settings } from '../utils/settings';
import { recoverySessionIdForBackend, SessionRecoveryRegistry } from './sessionRecoveryRegistry';

function harness() {
  let settings: Settings = {
    ...defaultSettings,
    pendingSessionRecoveries: [],
  };
  const updateSettings = vi.fn((modifier: (settings: Settings) => void) => {
    modifier(settings);
    settings = { ...settings };
  });
  const registry = new SessionRecoveryRegistry(
    () => settings,
    updateSettings,
    () => 456
  );
  return { registry, settings: () => settings, updateSettings };
}

describe('SessionRecoveryRegistry', () => {
  it('refuses a recovery route after startup falls back to a different backend', () => {
    const marker = {
      backendId: 'external-a',
      sessionId: 'same-id',
      workingDir: '/a',
      startedAt: 123,
    };
    expect(recoverySessionIdForBackend(marker, 'external-a')).toBe('same-id');
    expect(recoverySessionIdForBackend(marker, 'local-b')).toBeUndefined();
    expect(
      recoverySessionIdForBackend({ ...marker, backendId: undefined }, 'local-b')
    ).toBeUndefined();
  });

  it('persists and clears a running session recovery marker', () => {
    const { registry, settings } = harness();

    expect(registry.setActive(7, ' session-1 ', '/workspace', true, 'backend-a')).toBe(true);
    expect(settings().pendingSessionRecoveries).toEqual([
      { backendId: 'backend-a', sessionId: 'session-1', workingDir: '/workspace', startedAt: 456 },
    ]);

    expect(registry.setActive(7, 'session-1', '', false, 'backend-a')).toBe(true);
    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('keeps the marker until the final attached window closes', () => {
    const { registry, settings } = harness();
    const recovery = {
      backendId: 'backend-a',
      sessionId: 'session-1',
      workingDir: '/workspace',
      startedAt: 123,
    };

    registry.setActive(7, recovery.sessionId, recovery.workingDir, true, 'backend-a');
    registry.attachPending(8, recovery);
    registry.setActive(7, recovery.sessionId, recovery.workingDir, false, 'backend-a');
    expect(settings().pendingSessionRecoveries).toHaveLength(1);

    registry.clearWindow(8);
    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('rejects malformed renderer input without changing settings', () => {
    const { registry, settings, updateSettings } = harness();

    expect(registry.setActive(0, 'session-1', '/workspace', true, 'backend-a')).toBe(false);
    expect(registry.setActive(7, '', '/workspace', true, 'backend-a')).toBe(false);
    expect(registry.setActive(7, 'session-1', '', true, 'backend-a')).toBe(false);
    expect(settings().pendingSessionRecoveries).toEqual([]);
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it('clears every marker during a clean application shutdown', () => {
    const { registry, settings } = harness();
    registry.setActive(7, 'session-1', '/workspace', true, 'backend-a');
    registry.setActive(8, 'session-2', '/other', true, 'backend-a');

    registry.clearAll();

    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('keeps matching session IDs isolated during activity, restore, and window cleanup', () => {
    const { registry, settings } = harness();
    registry.setActive(7, 'same-id', '/a', true, 'backend-a');
    registry.setActive(8, 'same-id', '/b', true, 'backend-b');
    expect(settings().pendingSessionRecoveries).toHaveLength(2);
    const a = registry.pendingForBackend(() => 'backend-a');
    expect(a).toEqual([
      { backendId: 'backend-a', sessionId: 'same-id', workingDir: '/a', startedAt: 456 },
    ]);
    registry.attachPending(9, a[0]);
    registry.clearWindow(7);
    expect(settings().pendingSessionRecoveries).toHaveLength(2);
    registry.setActive(8, 'same-id', '', false, 'backend-b');
    expect(settings().pendingSessionRecoveries).toEqual(a);
    registry.clearWindow(9);
    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('preserves but never replays legacy or other-backend markers on clean shutdown', () => {
    const { registry, settings, updateSettings } = harness();
    const legacy = { sessionId: 'same-id', workingDir: '/legacy', startedAt: 123 };
    const other = {
      backendId: 'backend-b',
      sessionId: 'same-id',
      workingDir: '/b',
      startedAt: 123,
    };
    updateSettings((stored) => {
      stored.pendingSessionRecoveries = [legacy, other];
    });
    registry.setActive(7, 'same-id', '/a', true, 'backend-a');
    expect(registry.pendingForBackend(() => 'backend-a')).toHaveLength(1);
    registry.attachPending(8, legacy);
    registry.clearAll();
    expect(settings().pendingSessionRecoveries).toEqual([legacy, other]);
    expect(registry.pendingForBackend(() => 'backend-a')).toEqual([]);
    expect(registry.pendingForBackend(() => 'backend-b')).toEqual([other]);
  });
});
