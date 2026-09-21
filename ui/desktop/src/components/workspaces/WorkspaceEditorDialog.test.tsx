import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { WorkspaceEditorDialog } from './WorkspaceEditorDialog';

// Spying on navigation needs the real MemoryRouter kept for Router context,
// with only useNavigate replaced.
const { mockNavigate } = vi.hoisted(() => ({ mockNavigate: vi.fn() }));
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return { ...actual, useNavigate: () => mockNavigate };
});

// The dialog's "Configure other providers" option navigates via useNavigate(),
// which requires Router context (see GroupedExtensionLoadingToast.test.tsx for
// the same pattern).
function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <IntlTestWrapper>
      <MemoryRouter>{children}</MemoryRouter>
    </IntlTestWrapper>
  );
}

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
}));

vi.mock('../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListProviderModels: vi.fn(),
}));

vi.mock('./CredentialProfileManagerDialog', () => ({
  CredentialProfileManagerDialog: () => null,
}));

vi.mock('../../acp/extensions', () => ({
  getConfiguredExtensions: vi.fn(async () => ({
    extensions: [
      { name: 'developer', enabled: true },
      { name: 'muninn', enabled: true },
      { name: 'chrome-devtools', enabled: true },
    ],
    warnings: [],
  })),
}));

const createWorkspace = vi.fn();
const updateWorkspace = vi.fn();
const validateWorkspace = vi.fn();

const activeWorkspace = {
  id: 'workspace-default',
  schemaVersion: 1,
  name: 'Default',
  workingFolder: '/projects/default',
  productOutputFolders: [
    {
      id: 'output-default',
      label: 'Outputs',
      path: '/projects/default/Outputs',
      productTypes: ['document' as const],
      isDefault: true,
      createIfMissing: true,
    },
  ],
  createdAt: '2026-07-18T00:00:00Z',
  updatedAt: '2026-07-18T00:00:00Z',
  lastOpenedAt: '2026-07-18T00:00:00Z',
};

