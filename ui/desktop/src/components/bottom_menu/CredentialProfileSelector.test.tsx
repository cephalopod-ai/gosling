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

const profiles = [
  {
    id: 'profile-1',
    name: 'Team OpenAI',
    providerOrServiceId: 'openai',
    authKind: 'config_fields' as const,
    configuredSecretFields: ['OPENAI_API_KEY'],
    nonSecretFields: {},
    status: 'configured' as const,
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
    createdAt: '2026-07-20T00:00:00Z',
    updatedAt: '2026-07-20T00:00:00Z',
  },
];

describe('CredentialProfileSelector', () => {
  beforeEach(() => {
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

    expect(screen.getAllByText('Team OpenAI')).toHaveLength(2);
    expect(screen.getByText('Personal Anthropic')).toBeInTheDocument();
    expect(screen.getByText('Start a new chat to use another profile.')).toBeInTheDocument();

    await user.click(screen.getByRole('menuitem', { name: 'Manage credential profiles' }));
    expect(screen.getByRole('dialog', { name: '' })).toHaveTextContent('Credential profile manager');
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
});
