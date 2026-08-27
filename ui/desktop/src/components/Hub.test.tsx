import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { useConfig } from './ConfigContext';
import Hub from './Hub';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { createSession } from '../sessions';
import { addResearchInitialInputs, resolveSessionLibraryInputs } from '../acp/sessionLibraryInputs';
import { acpAppendSessionSystemPrompt, acpDeleteSession } from '../acp/sessions';
import { acpListProviderDetails, acpListProviderModels } from '../acp/providers';
import {
  RESEARCH_SCIENTIFIC_METHOD_PROMPT,
  RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
} from '../prompts/researchScientificMethod';
import { RESEARCH_MODEL_TEAM_PROMPT_KEY } from '../prompts/researchModelTeam';
import { RESEARCH_LIBRARY_PROMPT_KEY } from '../prompts/researchLibrary';

vi.mock('./ConfigContext', () => ({ useConfig: vi.fn() }));
vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
vi.mock('../sessions', () => ({ createSession: vi.fn() }));
vi.mock('../acp/sessionLibraryInputs', () => ({
  addResearchInitialInputs: vi.fn(),
  resolveSessionLibraryInputs: vi.fn(),
}));
vi.mock('../acp/sessions', () => ({
  acpAppendSessionSystemPrompt: vi.fn(),
  acpDeleteSession: vi.fn(),
}));
vi.mock('../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListProviderModels: vi.fn(),
}));
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
    sessionModel,
    sessionProvider,
    onModelChanged,
  }: {
    handleSubmit(input: { msg: string; images: [] }): void;
    initialValue?: string;
    submitDisabled?: boolean;
    submitDisabledReason?: string;
    allowEmptySubmit?: boolean;
    sessionModel?: string;
    sessionProvider?: string;
    onModelChanged?(selection: { model: string; provider: string }): void;
  }) => (
    <>
      <output data-testid="composer-model">
        {sessionProvider && sessionModel ? `${sessionProvider}/${sessionModel}` : 'current model'}
      </output>
      <button
        type="button"
        onClick={() => onModelChanged?.({ provider: 'claude', model: 'claude-opus-5' })}
      >
        Use Claude in composer
      </button>
      <button
        type="button"
        onClick={() => onModelChanged?.({ provider: 'claude-code', model: 'claude-opus-5' })}
      >
        Use Claude Code in composer
      </button>
      <button
        disabled={submitDisabled}
        title={submitDisabledReason}
        data-initial-value={initialValue}
        onClick={() =>
          handleSubmit({ msg: allowEmptySubmit ? '' : 'Start the project', images: [] })
        }
      >
        Send message
      </button>
    </>
  ),
}));

vi.mock('./ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({ currentModel: null, currentProvider: null }),
}));

const setActiveWorkspace = vi.fn();
const researchExtensions = [
  {
    name: 'math_mcp',
    enabled: false,
    type: 'stdio' as const,
    cmd: '/usr/local/bin/mathmcp',
  },
  {
    name: 'summon',
    enabled: true,
    type: 'platform' as const,
  },
];