describe('WorkspaceEditorDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    validateWorkspace.mockResolvedValue({ validForSession: true, issues: [] });
    createWorkspace.mockResolvedValue(activeWorkspace);
    vi.mocked(acpListProviderDetails).mockResolvedValue([
      {
        name: 'chatgpt_codex',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Preferred',
        metadata: {
          name: 'chatgpt_codex',
          display_name: 'ChatGPT Codex',
          description: 'Codex via ChatGPT',
          default_model: 'gpt-5.6-sol',
          known_models: [],
          model_doc_link: '',
          config_keys: [],
        },
      },
      {
        name: 'local_fast',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Custom',
        metadata: {
          name: 'local_fast',
          display_name: 'Local Fast',
          description: 'Local non-reasoning provider',
          default_model: 'fast-model',
          known_models: [],
          model_doc_link: '',
          config_keys: [],
        },
      },
    ]);
    vi.mocked(acpListProviderModels).mockImplementation(async (providerId) =>
      providerId === 'chatgpt_codex'
        ? [
            {
              id: 'gpt-5.6-sol',
              reasoning: true,
              thinkingEfforts: ['off', 'low', 'medium', 'high', 'max'],
            },
            {
              id: 'gpt-5.6-terra',
              reasoning: true,
              thinkingEfforts: ['off', 'low', 'medium', 'high', 'max'],
            },
          ]
        : [{ id: 'fast-model', reasoning: false, thinkingEfforts: [] }]
    );
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [],
      activeWorkspace,
      activeWorkspaceId: activeWorkspace.id,
      defaultWorkspaceId: activeWorkspace.id,
      credentialProfiles: [],
      loading: false,
      error: null,
      sessionWorkspaceFilterId: activeWorkspace.id,
      setSessionWorkspaceFilterId: vi.fn(),
      refreshWorkspaces: vi.fn(),
      createWorkspace,
      updateWorkspace,
      duplicateWorkspace: vi.fn(),
      deleteWorkspace: vi.fn(),
      setActiveWorkspace: vi.fn(),
      validateWorkspace,
      createCredentialProfile: vi.fn(),
      updateCredentialProfile: vi.fn(),
      deleteCredentialProfile: vi.fn(),
    });
    Object.assign(window.electron, {
      directoryChooser: vi.fn().mockResolvedValue({
        canceled: false,
        filePaths: ['/projects/annual-meeting'],
      }),
      addRecentDir: vi.fn().mockResolvedValue(undefined),
      openDirectoryInExplorer: vi.fn().mockResolvedValue(true),
    });
    Object.assign(window, {
      appConfig: {
        get: vi.fn((key: string) => {
          if (key === 'GOSLING_HOME_DIR') return '/Users/tester';
          if (key === 'GOSLING_WORKING_DIR') return '/Users/tester';
          return undefined;
        }),
        getAll: vi.fn(() => ({})),
      },
    });
  });

  it('preselects Codex Terra with medium effort in ~/Work for new drafts', async () => {
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    expect(screen.getByLabelText('Primary working folder')).toHaveValue('/Users/tester/Work');
    expect(screen.getByLabelText('Output path')).toHaveValue('/Users/tester/Work/Outputs');
    expect(await screen.findByRole('option', { name: 'ChatGPT Codex' })).toBeInTheDocument();
    expect(screen.getByLabelText('Default provider (optional)')).toHaveValue('chatgpt_codex');
    expect(await screen.findByRole('option', { name: 'gpt-5.6-terra' })).toBeInTheDocument();
    expect(screen.getByLabelText('Default model (optional)')).toHaveValue('gpt-5.6-terra');
    expect(screen.getByLabelText('Default reasoning effort (optional)')).toHaveValue('medium');
    expect(screen.queryByRole('option', { name: 'Ultra' })).not.toBeInTheDocument();
  });

  it('uses the directory chooser and submits folders plus product outputs', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    render(<WorkspaceEditorDialog open onOpenChange={onOpenChange} />, {
      wrapper: TestWrapper,
    });

    await user.type(screen.getByLabelText('Name'), 'Annual Meeting');
    await user.click(screen.getByRole('button', { name: 'Choose Primary working folder' }));
    expect(screen.getByLabelText('Primary working folder')).toHaveValue('/projects/annual-meeting');
    expect(screen.getByLabelText('Output path')).toHaveValue('/projects/annual-meeting/Outputs');
    await user.click(screen.getByRole('button', { name: 'Add source/reference folder' }));
    await user.click(screen.getByRole('button', { name: 'Add working folder' }));
    await user.click(screen.getByRole('button', { name: 'Add output destination' }));
    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    expect(validateWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Annual Meeting',
        workingFolder: '/projects/annual-meeting',
        folders: expect.arrayContaining([
          expect.objectContaining({ kind: 'reference', access: 'read' }),
          expect.objectContaining({ kind: 'working', access: 'read_write' }),
        ]),
        productOutputFolders: expect.arrayContaining([
          expect.objectContaining({ label: 'Outputs', isDefault: true }),
          expect.objectContaining({ label: 'Output', isDefault: false }),
        ]),
      }),
      undefined
    );
    expect(createWorkspace).toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('includes a folder note in the saved workspace', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.type(screen.getByLabelText('Name'), 'Annual Meeting');
    await user.click(screen.getByRole('button', { name: 'Add source/reference folder' }));
    await user.type(
      screen.getByLabelText('Folder note for the agent'),
      "Similar code lives here for comparison, but it's not meant to be kept identical"
    );
    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    expect(validateWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        folders: expect.arrayContaining([
          expect.objectContaining({
            kind: 'reference',
            description:
              "Similar code lives here for comparison, but it's not meant to be kept identical",
          }),
        ]),
      }),
      undefined
    );
  });

  it('pins only the chosen MCP servers, leaving built-in tools out of the choice', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, { wrapper: TestWrapper });

    await user.type(screen.getByLabelText('Name'), 'Muninn only');
    // Leaving the box ticked inherits whatever is enabled globally.
    await user.click(await screen.findByLabelText('Use everything enabled'));

    // Built-in tools are never offered here; the backend keeps them on regardless.
    expect(screen.queryByLabelText('developer')).not.toBeInTheDocument();
    await user.click(screen.getByLabelText('chrome-devtools'));

    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    expect(validateWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({ defaultExtensions: ['muninn'] }),
      undefined
    );
  });

  it('leaves defaultExtensions unset when the workspace inherits everything', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, { wrapper: TestWrapper });

    await user.type(screen.getByLabelText('Name'), 'Everything');
    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    // Absent and null both mean "inherit the globally enabled set"; the backend
    // only narrows when the field carries a list.
    const saved = vi.mocked(validateWorkspace).mock.calls[0]?.[0] as {
      defaultExtensions?: string[] | null;
    };
    expect(saved.defaultExtensions ?? null).toBeNull();
  });

  it('hides unconfigured providers behind a configure-providers escape hatch', async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    vi.mocked(acpListProviderDetails).mockResolvedValueOnce([
      {
        name: 'chatgpt_codex',
        is_configured: true,
        manages_own_context: false,
        provider_type: 'Preferred',
        metadata: {
          name: 'chatgpt_codex',
          display_name: 'ChatGPT Codex',
          description: 'Codex via ChatGPT',
          default_model: 'gpt-5.6-sol',
          known_models: [],
          model_doc_link: '',
          config_keys: [],
        },
      },
      {
        name: 'anthropic',
        is_configured: false,
        manages_own_context: false,
        provider_type: 'Preferred',
        metadata: {
          name: 'anthropic',
          display_name: 'Anthropic',
          description: 'Anthropic API',
          default_model: 'claude',
          known_models: [],
          model_doc_link: '',
          config_keys: [],
        },
      },
    ]);

    render(<WorkspaceEditorDialog open onOpenChange={onOpenChange} />, {
      wrapper: TestWrapper,
    });

    await screen.findByRole('option', { name: 'ChatGPT Codex' });
    expect(screen.queryByRole('option', { name: 'Anthropic' })).not.toBeInTheDocument();

    await user.selectOptions(
      screen.getByLabelText('Default provider (optional)'),
      'Configure other providers…'
    );

    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it('clears effort when the selected model does not support reasoning', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await screen.findByRole('option', { name: 'Local Fast' });
    await user.selectOptions(screen.getByLabelText('Default provider (optional)'), 'local_fast');
    await screen.findByRole('option', { name: 'fast-model' });

    expect(screen.getByLabelText('Default model (optional)')).toHaveValue('fast-model');
    expect(screen.getByLabelText('Default reasoning effort (optional)')).toBeDisabled();
    expect(screen.getByLabelText('Default reasoning effort (optional)')).toHaveValue('');
  });

  it('does not retain models from the previous provider while the new catalog loads', async () => {
    const user = userEvent.setup();
    let resolveLocalModels:
      | ((models: Awaited<ReturnType<typeof acpListProviderModels>>) => void)
      | undefined;
    vi.mocked(acpListProviderModels).mockImplementation((providerId) => {
      if (providerId === 'chatgpt_codex') {
        return Promise.resolve([
          {
            id: 'gpt-5.6-terra',
            reasoning: true,
            thinkingEfforts: ['off', 'low', 'medium', 'high', 'max'],
          },
        ]);
      }
      return new Promise((resolve) => {
        resolveLocalModels = resolve;
      });
    });

    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await screen.findByRole('option', { name: 'gpt-5.6-terra' });
    await user.selectOptions(screen.getByLabelText('Default provider (optional)'), 'local_fast');

    const modelSelect = screen.getByLabelText('Default model (optional)');
    expect(modelSelect).toBeDisabled();
    expect(screen.queryByRole('option', { name: 'gpt-5.6-terra' })).not.toBeInTheDocument();

    expect(resolveLocalModels).toBeDefined();
    resolveLocalModels!([{ id: 'fast-model', reasoning: false, thinkingEfforts: [] }]);
    expect(await screen.findByRole('option', { name: 'fast-model' })).toBeInTheDocument();
    expect(modelSelect).not.toBeDisabled();
    expect(modelSelect).toHaveValue('fast-model');
  });

  it('keeps a provider-list failure visible when model loading succeeds', async () => {
    vi.mocked(acpListProviderDetails).mockRejectedValueOnce(new Error('inventory unavailable'));

    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    expect(await screen.findByText('inventory unavailable')).toBeInTheDocument();
    expect(await screen.findByRole('option', { name: 'gpt-5.6-terra' })).toBeInTheDocument();
  });

  it('updates an existing workspace without creating a replacement', async () => {
    const user = userEvent.setup();
    updateWorkspace.mockResolvedValue(activeWorkspace);
    render(<WorkspaceEditorDialog open workspace={activeWorkspace} onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.clear(screen.getByLabelText('Name'));
    await user.type(screen.getByLabelText('Name'), 'Renamed workspace');
    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    expect(updateWorkspace).toHaveBeenCalledWith(
      activeWorkspace.id,
      expect.objectContaining({ name: 'Renamed workspace' })
    );
    expect(createWorkspace).not.toHaveBeenCalled();
  });

  it('validates a draft without persisting it', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.type(screen.getByLabelText('Name'), 'Validated workspace');
    await user.click(screen.getByRole('button', { name: 'Validate' }));

    expect(validateWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'Validated workspace' }),
      undefined
    );
    expect(await screen.findByText('Workspace validation passed.')).toBeInTheDocument();
    expect(createWorkspace).not.toHaveBeenCalled();
    expect(updateWorkspace).not.toHaveBeenCalled();
  });

  it('reassigns the default before removing the current default output', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Add output destination' }));
    await user.click(screen.getByRole('button', { name: 'Remove Outputs' }));

    expect(screen.getByRole('radio', { name: 'Default output' })).toBeChecked();
  });

  it('starts a new credential binding unselected rather than binding whichever profile sorts first', async () => {
    const user = userEvent.setup();
    const profile = (id: string, name: string) => ({
      id,
      name,
      providerOrServiceId: name,
      authKind: 'config_fields' as const,
      configuredSecretFields: [],
      nonSecretFields: {},
      status: 'configured' as const,
      source: 'workspace_secure_storage' as const,
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
    });
    vi.mocked(useWorkspace).mockReturnValue({
      ...vi.mocked(useWorkspace)(),
      credentialProfiles: [profile('p1', 'featherless'), profile('p2', 'anthropic')],
    });

    render(<WorkspaceEditorDialog open workspace={null} onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.click(screen.getByRole('button', { name: /Add credential binding/ }));

    // Pre-selecting profiles[0] made every click add the same profile again.
    expect(screen.getByRole('combobox', { name: 'Credential profile' })).toHaveValue('');
    expect(screen.getByRole('textbox', { name: 'Credential binding label' })).toHaveValue('');
    expect(screen.getByText('Choose a credential profile for this binding.')).toBeInTheDocument();
  });

  it('fills an empty binding label from the profile the user picks', async () => {
    const user = userEvent.setup();
    vi.mocked(useWorkspace).mockReturnValue({
      ...vi.mocked(useWorkspace)(),
      credentialProfiles: [
        {
          id: 'p2',
          name: 'anthropic',
          providerOrServiceId: 'anthropic',
          authKind: 'config_fields' as const,
          configuredSecretFields: [],
          nonSecretFields: {},
          status: 'configured' as const,
          source: 'workspace_secure_storage' as const,
          createdAt: '2026-01-01T00:00:00Z',
          updatedAt: '2026-01-01T00:00:00Z',
        },
      ],
    });

    render(<WorkspaceEditorDialog open workspace={null} onOpenChange={vi.fn()} />, {
      wrapper: TestWrapper,
    });

    await user.click(screen.getByRole('button', { name: /Add credential binding/ }));
    await user.selectOptions(screen.getByRole('combobox', { name: 'Credential profile' }), 'p2');

    expect(screen.getByRole('textbox', { name: 'Credential binding label' })).toHaveValue(
      'anthropic'
    );
  });

  it('offers setup for an unconfigured profile, routed by where its secret lives', async () => {
    const user = userEvent.setup();
    const binding = (id: string, profileId: string) => ({
      id,
      label: profileId,
      credentialProfileId: profileId,
      targetKind: 'provider' as const,
      targetId: 'featherless',
      isDefault: id === 'binding-1',
    });
    vi.mocked(useWorkspace).mockReturnValue({
      ...vi.mocked(useWorkspace)(),
      credentialProfiles: [
        {
          id: 'workspace-owned',
          name: 'featherless',
          providerOrServiceId: 'featherless',
          authKind: 'config_fields' as const,
          configuredSecretFields: [],
          nonSecretFields: {},
          status: 'missing' as const,
          source: 'workspace_secure_storage' as const,
          createdAt: '2026-01-01T00:00:00Z',
          updatedAt: '2026-01-01T00:00:00Z',
        },
        {
          id: 'global-provider::featherless',
          name: 'Current featherless configuration',
          providerOrServiceId: 'featherless',
          authKind: 'config_fields' as const,
          configuredSecretFields: [],
          nonSecretFields: {},
          status: 'missing' as const,
          source: 'global_configuration_alias' as const,
          createdAt: '2026-01-01T00:00:00Z',
          updatedAt: '2026-01-01T00:00:00Z',
        },
      ],
    });

    render(
      <WorkspaceEditorDialog
        open
        workspace={{
          ...activeWorkspace,
          credentialBindings: [
            binding('binding-1', 'workspace-owned'),
            binding('binding-2', 'global-provider::featherless'),
          ],
          defaultCredentialBindingId: 'binding-1',
        }}
        onOpenChange={vi.fn()}
      />,
      { wrapper: TestWrapper }
    );

    // A workspace-owned profile is fixable here; an alias only points at the
    // provider's own saved credentials, so it must send the user there instead.
    expect(screen.getByText('This profile has no stored secret yet.')).toBeInTheDocument();
    expect(
      screen.getByText('featherless has no saved credentials to reference.')
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Open provider settings' }));
    expect(mockNavigate).toHaveBeenCalledWith('/configure-providers');
  });

  it('shows an actionable relink state for a missing credential profile', () => {
    render(
      <WorkspaceEditorDialog
        open
        workspace={{
          ...activeWorkspace,
          credentialBindings: [
            {
              id: 'binding-1',
              label: 'AFRL Anthropic',
              credentialProfileId: 'deleted-profile',
              targetKind: 'provider',
              targetId: 'anthropic',
              isDefault: true,
            },
          ],
          defaultCredentialBindingId: 'binding-1',
        }}
        onOpenChange={vi.fn()}
      />,
      { wrapper: TestWrapper }
    );

    expect(
      screen.getByText('This credential profile is missing and must be relinked.')
    ).toBeInTheDocument();
  });
});
