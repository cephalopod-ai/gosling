/**
 * @vitest-environment jsdom
 */
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { ContextHistoryDialog } from './ContextHistoryDialog';
import {
  getContextHistory,
  getContextHistoryRevision,
  setContextHistoryPinned,
} from '../../acp/contextHistory';

vi.mock('../../acp/contextHistory', () => ({
  getContextHistory: vi.fn(),
  getContextHistoryRevision: vi.fn(),
  setContextHistoryPinned: vi.fn(),
  deleteContextHistoryRevision: vi.fn(),
  purgeContextHistory: vi.fn(),
}));

const listItem = {
  revisionId: 'revision-2',
  generation: 2,
  trigger: 'automatic_threshold' as const,
  effect: 'durable' as const,
  sourceMessageCount: 8,
  summaryHash: 'summary-hash',
  provider: 'openai',
  selectedModel: 'gpt-selected',
  resolvedModel: 'gpt-resolved',
  estimatedTokensBefore: 1200,
  estimatedTokensAfter: 300,
  createdAt: '2026-09-14T12:00:00Z',
  expiresAt: '2026-12-13T12:00:00Z',
  purgeAfter: '2026-12-20T12:00:00Z',
  pinnedAt: null,
  expired: false,
  payloadBytes: 400,
};

const revision = {
  ...listItem,
  sessionId: 'session-1',
  parentRevisionId: 'revision-1',
  firstSourceMessageId: 'message-1',
  lastSourceMessageId: 'message-8',
  sourceHash: 'source-hash',
  promptHash: 'prompt-hash',
  usage: {},
  summary: 'Exact compacted summary for the walkthrough.',
  sourceMessageIds: ['message-1', 'message-8'],
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getContextHistory).mockResolvedValue({
    revisions: [listItem],
    nextBeforeGeneration: null,
    totalCount: 1,
    purgedCount: 0,
  });
  vi.mocked(getContextHistoryRevision).mockResolvedValue({ revision });
  vi.mocked(setContextHistoryPinned).mockResolvedValue({
    revision: { ...revision, pinnedAt: '2026-09-14T12:05:00Z', expiresAt: null, purgeAfter: null },
  });
});

it('shows the exact saved summary and pins the selected snapshot', async () => {
  const user = userEvent.setup();
  render(<ContextHistoryDialog sessionId="session-1" open onOpenChange={vi.fn()} />, {
    wrapper: IntlTestWrapper,
  });

  expect(await screen.findByText('Exact compacted summary for the walkthrough.')).toBeVisible();
  expect(screen.getByText('1,200 → 300 estimated tokens (900 removed)')).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Pin' }));
  expect(setContextHistoryPinned).toHaveBeenCalledWith('session-1', 2, true);
  expect(await screen.findByRole('button', { name: 'Unpin' })).toBeVisible();
});
