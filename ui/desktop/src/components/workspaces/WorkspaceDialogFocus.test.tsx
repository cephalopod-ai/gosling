import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import { useArtifactRouter } from '../../contexts/ArtifactRouterContext';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { WorkspaceSidebarSection } from './WorkspaceSidebarSection';

vi.mock('../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListProviderModels: vi.fn(),
}));

vi.mock('../../acp/workspaces', async () => ({
  ...(await vi.importActual<typeof import('../../acp/workspaces')>('../../acp/workspaces')),
  acpExportWorkspace: vi.fn(),
  acpCreateWorkspaceOutput: vi.fn(),
  acpCredentialProfileUsage: vi.fn(),
  acpTestCredentialProfile: vi.fn(),
}));

vi.mock('../../acp/extensions', () => ({
  getConfiguredExtensions: vi.fn(async () => ({ extensions: [], warnings: [] })),
}));

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
}));

vi.mock('../../contexts/ArtifactRouterContext', () => ({
  useArtifactRouter: vi.fn(),
}));

const workspace = {
  id: 'workspace-1',
  schemaVersion: 1,
  name: 'Annual Meeting',
  workingFolder: '/projects/annual-meeting',
  folders: [],
  productOutputFolders: [],
  createdAt: '2026-07-18T00:00:00Z',
  updatedAt: '2026-07-18T00:00:00Z',
  lastOpenedAt: '2026-07-18T00:00:00Z',
};

const profile = {
  id: 'profile-1',
  name: 'Work key',
  providerOrServiceId: 'anthropic',
  authKind: 'config_fields',
  status: 'configured',
  source: 'workspace_secure_storage',
  nonSecretFields: {},
};

function Wrapper({ children }: { children: React.ReactNode }) {
  return (
    <IntlTestWrapper>
      <MemoryRouter>{children}</MemoryRouter>
    </IntlTestWrapper>
  );
}

describe('workspace dialogs return focus to the control that opened them', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(acpListProviderDetails).mockResolvedValue([]);
    vi.mocked(acpListProviderModels).mockResolvedValue([]);
    vi.mocked(useArtifactRouter).mockReturnValue({
      saveArtifact: vi.fn(),
      setVisibleSessionArtifacts: vi.fn(),
      setVisibleSessionWorkspaceId: vi.fn(),
    });
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [{ workspace, validation: { validForSession: true, issues: [] } }],
      activeWorkspace: workspace,
      activeWorkspaceId: workspace.id,
      defaultWorkspaceId: workspace.id,
      credentialProfiles: [profile],
      loading: false,
      error: null,
      sessionWorkspaceFilterId: null,
      setSessionWorkspaceFilterId: vi.fn(),
      refreshWorkspaces: vi.fn(),
      createWorkspace: vi.fn(),
      updateWorkspace: vi.fn(),
      duplicateWorkspace: vi.fn(),
      deleteWorkspace: vi.fn(),
      setActiveWorkspace: vi.fn(),
      validateWorkspace: vi.fn().mockResolvedValue({ validForSession: true, issues: [] }),
      createCredentialProfile: vi.fn(),
      updateCredentialProfile: vi.fn(),
      deleteCredentialProfile: vi.fn(),
    } as unknown as ReturnType<typeof useWorkspace>);
  });

  it('returns focus to the actions trigger and to Manage profiles after Escape', async () => {
    const user = userEvent.setup();
    render(<WorkspaceSidebarSection onNewChat={vi.fn()} unreadWorkspaceIds={new Set()} />, {
      wrapper: Wrapper,
    });
    const trigger = screen.getByRole('button', { name: 'Actions for Annual Meeting' });

    trigger.focus();
    await user.keyboard('{Enter}');
    await user.click(await screen.findByRole('menuitem', { name: 'Edit' }));
    const manageProfiles = await screen.findByRole('button', { name: 'Manage profiles' });

    manageProfiles.focus();
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog', { name: 'Credential profiles' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Credential profiles' })).not.toBeInTheDocument()
    );
    await waitFor(() => expect(manageProfiles).toHaveFocus());

    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    await waitFor(() => expect(trigger).toHaveFocus());
  });
});
