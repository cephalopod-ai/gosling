import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { WorkspaceEditorDialog } from './WorkspaceEditorDialog';

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
      wrapper: IntlTestWrapper,
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
      wrapper: IntlTestWrapper,
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

  it('clears effort when the selected model does not support reasoning', async () => {
    const user = userEvent.setup();
    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await screen.findByRole('option', { name: 'Local Fast' });
    await user.selectOptions(screen.getByLabelText('Default provider (optional)'), 'local_fast');
    await screen.findByRole('option', { name: 'fast-model' });

    expect(screen.getByLabelText('Default model (optional)')).toHaveValue('fast-model');
    expect(screen.getByLabelText('Default reasoning effort (optional)')).toBeDisabled();
    expect(screen.getByLabelText('Default reasoning effort (optional)')).toHaveValue('');
  });

  it('keeps a provider-list failure visible when model loading succeeds', async () => {
    vi.mocked(acpListProviderDetails).mockRejectedValueOnce(new Error('inventory unavailable'));

    render(<WorkspaceEditorDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(await screen.findByText('inventory unavailable')).toBeInTheDocument();
    expect(await screen.findByRole('option', { name: 'gpt-5.6-terra' })).toBeInTheDocument();
  });

  it('updates an existing workspace without creating a replacement', async () => {
    const user = userEvent.setup();
    updateWorkspace.mockResolvedValue(activeWorkspace);
    render(<WorkspaceEditorDialog open workspace={activeWorkspace} onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
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
      wrapper: IntlTestWrapper,
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
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Add output destination' }));
    await user.click(screen.getByRole('button', { name: 'Remove Outputs' }));

    expect(screen.getByRole('radio', { name: 'Default output' })).toBeChecked();
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
      { wrapper: IntlTestWrapper }
    );

    expect(
      screen.getByText('This credential profile is missing and must be relinked.')
    ).toBeInTheDocument();
  });
});
