import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { useConfig } from './ConfigContext';
import Hub from './Hub';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { createSession } from '../sessions';
import { addResearchInitialInputs, resolveSessionLibraryInputs } from '../acp/sessionLibraryInputs';
import { acpDeleteSession } from '../acp/sessions';

vi.mock('./ConfigContext', () => ({ useConfig: vi.fn() }));
vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
vi.mock('../sessions', () => ({ createSession: vi.fn() }));
vi.mock('../acp/sessionLibraryInputs', () => ({
  addResearchInitialInputs: vi.fn(),
  resolveSessionLibraryInputs: vi.fn(),
}));
vi.mock('../acp/sessions', () => ({ acpDeleteSession: vi.fn() }));
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
    allowEmptySubmit,
  }: {
    handleSubmit(input: { msg: string; images: [] }): void;
    initialValue?: string;
    submitDisabled?: boolean;
    submitDisabledReason?: string;
    allowEmptySubmit?: boolean;
  }) => (
    <button
      disabled={submitDisabled}
      title={submitDisabledReason}
      data-initial-value={initialValue}
      onClick={() => handleSubmit({ msg: allowEmptySubmit ? '' : 'Start the project', images: [] })}
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

function workspace(
  id: string,
  name: string,
  workingFolder: string,
  validForSession = true,
  createdAt = '2026-07-19T00:00:00Z'
) {
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
      createdAt,
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
    vi.mocked(addResearchInitialInputs).mockResolvedValue([
      'initial-notes',
      'report-one',
      'report-two',
    ]);
    vi.mocked(resolveSessionLibraryInputs).mockResolvedValue({
      assistantContext: 'Resolved initial research inputs',
      images: [],
    });
  });

  it('preselects the active workspace for a global new chat', async () => {
    const user = userEvent.setup();
    const setView = vi.fn();
    render(<Hub setView={setView} />, { wrapper: IntlTestWrapper });

    expect(screen.getByLabelText('Workspace')).toHaveValue('default');
    expect(screen.getByRole('option', { name: 'Missing folder — needs attention' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Send message' })).toBeEnabled();

    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Work', {
        allExtensions: [],
        workspaceId: 'default',
        workspaceWorkingDir: '/Users/tester/Work',
      })
    );
    expect(setActiveWorkspace).not.toHaveBeenCalled();
    expect(setView).toHaveBeenCalledWith(
      'pair',
      expect.objectContaining({ resumeSessionId: 'session-personal' })
    );
  });

  it('scaffolds a tagged research session on the shared new-session flow', async () => {
    const user = userEvent.setup();
    const setView = vi.fn();
    Object.assign(window.electron, {
      getPathForFile: vi.fn((file: File) => `/Users/tester/Inputs/${file.name}`),
    });
    render(<Hub setView={setView} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByRole('heading', { name: 'Deep Research' })).toBeInTheDocument();
    expect(screen.getByText('Research session')).toBeInTheDocument();
    expect(screen.queryByText('Reports')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Initial Inputs' }));
    await user.type(
      screen.getByLabelText('Paste content'),
      'Review https://example.com and compare the uploaded reports.'
    );
    await user.upload(screen.getByLabelText('Choose initial research files'), [
      new File(['report one'], 'report-one.txt', { type: 'text/plain' }),
      new File(['report two'], 'report-two.pdf', { type: 'application/pdf' }),
    ]);
    expect(screen.getByText('report-one.txt')).toBeInTheDocument();
    expect(screen.getByText('report-two.pdf')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(screen.getByText('3 inputs')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(addResearchInitialInputs).toHaveBeenCalledWith('session-personal', {
        text: 'Review https://example.com and compare the uploaded reports.',
        files: [
          expect.objectContaining({
            name: 'report-one.txt',
            path: '/Users/tester/Inputs/report-one.txt',
          }),
          expect.objectContaining({
            name: 'report-two.pdf',
            path: '/Users/tester/Inputs/report-two.pdf',
          }),
        ],
      })
    );
    expect(resolveSessionLibraryInputs).toHaveBeenCalledWith('session-personal', [
      'initial-notes',
      'report-one',
      'report-two',
    ]);
    await waitFor(() =>
      expect(setView).toHaveBeenCalledWith(
        'pair',
        expect.objectContaining({
          resumeSessionId: 'session-personal',
          sessionExperience: 'research',
          initialMessage: {
            msg: 'Begin the research using the initial inputs I provided.',
            images: [],
            assistantContext: 'Resolved initial research inputs',
          },
        })
      )
    );
  });

  it('removes an incomplete session when initial input storage fails', async () => {
    const user = userEvent.setup();
    vi.mocked(addResearchInitialInputs).mockRejectedValueOnce(new Error('The report is too large'));
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Initial Inputs' }));
    await user.type(screen.getByLabelText('Paste content'), 'A starting research prompt');
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() => expect(acpDeleteSession).toHaveBeenCalledWith('session-personal'));
    expect(await screen.findByRole('alert')).toHaveTextContent('The report is too large');
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

    expect(screen.getByLabelText('Workspace')).toHaveValue('missing');
    expect(screen.getByRole('option', { name: 'Missing folder — needs attention' })).toBeDisabled();
    const send = screen.getByRole('button', { name: 'Send message' });
    expect(send).toBeDisabled();
    await user.click(send);

    expect(createSession).not.toHaveBeenCalled();
    expect(setView).not.toHaveBeenCalled();
  });

  it('keeps a launcher prompt in the new-chat draft with the default workspace selected', () => {
    render(<Hub setView={vi.fn()} initialMessage={{ msg: 'Review this project', images: [] }} />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByRole('button', { name: 'Send message' })).toHaveAttribute(
      'data-initial-value',
      'Review this project'
    );
    expect(screen.getByRole('button', { name: 'Send message' })).toBeEnabled();
  });

  it('prefers the workspace supplied by a sidebar new-chat action', () => {
    render(<Hub setView={vi.fn()} initialWorkspaceId="personal" />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByLabelText('Workspace')).toHaveValue('personal');
  });

  it('adopts the active workspace when workspace loading finishes', async () => {
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [],
      activeWorkspaceId: null,
      defaultWorkspaceId: null,
      credentialProfiles: [],
      loading: true,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);

    const { rerender } = render(<Hub setView={vi.fn()} />, { wrapper: IntlTestWrapper });
    expect(screen.getByLabelText('Workspace')).toHaveValue('');

    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [workspace('default', 'Default', '/Users/tester/Work')],
      activeWorkspaceId: 'default',
      defaultWorkspaceId: 'default',
      credentialProfiles: [],
      loading: false,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);
    rerender(<Hub setView={vi.fn()} />);

    await waitFor(() => expect(screen.getByLabelText('Workspace')).toHaveValue('default'));
  });

  it('falls back to the configured default and then the newest workspace', () => {
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [
        workspace('older', 'Older', '/Users/tester/Older', true, '2026-07-18T00:00:00Z'),
        workspace('newest', 'Newest', '/Users/tester/Newest', true, '2026-07-20T00:00:00Z'),
      ],
      activeWorkspaceId: 'deleted',
      defaultWorkspaceId: 'older',
      credentialProfiles: [],
      loading: false,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);

    const { unmount } = render(<Hub setView={vi.fn()} />, { wrapper: IntlTestWrapper });
    expect(screen.getByLabelText('Workspace')).toHaveValue('older');
    unmount();

    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [
        workspace('older', 'Older', '/Users/tester/Older', true, '2026-07-18T00:00:00Z'),
        workspace('newest', 'Newest', '/Users/tester/Newest', true, '2026-07-20T00:00:00Z'),
      ],
      activeWorkspaceId: 'deleted',
      defaultWorkspaceId: 'also-deleted',
      credentialProfiles: [],
      loading: false,
      error: null,
    } as unknown as ReturnType<typeof useWorkspace>);

    render(<Hub setView={vi.fn()} />, { wrapper: IntlTestWrapper });
    expect(screen.getByLabelText('Workspace')).toHaveValue('newest');
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
