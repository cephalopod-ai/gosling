import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Session } from '../types/session';
import { useWorkspace } from '../contexts/WorkspaceContext';
import SessionActionsHeader from './SessionActionsHeader';

vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
const { readCheckpoint } = vi.hoisted(() => ({ readCheckpoint: vi.fn() }));
vi.mock('../acp/providers', () => ({
  acpReadSessionHandoffCheckpoint: (...args: unknown[]) => readCheckpoint(...args),
}));

const session: Session = {
  id: 'session-1',
  name: 'Review packet',
  message_count: 2,
  created_at: '2026-07-18T00:00:00Z',
  updated_at: '2026-07-18T00:00:00Z',
  working_dir: '/projects/annual-meeting',
  extension_data: { active: [], installed: [] },
  workspace_id: 'annual-meeting',
  workspace_name: 'Annual Meeting',
};

describe('SessionActionsHeader workspace badge', () => {
  it('shows that a visible session remains pinned after the active workspace changes', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: {
        id: 'personal',
        schemaVersion: 1,
        name: 'Personal',
        workingFolder: '/projects/personal',
        productOutputFolders: [],
        createdAt: '2026-07-18T00:00:00Z',
        updatedAt: '2026-07-18T00:00:00Z',
        lastOpenedAt: '2026-07-18T00:00:00Z',
      },
      workspaces: [],
    } as unknown as ReturnType<typeof useWorkspace>);

    render(<SessionActionsHeader session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByText('Annual Meeting')).toHaveAttribute(
      'title',
      'Pinned to Annual Meeting; new chats use Personal'
    );
  });

  it('names the pinned workspace by its current name after a rename and label reuse', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: null,
      workspaces: [
        { workspace: { id: 'annual-meeting', name: 'Board Meeting' } },
        { workspace: { id: 'personal', name: 'Annual Meeting' } },
      ],
    } as unknown as ReturnType<typeof useWorkspace>);

    render(<SessionActionsHeader session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByText('Board Meeting')).toHaveAttribute('title', 'Workspace: Board Meeting');
    expect(screen.queryByText('Annual Meeting')).not.toBeInTheDocument();
  });

  it('marks the stored name when the pinned workspace was deleted', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: null,
      workspaces: [{ workspace: { id: 'personal', name: 'Personal' } }],
    } as unknown as ReturnType<typeof useWorkspace>);

    render(
      <SessionActionsHeader
        session={{ ...session, workspace_name: 'Annual Meeting (removed)' }}
        onSessionChange={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );
    expect(screen.getByText('Annual Meeting (removed)')).toBeInTheDocument();
  });

  it('marks a workspace deleted while the chat is open', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: null,
      workspaces: [{ workspace: { id: 'personal', name: 'Personal' } }],
    } as unknown as ReturnType<typeof useWorkspace>);

    render(<SessionActionsHeader session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByText('Annual Meeting (removed)')).toBeInTheDocument();
  });

  it('flows in the header row and lets the title and badges shrink instead of overlapping', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: null,
      workspaces: [],
    } as unknown as ReturnType<typeof useWorkspace>);

    render(
      <SessionActionsHeader
        session={{ ...session, credential_profile_name: 'G1 Fixture B' }}
        onSessionChange={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    const trigger = screen.getByRole('button', { name: 'Session actions' });
    const container = trigger.parentElement as HTMLElement;
    expect(container).not.toHaveClass('absolute');
    expect(container).not.toHaveClass('-translate-x-1/2');
    expect(container).toHaveClass('min-w-0');
    expect(trigger).toHaveClass('min-w-0', 'max-w-full');
    expect(screen.getByText('Review packet')).toHaveClass('min-w-0', 'truncate');
    for (const badge of [screen.getByText('Annual Meeting'), screen.getByText('G1 Fixture B')]) {
      expect(badge).toHaveClass('min-w-0', 'truncate');
    }
  });

  it('opens the latest durable handoff checkpoint from session actions', async () => {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: null,
      workspaces: [],
    } as unknown as ReturnType<typeof useWorkspace>);
    readCheckpoint.mockResolvedValue({
      snapshotId: 'handoff-1',
      continuityClass: 'summarized_handoff',
      coverage: { coveredMessageCount: 2, totalMessageCount: 2 },
    });
    const user = userEvent.setup();
    render(<SessionActionsHeader session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Session actions' }));
    await user.click(await screen.findByText('View handoff checkpoint'));

    expect(await screen.findByText('Session handoff checkpoint')).toBeInTheDocument();
    expect(readCheckpoint).toHaveBeenCalledWith('session-1');
    expect(screen.getByText('"handoff-1"')).toBeInTheDocument();
  });
});
