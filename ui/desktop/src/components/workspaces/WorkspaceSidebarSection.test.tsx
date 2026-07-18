import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { WorkspaceSidebarSection } from './WorkspaceSidebarSection';

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
}));

vi.mock('./WorkspaceEditorDialog', () => ({
  WorkspaceEditorDialog: ({ open }: { open: boolean }) =>
    open ? <div role="dialog">Workspace editor</div> : null,
}));

const workspace = {
  id: 'workspace-1',
  schemaVersion: 1,
  name: 'Annual Meeting',
  workingFolder: '/projects/annual-meeting',
  folders: [],
  productOutputFolders: [
    {
      id: 'output-1',
      label: 'Documents',
      path: '/projects/annual-meeting/documents',
      productTypes: ['document' as const],
      isDefault: true,
      createIfMissing: true,
    },
  ],
  createdAt: '2026-07-18T00:00:00Z',
  updatedAt: '2026-07-18T00:00:00Z',
  lastOpenedAt: '2026-07-18T00:00:00Z',
};

describe('WorkspaceSidebarSection', () => {
  const setActiveWorkspace = vi.fn();
  const setSessionWorkspaceFilterId = vi.fn();
  const duplicateWorkspace = vi.fn();
  const deleteWorkspace = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [
        {
          workspace,
          validation: {
            validForSession: false,
            issues: [
              {
                code: 'missing_primary_folder',
                severity: 'error',
                message: 'Relink the primary folder',
              },
            ],
          },
        },
      ],
      activeWorkspace: workspace,
      activeWorkspaceId: workspace.id,
      defaultWorkspaceId: workspace.id,
      credentialProfiles: [],
      loading: false,
      error: null,
      sessionWorkspaceFilterId: workspace.id,
      setSessionWorkspaceFilterId,
      refreshWorkspaces: vi.fn(),
      createWorkspace: vi.fn(),
      updateWorkspace: vi.fn(),
      duplicateWorkspace,
      deleteWorkspace,
      setActiveWorkspace,
      validateWorkspace: vi.fn(),
      createCredentialProfile: vi.fn(),
      updateCredentialProfile: vi.fn(),
      deleteCredentialProfile: vi.fn(),
    });
  });

  it('renders the active workspace and its actionable warning accessibly', () => {
    render(<WorkspaceSidebarSection />);

    expect(screen.getByText('Workspaces')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Annual Meeting, active workspace' })
    ).toHaveAttribute('aria-current', 'true');
    expect(
      screen.getByLabelText('Workspace needs attention: Relink the primary folder')
    ).toBeInTheDocument();
  });

  it('opens the create workflow and supports the all-workspaces session filter', () => {
    render(<WorkspaceSidebarSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Add workspace' }));
    expect(screen.getByRole('dialog')).toHaveTextContent('Workspace editor');

    fireEvent.click(screen.getByRole('button', { name: /All workspaces/i }));
    expect(setSessionWorkspaceFilterId).toHaveBeenCalledWith(null);
  });

  it('switches active workspace for future chats', () => {
    setActiveWorkspace.mockResolvedValue(workspace);
    render(<WorkspaceSidebarSection />);

    fireEvent.click(screen.getByRole('button', { name: 'Annual Meeting, active workspace' }));

    expect(setActiveWorkspace).toHaveBeenCalledWith('workspace-1');
  });

  it('exposes edit, duplicate, reveal, export, and delete actions', async () => {
    const user = userEvent.setup();
    duplicateWorkspace.mockResolvedValue(workspace);
    render(<WorkspaceSidebarSection />);

    await user.click(screen.getByRole('button', { name: 'Actions for Annual Meeting' }));
    expect(screen.getByRole('menuitem', { name: 'Edit' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Reveal primary folder' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Export metadata' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Delete' })).toHaveAttribute('data-disabled');
    await user.click(screen.getByRole('menuitem', { name: 'Duplicate' }));
    expect(duplicateWorkspace).toHaveBeenCalledWith('workspace-1');
  });

  it('confirms deletion and reports the exact workspace to the backend', async () => {
    const user = userEvent.setup();
    const context = vi.mocked(useWorkspace)();
    vi.mocked(useWorkspace).mockReturnValue({
      ...context,
      defaultWorkspaceId: 'another-workspace',
      deleteWorkspace,
    });
    Object.assign(window.electron, {
      showMessageBox: vi.fn().mockResolvedValue({ response: 1 }),
    });
    deleteWorkspace.mockResolvedValue(undefined);
    render(<WorkspaceSidebarSection />);

    await user.click(screen.getByRole('button', { name: 'Actions for Annual Meeting' }));
    await user.click(screen.getByRole('menuitem', { name: 'Delete' }));

    expect(window.electron.showMessageBox).toHaveBeenCalledWith(
      expect.objectContaining({
        message: expect.stringContaining('Sessions and files will not be deleted'),
      })
    );
    expect(deleteWorkspace).toHaveBeenCalledWith('workspace-1');
  });
});
