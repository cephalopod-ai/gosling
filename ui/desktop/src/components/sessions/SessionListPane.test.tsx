import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { acpListSessions } from '../../acp/sessions';
import SessionListPane from './SessionListPane';

vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}));

vi.mock('../../contexts/ArtifactRouterContext', () => ({
  useArtifactRouter: () => ({ saveArtifact: vi.fn() }),
}));

vi.mock('../../acp/sessions', () => ({
  acpDeleteSession: vi.fn(),
  acpForkSession: vi.fn(),
  acpListSessions: vi.fn(),
  acpRenameSession: vi.fn(),
  acpShareSessionNostr: vi.fn(),
  acpUnarchiveSession: vi.fn(),
}));

const session = {
  id: 'math-1',
  workspaceId: 'math',
  name: 'First math chat',
  workingDir: '/math',
  createdAt: '2026-09-08T10:00:00Z',
  updatedAt: '2026-09-08T10:00:00Z',
  messageCount: 2,
};

describe('SessionListPane keyboard access', () => {
  afterEach(() => vi.unstubAllGlobals());

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    );
    Object.assign(window.electron, { getConfig: () => ({}), createChatWindow: vi.fn() });
    vi.mocked(acpListSessions).mockResolvedValue({ sessions: [session], nextCursor: null });
  });

  it('reaches a session card with Tab and opens it with Enter', async () => {
    const onSelectSession = vi.fn();
    const user = userEvent.setup();
    render(<SessionListPane mode="active" isActive onSelectSession={onSelectSession} />, {
      wrapper: IntlTestWrapper,
    });
    const open = await screen.findByRole('button', { name: 'First math chat' });

    for (let stop = 0; stop < 10 && document.activeElement !== open; stop += 1) {
      await user.tab();
    }
    expect(open).toHaveFocus();
    await user.keyboard('{Enter}');

    expect(onSelectSession).toHaveBeenCalledTimes(1);
    expect(onSelectSession).toHaveBeenCalledWith('math-1');
  });

  it('keeps the card actions from opening the session', async () => {
    const onSelectSession = vi.fn();
    const user = userEvent.setup();
    render(<SessionListPane mode="active" isActive onSelectSession={onSelectSession} />, {
      wrapper: IntlTestWrapper,
    });
    const open = await screen.findByRole('button', { name: 'First math chat' });
    const newWindow = screen.getByTitle('Open in new window');

    expect(open).not.toContainElement(newWindow);
    await user.click(newWindow);
    expect(window.electron.createChatWindow).toHaveBeenCalledWith(
      expect.objectContaining({ resumeSessionId: 'math-1' })
    );
    expect(onSelectSession).not.toHaveBeenCalled();
  });

  it('does not offer to open an archived session', async () => {
    vi.mocked(acpListSessions).mockResolvedValue({
      sessions: [{ ...session, archivedAt: '2026-09-09T10:00:00Z' }],
      nextCursor: null,
    });
    render(<SessionListPane mode="archived" isActive />, { wrapper: IntlTestWrapper });

    expect(await screen.findByRole('heading', { name: 'First math chat' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'First math chat' })).not.toBeInTheDocument();
  });
});
