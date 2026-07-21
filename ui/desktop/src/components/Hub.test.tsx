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
    initialValue,
    submitDisabled,
    submitDisabledReason,
  }: {
    handleSubmit(input: { msg: string; images: [] }): void;
    initialValue?: string;
    submitDisabled?: boolean;
    submitDisabledReason?: string;
  }) => (
    <button
      disabled={submitDisabled}
      title={submitDisabledReason}
      data-initial-value={initialValue}
      onClick={() => handleSubmit({ msg: 'Start the project', images: [] })}
    >
      Send message
    </button>
  ),
}));

vi.mock('./ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({ currentModel: null, currentProvider: null }),
}));

const setActiveWorkspace = vi.fn();
const configuredCredentialProfile = {
  id: 'profile-personal',
  name: 'Personal API key',
  providerOrServiceId: 'openai',
  authKind: 'config_fields',
  configuredSecretFields: ['OPENAI_API_KEY'],
  nonSecretFields: {},
  status: 'configured',
  source: 'workspace_secure_storage',
  createdAt: '2026-07-19T00:00:00Z',
  updatedAt: '2026-07-19T00:00:00Z',
};

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
      credentialProfiles: [configuredCredentialProfile],
      loading: false,
      error: null,
      setActiveWorkspace,
    } as unknown as ReturnType<typeof useWorkspace>);
    vi.mocked(createSession).mockResolvedValue({ id: 'session-personal' } as never);
  });

  it('requires a workspace choice instead of inheriting the active workspace', async () => {
    const user = userEvent.setup();
    const setView = vi.fn();
    render(<Hub setView={setView} />, { wrapper: IntlTestWrapper });

    expect(screen.getByLabelText('Workspace')).toHaveValue('');
    expect(screen.getByRole('option', { name: 'Missing folder — needs attention' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();

    await user.selectOptions(screen.getByLabelText('Workspace'), 'personal');
    expect(screen.getByTitle('/Users/tester/Personal')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Personal', {
        allExtensions: [],
        workspaceId: 'personal',
        workspaceWorkingDir: '/Users/tester/Personal',
      })
    );
    expect(setActiveWorkspace).not.toHaveBeenCalled();
    expect(setView).toHaveBeenCalledWith(
      'pair',
      expect.objectContaining({ resumeSessionId: 'session-personal' })
    );
  });

  it('does not allow a workspace with an unavailable primary folder', async () => {
    const user = userEvent.setup();
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [workspace('missing', 'Missing folder', '/missing', false)],
      activeWorkspaceId: 'missing',
      defaultWorkspaceId: 'missing',
      credentialProfiles: [],
      loading: false,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);

    const setView = vi.fn();
    render(<Hub setView={setView} />, { wrapper: IntlTestWrapper });

    expect(screen.getByLabelText('Workspace')).toHaveValue('');
    expect(screen.getByRole('option', { name: 'Missing folder — needs attention' })).toBeDisabled();
    const send = screen.getByRole('button', { name: 'Send message' });
    expect(send).toBeDisabled();
    await user.click(send);

    expect(createSession).not.toHaveBeenCalled();
    expect(setView).not.toHaveBeenCalled();
  });

  it('keeps a launcher prompt in the new-chat draft until a workspace is chosen', () => {
    render(<Hub setView={vi.fn()} initialMessage={{ msg: 'Review this project', images: [] }} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByRole('button', { name: 'Send message' })).toHaveAttribute(
      'data-initial-value',
      'Review this project'
    );
    expect(screen.getByRole('button', { name: 'Send message' })).toBeDisabled();
  });

  it('uses an explicitly selected credential only for the new chat', async () => {
    const user = userEvent.setup();
    render(<Hub setView={vi.fn()} />, { wrapper: IntlTestWrapper });

    await user.selectOptions(screen.getByLabelText('Workspace'), 'personal');
    await user.selectOptions(screen.getByLabelText('Credential'), 'profile-personal');
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Personal', {
        allExtensions: [],
        workspaceId: 'personal',
        workspaceWorkingDir: '/Users/tester/Personal',
        workspaceCredentialProfileId: 'profile-personal',
      })
    );
    expect(setActiveWorkspace).not.toHaveBeenCalled();
  });

  it('clears draft credentials and folders when the workspace changes', async () => {
    const user = userEvent.setup();
    Object.assign(window.electron, {
      directoryChooser: vi.fn().mockResolvedValue({
        canceled: false,
        filePaths: ['/Users/tester/Shared'],
      }),
    });
    render(<Hub setView={vi.fn()} />, { wrapper: IntlTestWrapper });

    await user.selectOptions(screen.getByLabelText('Workspace'), 'default');
    await user.selectOptions(screen.getByLabelText('Credential'), 'profile-personal');
    await user.click(screen.getByRole('button', { name: 'Add folder' }));
    await screen.findByTitle('/Users/tester/Shared');

    await user.selectOptions(screen.getByLabelText('Workspace'), 'personal');
    expect(screen.getByLabelText('Credential')).toHaveValue('');
    expect(screen.queryByTitle('/Users/tester/Shared')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Send message' }));
    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Personal', {
        allExtensions: [],
        workspaceId: 'personal',
        workspaceWorkingDir: '/Users/tester/Personal',
      })
    );
  });
});
