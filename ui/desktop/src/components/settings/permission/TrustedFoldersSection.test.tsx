import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { TRUSTED_DIRS_CONFIG_KEY, TrustedFoldersSection } from './TrustedFoldersSection';

const read = vi.fn();
const upsert = vi.fn();

vi.mock('../../ConfigContext', () => ({
  useConfig: () => ({ read, upsert }),
}));

describe('TrustedFoldersSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    read.mockResolvedValue(['/Users/tester/Work']);
    upsert.mockResolvedValue(undefined);
  });

  it('adds a chosen folder to the stored trusted list', async () => {
    vi.mocked(window.electron.directoryChooser).mockResolvedValue({
      canceled: false,
      filePaths: ['/Users/tester/Downloads'],
    } as Awaited<ReturnType<typeof window.electron.directoryChooser>>);

    render(
      <IntlTestWrapper>
        <TrustedFoldersSection />
      </IntlTestWrapper>
    );

    expect(await screen.findByText('/Users/tester/Work')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Add folder' }));

    await waitFor(() =>
      expect(upsert).toHaveBeenCalledWith(
        TRUSTED_DIRS_CONFIG_KEY,
        ['/Users/tester/Work', '/Users/tester/Downloads'],
        false
      )
    );
    expect(screen.getByText('/Users/tester/Downloads')).toBeInTheDocument();
  });

  it('removes a folder and restores it when the write fails', async () => {
    upsert.mockRejectedValue(new Error('backend unavailable'));

    render(
      <IntlTestWrapper>
        <TrustedFoldersSection />
      </IntlTestWrapper>
    );

    fireEvent.click(
      await screen.findByRole('button', { name: 'Stop trusting /Users/tester/Work' })
    );

    expect(await screen.findByText('Could not save your trusted folders.')).toBeInTheDocument();
    expect(screen.getByText('/Users/tester/Work')).toBeInTheDocument();
  });

  it('ignores a canceled folder picker', async () => {
    vi.mocked(window.electron.directoryChooser).mockResolvedValue({
      canceled: true,
      filePaths: [],
    } as Awaited<ReturnType<typeof window.electron.directoryChooser>>);

    render(
      <IntlTestWrapper>
        <TrustedFoldersSection />
      </IntlTestWrapper>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Add folder' }));

    await waitFor(() => expect(window.electron.directoryChooser).toHaveBeenCalledTimes(1));
    expect(upsert).not.toHaveBeenCalled();
  });
});
