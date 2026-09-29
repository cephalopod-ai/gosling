import { describe, expect, it } from 'vitest';
import {
  clearExtensionFailure,
  getCurrentExtensionSessionId,
  getExtensionLoadFailures,
  extensionFailureKey,
  recordExtensionFailure,
  recordExtensionLoadResults,
  setCurrentExtensionSession,
} from './extensionLoadFailures';

describe('extensionLoadFailures', () => {
  it('keeps the cause of each failed extension per chat and replaces it on the next load', () => {
    recordExtensionLoadResults('s-load', [
      { name: 'developer', success: true },
      { name: 'Broken Tool', success: false, error: 'process quit with exit status: 1' },
    ]);

    const failures = getExtensionLoadFailures('s-load');
    expect(failures.size).toBe(1);
    expect(failures.get(extensionFailureKey('brokentool'))).toBe(
      'process quit with exit status: 1'
    );
    expect(getExtensionLoadFailures('another-chat').size).toBe(0);

    recordExtensionLoadResults('s-load', [{ name: 'Broken Tool', success: true }]);
    expect(getExtensionLoadFailures('s-load').size).toBe(0);
  });

  it('leaves a chat untouched when a response carries no load results', () => {
    recordExtensionLoadResults('s-missing', [{ name: 'broken', success: false, error: 'boom' }]);
    recordExtensionLoadResults('s-missing', undefined);
    expect(getExtensionLoadFailures('s-missing').get('broken')).toBe('boom');
  });

  it('records a failed add and clears it once the extension starts', () => {
    recordExtensionFailure('s-toggle', 'broken', 'Connection refused');
    expect(getExtensionLoadFailures('s-toggle').get('broken')).toBe('Connection refused');

    clearExtensionFailure('s-toggle', 'broken');
    expect(getExtensionLoadFailures('s-toggle').size).toBe(0);
  });

  it('tracks the chat most recently opened in the window', () => {
    setCurrentExtensionSession('s-a');
    setCurrentExtensionSession('s-b');
    expect(getCurrentExtensionSessionId()).toBe('s-b');
  });
});
