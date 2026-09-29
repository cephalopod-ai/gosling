import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toastService } from '../toasts';
import {
  formatExtensionErrorMessage,
  MAX_ERROR_MESSAGE_LENGTH,
  showExtensionLoadResults,
} from './extensionErrorUtils';

vi.mock('../toasts', () => ({
  toastService: {
    error: vi.fn(),
    extensionLoading: vi.fn(),
  },
}));

const EXIT_CAUSE =
  'process quit before initialization with exit status: 1 (connection closed: initialize response)';
const REFUSED_CAUSE =
  'failed to initialize MCP client: could not send initialize request: error sending request for url (http://127.0.0.1:9/mcp): Connection refused (os error 61)';

describe('formatExtensionErrorMessage', () => {
  it('shows a start-up cause longer than the old 70-character cut-off instead of the fallback', () => {
    expect(formatExtensionErrorMessage(EXIT_CAUSE, 'Failed to add extension')).toBe(EXIT_CAUSE);
    expect(formatExtensionErrorMessage(REFUSED_CAUSE, 'Failed to add extension')).toBe(
      REFUSED_CAUSE
    );
  });

  it('shortens a very long cause with an ellipsis rather than dropping it', () => {
    const long = `${REFUSED_CAUSE} ${'x'.repeat(400)}`;
    const shown = formatExtensionErrorMessage(long, 'Failed to add extension');
    expect(shown.length).toBeLessThanOrEqual(MAX_ERROR_MESSAGE_LENGTH);
    expect(shown.endsWith('…')).toBe(true);
    expect(shown.startsWith('failed to initialize MCP client: could not send')).toBe(true);
  });

  it('falls back only when there is no cause', () => {
    expect(formatExtensionErrorMessage('  ', 'Failed to add extension')).toBe(
      'Failed to add extension'
    );
  });
});

describe('showExtensionLoadResults', () => {
  beforeEach(() => {
    vi.mocked(toastService.error).mockClear();
  });

  it('puts the cause of a single failed extension in the toast message', () => {
    showExtensionLoadResults([{ name: 'broken', success: false, error: EXIT_CAUSE }]);

    expect(toastService.error).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'broken', msg: EXIT_CAUSE, traceback: EXIT_CAUSE })
    );
  });
});
