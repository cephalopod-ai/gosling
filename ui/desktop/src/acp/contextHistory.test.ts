import { beforeEach, expect, it, vi } from 'vitest';
import type { Message } from '../types/message';
import { getAcpClient } from './acpConnection';
import {
  applyContextHistoryPolicy,
  getContextHistory,
  getContextHistorySourceMessages,
  previewContextHistoryPolicy,
  setContextHistoryPinned,
} from './contextHistory';
import { acpListSessionMessages } from './sessions';

vi.mock('./acpConnection', () => ({ getAcpClient: vi.fn() }));
vi.mock('./sessions', () => ({ acpListSessionMessages: vi.fn() }));

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

it('resolves recorded source IDs lazily and stops at the oldest source message', async () => {
  const sourceTwo: Message = {
    id: 'message-2',
    role: 'user',
    created: 2,
    content: [{ type: 'text', text: 'second' }],
    metadata: { userVisible: true, agentVisible: false },
  };
  const sourceFour: Message = {
    id: 'message-4',
    role: 'assistant',
    created: 4,
    content: [{ type: 'text', text: 'fourth' }],
    metadata: { userVisible: true, agentVisible: false },
  };
  vi.mocked(acpListSessionMessages)
    .mockResolvedValueOnce({
      messages: [
        {
          ...sourceFour,
          id: 'message-8',
        },
      ],
      nextBeforeCursor: '8',
      totalCount: 8,
    })
    .mockResolvedValueOnce({
      messages: [sourceTwo, sourceFour],
      nextBeforeCursor: '2',
      totalCount: 8,
    });

  const result = await getContextHistorySourceMessages('session-1', ['message-2', 'message-4'], 3);

  expect(acpListSessionMessages).toHaveBeenCalledTimes(2);
  expect(result).toEqual({ messages: [sourceTwo, sourceFour], unavailableCount: 1 });
});

it('does not load transcript pages when a snapshot recorded no stable source IDs', async () => {
  await expect(getContextHistorySourceMessages('session-1', [], 2)).resolves.toEqual({
    messages: [],
    unavailableCount: 2,
  });
  expect(acpListSessionMessages).not.toHaveBeenCalled();
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
