import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toastService } from '../../../toasts';
import { addSessionExtension } from '../../../acp/session-extensions';
import { getExtensionLoadFailures } from '../../../utils/extensionLoadFailures';
import type { ExtensionConfig } from '../../../types/extensions';
import { addToAgent } from './agent-api';

vi.mock('../../../toasts', () => ({
  toastService: {
    loading: vi.fn(() => 1),
    dismiss: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('../../../acp/session-extensions', () => ({
  addSessionExtension: vi.fn(),
  removeSessionExtension: vi.fn(),
}));

const CAUSE =
  'process quit before initialization with exit status: 1 (connection closed: initialize response)';

const broken = {
  name: 'broken',
  type: 'stdio',
  cmd: 'false',
  args: [],
} as unknown as ExtensionConfig;

describe('addToAgent', () => {
  beforeEach(() => {
    vi.mocked(toastService.error).mockClear();
    vi.mocked(addSessionExtension).mockReset();
  });

  it('shows the backend cause, not "Internal error" or the generic fallback', async () => {
    vi.mocked(addSessionExtension).mockRejectedValue({
      code: -32603,
      message: 'Internal error',
      data: CAUSE,
    });

    await expect(addToAgent(broken, 'session-g133', true)).rejects.toBeDefined();

    expect(toastService.error).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'broken', msg: CAUSE, traceback: CAUSE })
    );
    expect(getExtensionLoadFailures('session-g133').get('broken')).toBe(CAUSE);
  });

  it('clears the failure mark once the extension is added', async () => {
    vi.mocked(addSessionExtension).mockRejectedValueOnce({
      code: -32603,
      message: 'Internal error',
      data: CAUSE,
    });
    await expect(addToAgent(broken, 'session-g133-retry', false)).rejects.toBeDefined();
    expect(getExtensionLoadFailures('session-g133-retry').size).toBe(1);

    vi.mocked(addSessionExtension).mockResolvedValueOnce(undefined);
    await addToAgent(broken, 'session-g133-retry', false);
    expect(getExtensionLoadFailures('session-g133-retry').size).toBe(0);
  });
});
