import { describe, expect, it, vi } from 'vitest';
import { defaultSettings, type Settings } from '../utils/settings';
import { SessionRecoveryRegistry } from './sessionRecoveryRegistry';

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
  it('persists and clears a running session recovery marker', () => {
    const { registry, settings } = harness();

    expect(registry.setActive(7, ' session-1 ', '/workspace', true)).toBe(true);
    expect(settings().pendingSessionRecoveries).toEqual([
      { sessionId: 'session-1', workingDir: '/workspace', startedAt: 456 },
    ]);

    expect(registry.setActive(7, 'session-1', '', false)).toBe(true);
    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('keeps the marker until the final attached window closes', () => {
    const { registry, settings } = harness();
    const recovery = { sessionId: 'session-1', workingDir: '/workspace', startedAt: 123 };

    registry.setActive(7, recovery.sessionId, recovery.workingDir, true);
    registry.attachPending(8, recovery);
    registry.setActive(7, recovery.sessionId, recovery.workingDir, false);
    expect(settings().pendingSessionRecoveries).toHaveLength(1);

    registry.clearWindow(8);
    expect(settings().pendingSessionRecoveries).toEqual([]);
  });

  it('rejects malformed renderer input without changing settings', () => {
    const { registry, settings, updateSettings } = harness();

    expect(registry.setActive(0, 'session-1', '/workspace', true)).toBe(false);
    expect(registry.setActive(7, '', '/workspace', true)).toBe(false);
    expect(registry.setActive(7, 'session-1', '', true)).toBe(false);
    expect(settings().pendingSessionRecoveries).toEqual([]);
    expect(updateSettings).not.toHaveBeenCalled();
  });

  it('clears every marker during a clean application shutdown', () => {
    const { registry, settings } = harness();
    registry.setActive(7, 'session-1', '/workspace', true);
    registry.setActive(8, 'session-2', '/other', true);

    registry.clearAll();

    expect(settings().pendingSessionRecoveries).toEqual([]);
  });
});
