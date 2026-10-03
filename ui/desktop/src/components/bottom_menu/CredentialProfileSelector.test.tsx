import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { CredentialProfileSelector } from './CredentialProfileSelector';

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
}));

vi.mock('../workspaces/CredentialProfileManagerDialog', () => ({
  CredentialProfileManagerDialog: ({ open }: { open: boolean }) =>
    open ? <div role="dialog">Credential profile manager</div> : null,
}));

vi.mock('../workspaces/AuthenticationDialog', () => ({
  AuthenticationDialog: ({
    open,
    target,
  }: {
    open: boolean;
    target: { id: string; type: string };
  }) =>
    open ? (
      <div role="dialog">
        Authentication for {target.type} {target.id}
      </div>
    ) : null,
}));

const profiles = [
  {
    id: 'profile-1',
    name: 'Team OpenAI',
    providerOrServiceId: 'openai',
    authKind: 'config_fields' as const,
    configuredSecretFields: ['OPENAI_API_KEY'],
    nonSecretFields: {},
    status: 'configured' as const,
    source: 'workspace_secure_storage' as const,
    createdAt: '2026-07-20T00:00:00Z',
    updatedAt: '2026-07-20T00:00:00Z',
  },
  {
    id: 'profile-2',
    name: 'Personal Anthropic',
    providerOrServiceId: 'anthropic',
    authKind: 'config_fields' as const,
    configuredSecretFields: ['ANTHROPIC_API_KEY'],
    nonSecretFields: {},
    status: 'configured' as const,
    source: 'workspace_secure_storage' as const,
    createdAt: '2026-07-20T00:00:00Z',
    updatedAt: '2026-07-20T00:00:00Z',
  },
];

describe('CredentialProfileSelector', () => {
  beforeEach(() => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    );
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [],
      activeWorkspace: null,
      activeWorkspaceId: null,
      defaultWorkspaceId: null,
      credentialProfiles: profiles,
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
      validateWorkspace: vi.fn(),
      createCredentialProfile: vi.fn(),
      updateCredentialProfile: vi.fn(),
      deleteCredentialProfile: vi.fn(),
    });
  });

  it('shows the profile pinned to the active chat and opens the credential manager', async () => {
    const user = userEvent.setup();
    render(
      <CredentialProfileSelector
        credentialProfileId="profile-1"
        credentialProfileName="Team OpenAI"
      />,
      { wrapper: IntlTestWrapper }
    );

    await user.click(screen.getByRole('button', { name: 'Credential for this chat: Team OpenAI' }));

    expect(screen.getAllByText('Team OpenAI')).toHaveLength(3);
    expect(screen.getByText('Personal Anthropic')).toBeInTheDocument();
    expect(
      screen.getByText(
        'Credentials are pinned when a chat starts. Start a new chat to use another profile.'
      )
    ).toBeInTheDocument();

    await user.click(screen.getByRole('menuitem', { name: 'Manage credential profiles' }));
    expect(screen.getByRole('dialog', { name: '' })).toHaveTextContent(
      'Credential profile manager'
    );
  });

  it('lets the header chip shrink and truncate in a narrow header row', () => {
    render(
      <CredentialProfileSelector
        credentialProfileId="profile-1"
        credentialProfileName="Team OpenAI"
        surface="header"
      />,
      { wrapper: IntlTestWrapper }
    );

    const chip = screen.getByRole('button', { name: 'Credential for this chat: Team OpenAI' });
    expect(chip).toHaveClass('min-w-0');
    expect(screen.getByText('Team OpenAI')).toHaveClass('truncate');
  });

  it('names the app default credentials when no profile is pinned', async () => {
    const user = userEvent.setup();
    render(<CredentialProfileSelector surface="header" />, { wrapper: IntlTestWrapper });

    const chip = screen.getByRole('button', { name: 'Credential for this chat: App default' });
    expect(chip).toHaveTextContent('App default');
    expect(screen.queryByText('No credential')).not.toBeInTheDocument();

    await user.click(chip);

    expect(screen.getByText("Uses the app's default provider credentials")).toBeInTheDocument();
    expect(screen.queryByText('No credential profile is pinned')).not.toBeInTheDocument();
  });

  it('keeps the saved profile name visible when the pinned profile is unavailable', async () => {
    const user = userEvent.setup();
    vi.mocked(useWorkspace).mockReturnValue({
      ...vi.mocked(useWorkspace)(),
      credentialProfiles: [],
    });

    render(
      <CredentialProfileSelector
        credentialProfileId="deleted-profile"
        credentialProfileName="Retired OpenAI"
      />,
      { wrapper: IntlTestWrapper }
    );

    await user.click(
      screen.getByRole('button', { name: 'Credential for this chat: Retired OpenAI' })
    );

    expect(screen.getAllByText('Retired OpenAI')).toHaveLength(2);
    expect(screen.getByText('Profile unavailable')).toBeInTheDocument();
  });

  it('opens authentication controls for the active chat', async () => {
    const user = userEvent.setup();
    render(
      <CredentialProfileSelector
        session={{
          id: 'chat-current',
          name: 'Chat',
          created_at: '',
          updated_at: '',
          message_count: 0,
          extension_data: {},
          working_dir: '/tmp',
        }}
        credentialProfileId="profile-1"
      />,
      { wrapper: IntlTestWrapper }
    );
    await user.click(screen.getByRole('button', { name: 'Credential for this chat: Team OpenAI' }));
    await user.click(screen.getByRole('menuitem', { name: 'Manage chat authentication' }));
    expect(screen.getByRole('dialog')).toHaveTextContent('Authentication for session chat-current');
  });

  it('labels a disconnected chat without falling back to the app default label', () => {
    render(
      <CredentialProfileSelector
        session={{
          id: 'disconnected',
          name: 'Chat',
          created_at: '',
          updated_at: '',
          message_count: 0,
          extension_data: {},
          working_dir: '/tmp',
          provider_auth_disconnected: true,
        }}
      />,
      { wrapper: IntlTestWrapper }
    );
    expect(
      screen.getByRole('button', { name: 'Credential for this chat: Disconnected' })
    ).toBeInTheDocument();
    expect(screen.queryByText('App default')).not.toBeInTheDocument();
  });
});
