import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Session } from '../types/session';
import { useWorkspace } from '../contexts/WorkspaceContext';
import SessionActionsHeader from './SessionActionsHeader';

vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));

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
    } as unknown as ReturnType<typeof useWorkspace>);

    render(<SessionActionsHeader session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByText('Annual Meeting')).toHaveAttribute(
      'title',
      'Pinned to Annual Meeting; new chats use Personal'
    );
  });
});
