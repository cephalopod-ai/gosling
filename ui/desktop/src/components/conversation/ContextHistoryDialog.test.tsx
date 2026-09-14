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
  getContextHistorySourceMessages,
  setContextHistoryPinned,
} from '../../acp/contextHistory';

vi.mock('../../acp/contextHistory', () => ({
  getContextHistory: vi.fn(),
  getContextHistoryRevision: vi.fn(),
  getContextHistorySourceMessages: vi.fn(),
  setContextHistoryPinned: vi.fn(),
  deleteContextHistoryRevision: vi.fn(),
  purgeContextHistory: vi.fn(),
}));

const listItem = {
  revisionId: 'revision-2',
  generation: 2,
  trigger: 'automatic_threshold' as const,
  effect: 'durable' as const,
  sourceMessageCount: 2,
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

const previousListItem = {
  ...listItem,
  revisionId: 'revision-1',
  generation: 1,
  parentRevisionId: null,
  estimatedTokensBefore: 900,
  estimatedTokensAfter: 250,
  createdAt: '2026-09-14T11:00:00Z',
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

const previousRevision = {
  ...revision,
  ...previousListItem,
  sessionId: 'session-1',
  parentRevisionId: null,
  summary: 'Exact compacted summary for the earlier walkthrough.',
  sourceMessageIds: ['message-1', 'message-4'],
};

const sourceMessages = [
  {
    id: 'message-1',
    role: 'user' as const,
    created: 1_757_851_200,
    content: [{ type: 'text' as const, text: 'Original brainstorming prompt.' }],
    metadata: { userVisible: true, agentVisible: false },
  },
  {
    id: 'message-8',
    role: 'assistant' as const,
    created: 1_757_851_260,
    content: [{ type: 'text' as const, text: 'Original coding response.' }],
    metadata: { userVisible: true, agentVisible: false },
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(getContextHistory).mockResolvedValue({
    revisions: [listItem, previousListItem],
    nextBeforeGeneration: null,
    totalCount: 2,
    purgedCount: 0,
  });
  vi.mocked(getContextHistoryRevision).mockImplementation(async (_sessionId, generation) => ({
    revision: generation === 2 ? revision : previousRevision,
  }));
  vi.mocked(getContextHistorySourceMessages).mockResolvedValue({
    messages: sourceMessages,
    unavailableCount: 0,
  });
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

it('shows only changed summary content with selectable word and line detail', async () => {
  const user = userEvent.setup();
  render(<ContextHistoryDialog sessionId="session-1" open onOpenChange={vi.fn()} />, {
    wrapper: IntlTestWrapper,
  });

  await screen.findByText('Exact compacted summary for the walkthrough.');
  await user.click(screen.getByRole('checkbox', { name: 'Compare with previous available' }));

  const diff = await screen.findByRole('region', {
    name: 'Changes from snapshot #1 to #2',
  });
  expect(diff).toHaveTextContent('Unchanged content hidden');
  expect(diff).toHaveTextContent('earlier');
  expect(screen.queryByText('Previous summary')).not.toBeInTheDocument();
  expect(screen.getByRole('radio', { name: 'Words' })).toBeChecked();
  await user.click(screen.getByRole('radio', { name: 'Lines' }));
  expect(screen.getByRole('radio', { name: 'Lines' })).toBeChecked();
});

it('loads and displays source messages only after the action is selected', async () => {
  const user = userEvent.setup();
  render(<ContextHistoryDialog sessionId="session-1" open onOpenChange={vi.fn()} />, {
    wrapper: IntlTestWrapper,
  });

  await screen.findByText('Exact compacted summary for the walkthrough.');
  expect(getContextHistorySourceMessages).not.toHaveBeenCalled();

  await user.click(screen.getByRole('button', { name: 'View source messages' }));

  expect(getContextHistorySourceMessages).toHaveBeenCalledWith(
    'session-1',
    ['message-1', 'message-8'],
    2
  );
  expect(await screen.findByText('Original brainstorming prompt.')).toBeVisible();
  expect(screen.getByText('Original coding response.')).toBeVisible();
  expect(screen.getByText('Showing 2 of 2 source messages.')).toBeVisible();
  expect(screen.getByRole('button', { name: 'Hide source messages' })).toBeVisible();
});
