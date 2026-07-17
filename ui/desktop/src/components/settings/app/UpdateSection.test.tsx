import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import UpdateSection from './UpdateSection';
import { IntlTestWrapper } from '../../../i18n/test-utils';

interface UpdaterEvent {
  event: string;
  data?: unknown;
}
type UpdaterEventCallback = (event: UpdaterEvent) => void;
type CheckForUpdatesResult = { updateInfo: unknown; error: string | null };

const renderUpdateSection = () => render(<UpdateSection />, { wrapper: IntlTestWrapper });

describe('UpdateSection', () => {
  let updaterEventCallback: UpdaterEventCallback | null;
  let checkForUpdatesResolvers: Array<(value: CheckForUpdatesResult) => void>;

  beforeEach(() => {
    vi.clearAllMocks();
    updaterEventCallback = null;
    checkForUpdatesResolvers = [];

    Object.assign(window.electron, {
      getVersion: vi.fn(() => '1.0.0'),
      getUpdateState: vi.fn(() => Promise.resolve(null)),
      isUsingGitHubFallback: vi.fn(() => Promise.resolve(false)),
      getAutoDownloadDisabled: vi.fn(() => Promise.resolve(false)),
      onUpdaterEvent: vi.fn((callback: UpdaterEventCallback) => {
        updaterEventCallback = callback;
        return () => {
          updaterEventCallback = null;
        };
      }),
      // Never resolves on its own - each call queues a resolver so the test
      // controls exactly when the IPC round-trip "completes" relative to the
      // updater event, reproducing the interleaving that exposed the bug.
      checkForUpdates: vi.fn(
        () =>
          new Promise<CheckForUpdatesResult>((resolve) => {
            checkForUpdatesResolvers.push(resolve);
          })
      ),
      downloadUpdate: vi.fn(() => Promise.resolve({ success: true, error: null })),
      installUpdate: vi.fn(),
    });
  });

  it('shows the success confirmation when "update-not-available" arrives before the check promise resolves', async () => {
    // This ordering (event before the invoke() response) is what the real
    // main process does, and is exactly what made the stale `updateInfo`
    // closure read a value from before the click. Before the fix, this
    // assertion failed because the success branch never ran.
    const user = userEvent.setup();
    renderUpdateSection();

    const checkButton = await screen.findByRole('button', { name: 'Check for Updates' });
    await user.click(checkButton);

    await waitFor(() => expect(updaterEventCallback).not.toBeNull());
    act(() => {
      updaterEventCallback?.({ event: 'update-not-available' });
    });

    await act(async () => {
      checkForUpdatesResolvers.forEach((resolve) => resolve({ updateInfo: null, error: null }));
    });

    expect(await screen.findByText('You are running the latest version!')).toBeInTheDocument();
  });

  it('shows the success confirmation exactly once, without double-firing, in the normal flow', async () => {
    const user = userEvent.setup();
    renderUpdateSection();

    const checkButton = await screen.findByRole('button', { name: 'Check for Updates' });
    await user.click(checkButton);

    await waitFor(() => expect(updaterEventCallback).not.toBeNull());
    act(() => {
      updaterEventCallback?.({ event: 'update-not-available' });
    });
    await act(async () => {
      checkForUpdatesResolvers.forEach((resolve) => resolve({ updateInfo: null, error: null }));
    });

    expect(await screen.findAllByText('You are running the latest version!')).toHaveLength(1);
  });

  it('does not show a success confirmation when the check fails', async () => {
    const user = userEvent.setup();
    renderUpdateSection();

    const checkButton = await screen.findByRole('button', { name: 'Check for Updates' });
    await user.click(checkButton);

    await waitFor(() => expect(checkForUpdatesResolvers).toHaveLength(1));
    await act(async () => {
      checkForUpdatesResolvers[0]({ updateInfo: null, error: 'network unreachable' });
    });

    expect(await screen.findByText('network unreachable')).toBeInTheDocument();
    expect(screen.queryByText('You are running the latest version!')).not.toBeInTheDocument();
  });
});
