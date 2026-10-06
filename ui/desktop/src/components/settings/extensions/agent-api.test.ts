import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toastService } from '../../../toasts';
import { addSessionExtension, removeSessionExtension } from '../../../acp/session-extensions';
import { getExtensionLoadFailures } from '../../../utils/extensionLoadFailures';
import type { ExtensionConfig } from '../../../types/extensions';
import { addToAgent, removeFromAgent } from './agent-api';

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

describe('removeFromAgent', () => {
  beforeEach(() => {
    vi.mocked(removeSessionExtension).mockReset();
    vi.mocked(toastService.success).mockClear();
    vi.mocked(toastService.error).mockClear();
  });

  it('waits for confirmed removal before showing success', async () => {
    let finishRemoval!: () => void;
    vi.mocked(removeSessionExtension).mockReturnValue(
      new Promise<void>((resolve) => {
        finishRemoval = resolve;
      })
    );
    const removal = removeFromAgent('Supabase', 'restoring-chat', true);
    expect(toastService.success).not.toHaveBeenCalled();
    expect(removeSessionExtension).toHaveBeenCalledWith('restoring-chat', 'Supabase');
    finishRemoval();
    await removal;
    expect(toastService.success).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Supabase' })
    );
  });

  it('shows busy-chat guidance and never reports removal success on refusal', async () => {
    const cause =
      'This chat is busy. Wait for the current operation to finish or stop its response before changing extensions.';
    vi.mocked(removeSessionExtension).mockRejectedValue({
      code: -32602,
      message: 'Invalid params',
      data: cause,
    });
    await expect(removeFromAgent('Supabase', 'busy-chat', true)).rejects.toBeDefined();
    expect(toastService.error).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Supabase', msg: cause, traceback: cause })
    );
    expect(toastService.success).not.toHaveBeenCalled();
  });
});
