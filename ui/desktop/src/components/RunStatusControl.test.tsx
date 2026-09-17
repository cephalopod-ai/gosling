import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { RunStatus } from '../hooks/useRunStatus';
import { RunStatusControl } from './RunStatusControl';

function status(overrides: Partial<RunStatus> = {}): RunStatus {
  return {
    visible: true,
    foregroundActive: true,
    waitingForUser: false,
    tasks: [],
    result: { checkedAt: 0, backendResponded: true, error: null, tasks: [] },
    checking: false,
    quiet: false,
    unavailable: false,
    now: 60_000,
    lastActivityAt: 30_000,
    checkNow: vi.fn(async () => {}),
    ...overrides,
  };
}

describe('RunStatusControl', () => {
  it('exposes a keyboard-operable status check with the backend and output evidence', async () => {
    const user = userEvent.setup();
    const current = status();
    render(<RunStatusControl status={current} onOpenTask={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });
    screen.getByRole('button', { name: 'Activity' }).focus();
    await user.keyboard('{Enter}');
    expect(screen.getByText('Status checks every 15 minutes')).toBeInTheDocument();
    expect(screen.getByText(/Last check:/)).toBeInTheDocument();
    expect(screen.getByText(/Last output:/)).toBeInTheDocument();
    expect(screen.getByText(/This alone does not confirm progress/)).toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Check now' }));
    expect(current.checkNow).toHaveBeenCalledOnce();
  });

  it('surfaces stalled and unavailable states visibly, even when a spinner could keep running', () => {
    const { rerender } = render(
      <RunStatusControl status={status({ quiet: true })} onOpenTask={vi.fn()} />,
      {
        wrapper: IntlTestWrapper,
      }
    );
    expect(screen.getByRole('button', { name: 'No recent progress' })).toBeInTheDocument();
    rerender(
      <RunStatusControl status={status({ quiet: true, unavailable: true })} onOpenTask={vi.fn()} />
    );
    expect(screen.getByRole('button', { name: 'Status unavailable' })).toBeInTheDocument();
  });

  it('keeps background work visible after the main reply ends', async () => {
    const user = userEvent.setup();
    const onOpenTask = vi.fn();
    render(
      <RunStatusControl
        onOpenTask={onOpenTask}
        status={status({
          foregroundActive: false,
          tasks: [
            {
              id: '20260916_1',
              description: 'Verify sources',
              state: 'running',
              turns: 3,
              idleMs: 60_000,
              checkedAt: 0,
              error: null,
            },
          ],
        })}
      />,
      { wrapper: IntlTestWrapper }
    );
    await user.click(screen.getByRole('button', { name: '1 background agent' }));
    expect(screen.getByText('Verify sources')).toBeInTheDocument();
    expect(screen.getByText(/3 turns/)).toBeInTheDocument();
    expect(screen.getByText(/Idle: 1 min/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Open agent chat' }));
    expect(onOpenTask).toHaveBeenCalledWith('20260916_1');
  });
});
