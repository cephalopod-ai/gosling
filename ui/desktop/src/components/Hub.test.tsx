import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { useConfig } from './ConfigContext';
import Hub from './Hub';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { createSession } from '../sessions';

vi.mock('./ConfigContext', () => ({ useConfig: vi.fn() }));
vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
vi.mock('../sessions', () => ({ createSession: vi.fn() }));
vi.mock('./LoadingGosling', () => ({ default: () => null }));
vi.mock('./ChatInputCard', () => ({
  ChatInputCard: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
vi.mock('./ChatInput', () => ({
  default: ({
    handleSubmit,
    submitDisabled,
    submitDisabledReason,
  }: {
    handleSubmit(input: { msg: string; images: [] }): void;
    submitDisabled?: boolean;
    submitDisabledReason?: string;
  }) => (
    <button
      disabled={submitDisabled}
      title={submitDisabledReason}
      onClick={() => handleSubmit({ msg: 'Start the project', images: [] })}
    >
      Send message
    </button>
  ),
}));

const setActiveWorkspace = vi.fn();

function workspace(id: string, name: string, workingFolder: string, validForSession = true) {
  return {
    workspace: {
      id,
      schemaVersion: 1,
      name,
      workingFolder,
      folders: [],
      productOutputFolders: [
        {
          id: `${id}-output`,
          label: 'Outputs',
          path: `${workingFolder}/Outputs`,
          productTypes: ['other' as const],
          isDefault: true,
          createIfMissing: true,
        },
      ],
      credentialBindings: [],
      createdAt: '2026-07-19T00:00:00Z',
      updatedAt: '2026-07-19T00:00:00Z',
      lastOpenedAt: '2026-07-19T00:00:00Z',
    },
    validation: { validForSession, issues: [] },
  };
}

describe('Hub workspace selection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useConfig).mockReturnValue({
      extensionsList: [],
    } as unknown as ReturnType<typeof useConfig>);
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [
        workspace('default', 'Default', '/Users/tester/Work'),
        workspace('personal', 'Personal', '/Users/tester/Personal'),
        workspace('missing', 'Missing folder', '/missing', false),
      ],
      activeWorkspaceId: 'default',
      defaultWorkspaceId: 'default',
      loading: false,
      error: null,
      setActiveWorkspace,
    } as unknown as ReturnType<typeof useWorkspace>);
    vi.mocked(createSession).mockResolvedValue({ id: 'session-personal' } as never);
  });

  it('creates the chat with the workspace explicitly selected in the Hub', async () => {
    const user = userEvent.setup();
    const setView = vi.fn();
    render(<Hub setView={setView} />, { wrapper: IntlTestWrapper });

    expect(screen.getByLabelText('Workspace')).toHaveValue('default');
    expect(screen.getByRole('option', { name: 'Missing folder — needs attention' })).toBeDisabled();

    await user.selectOptions(screen.getByLabelText('Workspace'), 'personal');
    expect(screen.getByTitle('/Users/tester/Personal')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Personal', {
        allExtensions: [],
        workspaceId: 'personal',
      })
    );
    expect(setActiveWorkspace).not.toHaveBeenCalled();
    expect(setView).toHaveBeenCalledWith(
      'pair',
      expect.objectContaining({ resumeSessionId: 'session-personal' })
    );
  });

  it('blocks a session when the selected workspace primary folder is unavailable', async () => {
    const user = userEvent.setup();
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [workspace('missing', 'Missing folder', '/missing', false)],
      activeWorkspaceId: 'missing',
      defaultWorkspaceId: 'missing',
      loading: false,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);

    const setView = vi.fn();
    render(<Hub setView={setView} />, { wrapper: IntlTestWrapper });

    expect(screen.getByRole('alert')).toHaveTextContent('This workspace cannot start a session');
    const send = screen.getByRole('button', { name: 'Send message' });
    expect(send).toBeDisabled();
    await user.click(send);

    expect(createSession).not.toHaveBeenCalled();
    expect(setView).not.toHaveBeenCalled();
  });
});
