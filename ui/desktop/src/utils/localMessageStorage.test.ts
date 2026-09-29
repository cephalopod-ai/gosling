import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LocalMessageStorage } from './localMessageStorage';

let backendId = 'backend-a';

describe('backend-scoped composer history', () => {
  beforeEach(() => {
    backendId = 'backend-a';
    localStorage.clear();
    vi.stubGlobal('appConfig', { get: () => backendId });
  });
  afterEach(() => vi.unstubAllGlobals());

  it('isolates history and clearing, and retains the same backend history across reloads', () => {
    localStorage.setItem(
      'gosling-chat-history',
      JSON.stringify([{ content: 'Legacy unowned', timestamp: Date.now() }])
    );
    expect(LocalMessageStorage.getRecentMessages()).toEqual([]);
    LocalMessageStorage.addMessage('Private A');
    LocalMessageStorage.addMessage('Private A');
    backendId = 'backend-b';
    expect(LocalMessageStorage.getRecentMessages()).toEqual([]);
    LocalMessageStorage.addMessage('Private B');
    LocalMessageStorage.clearHistory();
    expect(LocalMessageStorage.getRecentMessages()).toEqual([]);
    backendId = 'backend-a';
    expect(LocalMessageStorage.getRecentMessages()).toEqual(['Private A']);
    expect(localStorage.getItem('gosling-chat-history')).toContain('Legacy unowned');
  });
});