function configureResearchExtensions() {
  vi.mocked(useConfig).mockReturnValue({
    extensionsList: researchExtensions,
  } as unknown as ReturnType<typeof useConfig>);
}
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
    vi.mocked(acpAppendSessionSystemPrompt).mockResolvedValue();
    vi.mocked(acpListProviderDetails).mockResolvedValue([
      {
        name: 'codex',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Builtin',
        metadata: {
          name: 'codex',
          display_name: 'A Codex',
          description: '',
          default_model: 'gpt-5.6-sol',
          model_doc_link: '',
          model_selection_hint: null,
          config_keys: [],
          known_models: [],
          setup_steps: [],
        },
      },
      {
        name: 'claude',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Builtin',
        metadata: {
          name: 'claude',
          display_name: 'B Anthropic',
          description: '',
          default_model: 'claude-opus-5',
          model_doc_link: '',
          model_selection_hint: null,
          config_keys: [],
          known_models: [],
          setup_steps: [],
        },
      },
      {
        name: 'xai',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Builtin',
        metadata: {
          name: 'xai',
          display_name: 'C xAI',
          description: '',
          default_model: 'grok-4.6',
          model_doc_link: '',
          model_selection_hint: null,
          config_keys: [],
          known_models: [],
          setup_steps: [],
        },
      },
      {
        name: 'claude-code',
        is_configured: true,
        manages_own_context: true,
        provider_type: 'Builtin',
        metadata: {
          name: 'claude-code',
          display_name: 'Z Claude Code',
          description: '',
          default_model: 'claude-opus-5',
          model_doc_link: '',
          model_selection_hint: null,
          config_keys: [],
          known_models: [],
          setup_steps: [],
        },
      },
    ]);
    vi.mocked(acpListProviderModels).mockImplementation(async (providerId) => {
      if (providerId === 'codex') return [{ id: 'gpt-5.6-sol' }];
      if (providerId === 'claude') return [{ id: 'claude-opus-5' }];
      if (providerId === 'claude-code') return [{ id: 'claude-opus-5' }];
      return [{ id: 'grok-4.6' }, { id: 'grok-4.6-mini' }];
    });
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
    expect(acpAppendSessionSystemPrompt).not.toHaveBeenCalled();
    expect(setView).toHaveBeenCalledWith(
      'pair',
      expect.objectContaining({ resumeSessionId: 'session-personal' })
    );
  });

  it('scaffolds a tagged research session on the shared new-session flow', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    const setView = vi.fn();
    Object.assign(window.electron, {
      getPathForFile: vi.fn((file: File) => `/Users/tester/Inputs/${file.name}`),
    });
    render(<Hub setView={setView} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    expect(screen.getByRole('heading', { name: 'Deep Research' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Choose research mode' })).toBeInTheDocument();
    expect(screen.getByText('Solo selected')).toBeInTheDocument();
    expect(screen.getByText('Research session')).toBeInTheDocument();
    expect(screen.queryByText('Reports')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Initial Inputs' }));
    await user.type(
      screen.getByLabelText('Paste content'),
      'Review https://example.com and compare the uploaded reports.'
    );
    await user.click(screen.getByRole('button', { name: 'Next' }));
    expect(screen.getByLabelText('Paste content')).toHaveValue('');
    expect(screen.getByText('Pasted input 1')).toBeInTheDocument();
    await user.type(
      screen.getByLabelText('Paste content'),
      'Treat this second pasted item as a separate source.'
    );
    await user.upload(screen.getByLabelText('Choose initial research files'), [
      new File(['report one'], 'report-one.txt', { type: 'text/plain' }),
      new File(['report two'], 'report-two.pdf', { type: 'application/pdf' }),
    ]);
    expect(screen.getByText('report-one.txt')).toBeInTheDocument();
    expect(screen.getByText('report-two.pdf')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(screen.getByText('4 inputs')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(addResearchInitialInputs).toHaveBeenCalledWith('session-personal', {
        texts: [
          'Review https://example.com and compare the uploaded reports.',
          'Treat this second pasted item as a separate source.',
        ],
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
    expect(acpAppendSessionSystemPrompt).toHaveBeenCalledWith(
      'session-personal',
      RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
      RESEARCH_SCIENTIFIC_METHOD_PROMPT
    );
    expect(acpAppendSessionSystemPrompt).toHaveBeenCalledWith(
      'session-personal',
      RESEARCH_LIBRARY_PROMPT_KEY,
      expect.stringContaining('/Users/tester/Documents/Gosling Research Library')
    );
    expect(createSession).toHaveBeenCalledWith(
      '/Users/tester/Work',
      expect.objectContaining({
        extensionConfigs: expect.arrayContaining([expect.objectContaining({ name: 'math_mcp' })]),
        workspaceAdditionalFolders: ['/Users/tester/Documents/Gosling Research Library'],
        researchLibraryPath: '/Users/tester/Documents/Gosling Research Library',
      })
    );
    expect(vi.mocked(acpAppendSessionSystemPrompt).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(addResearchInitialInputs).mock.invocationCallOrder[0]
    );
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

  it('removes an incomplete research session when its default instruction cannot be applied', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    vi.mocked(acpAppendSessionSystemPrompt).mockRejectedValueOnce(
      new Error('The research instruction was rejected')
    );
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Initial Inputs' }));
    await user.type(screen.getByLabelText('Paste content'), 'A starting research prompt');
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() => expect(acpDeleteSession).toHaveBeenCalledWith('session-personal'));
    expect(addResearchInitialInputs).not.toHaveBeenCalled();
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The research instruction was rejected'
    );
  });

  it('starts Dual research with the selected lead and a fixed delegation protocol', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    const dual = await screen.findByRole('radio', { name: 'Dual research mode' });
    await waitFor(() => expect(dual).toBeEnabled());
    await user.click(dual);
    await waitFor(() =>
      expect(screen.getByLabelText('Lead model')).toHaveValue(
        JSON.stringify(['codex', 'gpt-5.6-sol'])
      )
    );

    await user.click(screen.getByRole('button', { name: 'Send message' }));

    await waitFor(() =>
      expect(createSession).toHaveBeenCalledWith('/Users/tester/Work', {
        extensionConfigs: expect.arrayContaining([
          expect.objectContaining({ name: 'math_mcp' }),
          expect.objectContaining({ name: 'summon' }),
        ]),
        workspaceId: 'default',
        workspaceWorkingDir: '/Users/tester/Work',
        workspaceAdditionalFolders: ['/Users/tester/Documents/Gosling Research Library'],
        researchLibraryPath: '/Users/tester/Documents/Gosling Research Library',
        provider: 'codex',
        model: 'gpt-5.6-sol',
      })
    );
    expect(acpAppendSessionSystemPrompt).toHaveBeenCalledWith(
      'session-personal',
      RESEARCH_MODEL_TEAM_PROMPT_KEY,
      expect.stringContaining('claude/claude-opus-5 — independent researcher')
    );
    expect(acpAppendSessionSystemPrompt).toHaveBeenCalledWith(
      'session-personal',
      RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
      expect.stringContaining('No Initial Inputs were supplied')
    );
    const teamPromptCall = vi
      .mocked(acpAppendSessionSystemPrompt)
      .mock.calls.find(([, key]) => key === RESEARCH_MODEL_TEAM_PROMPT_KEY);
    expect(teamPromptCall?.[2]).not.toContain('Initial Input');
  });

  it('keeps the research lead and composer model synchronized without dropping seats', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    const trio = await screen.findByRole('radio', { name: 'Trio research mode' });
    await waitFor(() => expect(trio).toBeEnabled());
    await user.click(trio);

    const lead = screen.getByLabelText('Lead model');
    const researcher2 = screen.getByLabelText('Researcher 2');
    const researcher3 = screen.getByLabelText('Researcher 3');
    await waitFor(() =>
      expect(screen.getByTestId('composer-model')).toHaveTextContent('codex/gpt-5.6-sol')
    );

    await user.selectOptions(lead, JSON.stringify(['xai', 'grok-4.6-mini']));
    await waitFor(() =>
      expect(screen.getByTestId('composer-model')).toHaveTextContent('xai/grok-4.6-mini')
    );
    expect(researcher2).toHaveValue(JSON.stringify(['claude', 'claude-opus-5']));
    expect(researcher3).toHaveValue(JSON.stringify(['xai', 'grok-4.6']));

    await user.click(screen.getByRole('button', { name: 'Use Claude in composer' }));
    await waitFor(() => expect(lead).toHaveValue(JSON.stringify(['claude', 'claude-opus-5'])));
    expect(researcher2).toHaveValue(JSON.stringify(['xai', 'grok-4.6-mini']));
    expect(researcher3).toHaveValue(JSON.stringify(['xai', 'grok-4.6']));
  });

  it('keeps managed-context providers out of every multi-model seat', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    const trio = await screen.findByRole('radio', { name: 'Trio research mode' });
    await waitFor(() => expect(trio).toBeEnabled());
    await user.click(trio);
    await user.click(screen.getByRole('button', { name: 'Use Claude Code in composer' }));

    const managedContextModel = JSON.stringify(['claude-code', 'claude-opus-5']);
    await waitFor(() =>
      expect(screen.getByLabelText('Lead model')).not.toHaveValue(managedContextModel)
    );
    expect(screen.getByLabelText('Researcher 2')).not.toHaveValue(managedContextModel);
    expect(screen.getByLabelText('Researcher 3')).not.toHaveValue(managedContextModel);
    const send = screen.getByRole('button', { name: 'Send message' });
    await waitFor(() => expect(send).toBeEnabled());
    expect(createSession).not.toHaveBeenCalled();
  });

  it('removes an incomplete session when initial input storage fails', async () => {
    configureResearchExtensions();
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

  it('surfaces failed incomplete-session cleanup for manual recovery', async () => {
    configureResearchExtensions();
    const user = userEvent.setup();
    vi.mocked(addResearchInitialInputs).mockRejectedValueOnce(new Error('The report is too large'));
    vi.mocked(acpDeleteSession).mockRejectedValueOnce(new Error('database busy'));
    render(<Hub setView={vi.fn()} sessionExperience="research" />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Initial Inputs' }));
    await user.type(screen.getByLabelText('Paste content'), 'A starting research prompt');
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await user.click(screen.getByRole('button', { name: 'Send message' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The incomplete session could not be removed: database busy. Open Session History and remove it manually.'
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
      sessionDirectoryChooser: vi.fn().mockResolvedValue({
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
