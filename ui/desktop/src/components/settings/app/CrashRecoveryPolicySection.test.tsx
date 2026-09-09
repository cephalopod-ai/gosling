import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import CrashRecoveryPolicySection from './CrashRecoveryPolicySection';

describe('CrashRecoveryPolicySection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(window.electron.getSetting).mockResolvedValue('safe');
    vi.mocked(window.electron.setSetting).mockResolvedValue();
  });

  it('defaults to Safe and explains the recovery tradeoffs', async () => {
    render(<CrashRecoveryPolicySection />, { wrapper: IntlTestWrapper });

    expect(await screen.findByRole('button', { name: /Safe \(recommended\)/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    expect(screen.getByText(/cannot restore the exact interrupted provider stream/)).toBeVisible();
    expect(screen.getByText(/This can repeat external side effects/)).toBeVisible();
    expect(screen.getByText(/Permission prompts still apply in every mode/)).toBeVisible();
  });

  it('persists a selected policy', async () => {
    const user = userEvent.setup();
    render(<CrashRecoveryPolicySection />, { wrapper: IntlTestWrapper });

    await user.click(await screen.findByRole('button', { name: /Always/ }));

    await waitFor(() => {
      expect(window.electron.setSetting).toHaveBeenCalledWith('crashRecoveryPolicy', 'always');
    });
    expect(screen.getByRole('button', { name: /Always/ })).toHaveAttribute('aria-pressed', 'true');
  });

  it('restores the saved policy when persistence fails', async () => {
    const user = userEvent.setup();
    vi.mocked(window.electron.setSetting).mockRejectedValueOnce(new Error('disk full'));
    render(<CrashRecoveryPolicySection />, { wrapper: IntlTestWrapper });

    await user.click(await screen.findByRole('button', { name: /Always/ }));

    expect(await screen.findByRole('alert')).toHaveTextContent('disk full');
    expect(screen.getByRole('button', { name: /Safe \(recommended\)/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
  });
});
