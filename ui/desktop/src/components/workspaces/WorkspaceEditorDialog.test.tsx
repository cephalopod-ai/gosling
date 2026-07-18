import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { WorkspaceEditorDialog } from './WorkspaceEditorDialog';

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
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
    await user.click(screen.getByRole('button', { name: 'Add output destination' }));
    await user.click(screen.getByRole('button', { name: 'Save workspace' }));

    expect(validateWorkspace).toHaveBeenCalledWith(
      expect.objectContaining({
        name: 'Annual Meeting',
        workingFolder: '/projects/annual-meeting',
        folders: [expect.objectContaining({ kind: 'reference', access: 'read' })],
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
