import { beforeEach, expect, it, vi } from 'vitest';
import { getAcpClient } from './acpConnection';
import {
  applyContextHistoryPolicy,
  getContextHistory,
  previewContextHistoryPolicy,
  setContextHistoryPinned,
} from './contextHistory';

vi.mock('./acpConnection', () => ({ getAcpClient: vi.fn() }));

const history = vi.fn();
const pin = vi.fn();
const preview = vi.fn();
const apply = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getAcpClient).mockResolvedValue({
    gosling: {
      sessionCompactionsHistory_unstable: history,
      sessionCompactionsPin_unstable: pin,
      contextHistoryPolicyPreview_unstable: preview,
      contextHistoryPolicyApply_unstable: apply,
    },
  } as never);
});

it('requests bounded history pages and can include grace-period snapshots', async () => {
  history.mockResolvedValue({ revisions: [], totalCount: 0, purgedCount: 0 });
  await getContextHistory('session-1', 12, true);
  expect(history).toHaveBeenCalledWith({
    sessionId: 'session-1',
    beforeGeneration: 12,
    includeExpired: true,
    limit: 50,
  });
});

it('sends explicit pin state', async () => {
  pin.mockResolvedValue({ revision: {} });
  await setContextHistoryPinned('session-1', 4, true);
  expect(pin).toHaveBeenCalledWith({ sessionId: 'session-1', generation: 4, pinned: true });
});

it('applies exactly the policy state that was previewed', async () => {
  const policy = {
    version: 1,
    captureEnabled: true,
    retentionDays: 90,
    purgeGraceDays: 7,
    maxRevisionsPerSession: 100,
    maxTotalBytes: 268435456,
  };
  preview.mockResolvedValue({ policy, previewHash: 'preview', impact: {} });
  apply.mockResolvedValue({ policy, cleanup: {} });
  const reviewed = await previewContextHistoryPolicy(policy);
  await applyContextHistoryPolicy(reviewed.policy, reviewed.previewHash);
  expect(apply).toHaveBeenCalledWith({ policy, expectedPreviewHash: 'preview' });
});
