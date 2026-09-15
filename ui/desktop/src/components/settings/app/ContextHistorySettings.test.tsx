/**
 * @vitest-environment jsdom
 */
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import {
  applyContextHistoryPolicy,
  previewContextHistoryPolicy,
  readContextHistoryPolicy,
} from '../../../acp/contextHistory';
import ContextHistorySettings from './ContextHistorySettings';

vi.mock('../../../acp/contextHistory', () => ({
  readContextHistoryPolicy: vi.fn(),
  previewContextHistoryPolicy: vi.fn(),
  applyContextHistoryPolicy: vi.fn(),
}));

const policy = {
  version: 1,
  captureEnabled: true,
  retentionDays: 90,
  purgeGraceDays: 7,
  maxRevisionsPerSession: 100,
  maxTotalBytes: 268435456,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(readContextHistoryPolicy).mockResolvedValue({
    policy,
    stats: {
      revisionCount: 8,
      pinnedCount: 2,
      payloadBytes: 8192,
      pinnedBytes: 2048,
      purgedCount: 1,
    },
    managedByEnvironment: false,
  });
  vi.mocked(previewContextHistoryPolicy).mockResolvedValue({
    policy,
    previewHash: 'reviewed-state',
    impact: {
      current: {
        revisionCount: 8,
        pinnedCount: 2,
        payloadBytes: 8192,
        pinnedBytes: 2048,
        purgedCount: 1,
      },
      wouldExpireCount: 3,
      wouldPurgeNowCount: 1,
      wouldRemoveForLimitsCount: 0,
      projectedRevisionCount: 7,
      projectedPayloadBytes: 7168,
      projectedOverBudgetBytes: 0,
      warnings: [],
    },
  });
  vi.mocked(applyContextHistoryPolicy).mockResolvedValue({
    policy,
    cleanup: {
      deletedCount: 1,
      deletedBytes: 1024,
      remaining: {
        revisionCount: 7,
        pinnedCount: 2,
        payloadBytes: 7168,
        pinnedBytes: 2048,
        purgedCount: 2,
      },
    },
  });
});

it('previews retention impact before applying it', async () => {
  const user = userEvent.setup();
  render(<ContextHistorySettings />, { wrapper: IntlTestWrapper });

  await user.click(await screen.findByRole('button', { name: 'Review changes' }));
  expect(await screen.findByText(/3 will be marked expired, 1 will be deleted now/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Apply policy' }));
  expect(applyContextHistoryPolicy).toHaveBeenCalledWith(policy, 'reviewed-state');
});

it('reloads the effective policy when apply fails after a save', async () => {
  const user = userEvent.setup();
  vi.mocked(applyContextHistoryPolicy).mockRejectedValue(
    new Error('Policy was saved but cleanup could not be confirmed')
  );
  render(<ContextHistorySettings />, { wrapper: IntlTestWrapper });

  await screen.findByRole('button', { name: 'Review changes' });
  vi.mocked(readContextHistoryPolicy).mockResolvedValueOnce({
    policy: { ...policy, retentionDays: 30 },
    stats: {
      revisionCount: 8,
      pinnedCount: 2,
      payloadBytes: 8192,
      pinnedBytes: 2048,
      purgedCount: 1,
    },
    managedByEnvironment: false,
  });
  await user.click(screen.getByRole('button', { name: 'Review changes' }));
  await user.click(await screen.findByRole('button', { name: 'Apply policy' }));

  await waitFor(() => expect(readContextHistoryPolicy).toHaveBeenCalledTimes(2));
  expect(screen.getByRole('alert')).toHaveTextContent('cleanup could not be confirmed');
  expect(screen.getByRole('spinbutton', { name: 'Keep for (days)' })).toHaveValue(30);
});
