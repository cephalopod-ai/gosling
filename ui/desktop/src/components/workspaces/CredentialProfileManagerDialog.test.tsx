import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpListProviderDetails } from '../../acp/providers';
import { acpCredentialProfileUsage, acpTestCredentialProfile } from '../../acp/workspaces';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { CredentialProfileManagerDialog } from './CredentialProfileManagerDialog';

vi.mock('../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
}));

vi.mock('../../acp/workspaces', () => ({
  acpCredentialProfileUsage: vi.fn(),
  acpTestCredentialProfile: vi.fn(),
}));

vi.mock('../../contexts/WorkspaceContext', () => ({
  useWorkspace: vi.fn(),
}));

const createCredentialProfile = vi.fn();
const deleteCredentialProfile = vi.fn();

describe('CredentialProfileManagerDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(acpListProviderDetails).mockResolvedValue([
      {
        name: 'anthropic',
        provider_type: 'Builtin',
        is_configured: false,
        metadata: {
          name: 'anthropic',
          display_name: 'Anthropic',
          description: '',
          default_model: 'claude',
          known_models: [],
          model_doc_link: '',
          config_keys: [
            {
              name: 'ANTHROPIC_API_KEY',
              required: true,
              secret: true,
              oauth_flow: false,
            },
          ],
        },
      },
    ]);
    vi.mocked(useWorkspace).mockReturnValue({
      workspaces: [],
      activeWorkspace: null,
      activeWorkspaceId: '',
      defaultWorkspaceId: '',
      credentialProfiles: [],
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
      createCredentialProfile,
      updateCredentialProfile: vi.fn(),
      deleteCredentialProfile,
    });
  });

  it('keeps secrets in password inputs and clears them after cancel', async () => {
    const user = userEvent.setup();
    render(<CredentialProfileManagerDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'New profile' }));
    await user.type(screen.getByLabelText('Profile name'), 'AFRL Anthropic');
    await user.selectOptions(screen.getByLabelText('Provider or service'), 'anthropic');
    const secret = await screen.findByLabelText('ANTHROPIC_API_KEY (required)');
    expect(secret).toHaveAttribute('type', 'password');
    await user.type(secret, 'SENTINEL_WORKSPACE_SECRET');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    await user.click(screen.getByRole('button', { name: 'New profile' }));
    await user.selectOptions(screen.getByLabelText('Provider or service'), 'anthropic');
    expect(await screen.findByLabelText('ANTHROPIC_API_KEY (required)')).toHaveValue('');
    expect(screen.queryByDisplayValue('SENTINEL_WORKSPACE_SECRET')).not.toBeInTheDocument();
  });

  it('submits a secret once and removes it from renderer state after success', async () => {
    const user = userEvent.setup();
    createCredentialProfile.mockResolvedValue({});
    render(<CredentialProfileManagerDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'New profile' }));
    await user.type(screen.getByLabelText('Profile name'), 'Personal Anthropic');
    await user.selectOptions(screen.getByLabelText('Provider or service'), 'anthropic');
    await user.type(
      await screen.findByLabelText('ANTHROPIC_API_KEY (required)'),
      'SENTINEL_WORKSPACE_SECRET'
    );
    await user.click(screen.getByRole('button', { name: 'Save profile' }));

    expect(createCredentialProfile).toHaveBeenCalledWith(
      expect.objectContaining({
        secretFields: [{ key: 'ANTHROPIC_API_KEY', value: 'SENTINEL_WORKSPACE_SECRET' }],
      })
    );
    expect(screen.queryByDisplayValue('SENTINEL_WORKSPACE_SECRET')).not.toBeInTheDocument();
  });

  it('redacts secret-shaped provider errors and clears the failed value', async () => {
    const user = userEvent.setup();
    createCredentialProfile.mockRejectedValue(
      new Error('provider rejected api_key=SENTINEL_WORKSPACE_SECRET')
    );
    render(<CredentialProfileManagerDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'New profile' }));
    await user.type(screen.getByLabelText('Profile name'), 'Personal Anthropic');
    await user.selectOptions(screen.getByLabelText('Provider or service'), 'anthropic');
    await user.type(
      await screen.findByLabelText('ANTHROPIC_API_KEY (required)'),
      'SENTINEL_WORKSPACE_SECRET'
    );
    await user.click(screen.getByRole('button', { name: 'Save profile' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('api_key=[redacted]');
    expect(screen.queryByText(/SENTINEL_WORKSPACE_SECRET/)).not.toBeInTheDocument();
    expect(screen.getByLabelText('ANTHROPIC_API_KEY (required)')).toHaveValue('');
  });

  it('requires explicit confirmation and names dependent workspaces before deletion', async () => {
    const user = userEvent.setup();
    const context = vi.mocked(useWorkspace)();
    vi.mocked(useWorkspace).mockReturnValue({
      ...context,
      credentialProfiles: [
        {
          id: 'profile-1',
          name: 'AFRL Anthropic',
          providerOrServiceId: 'anthropic',
          authKind: 'config_fields',
          configuredSecretFields: ['ANTHROPIC_API_KEY'],
          nonSecretFields: {},
          status: 'configured',
          source: 'workspace_secure_storage',
          createdAt: '2026-07-18T00:00:00Z',
          updatedAt: '2026-07-18T00:00:00Z',
        },
      ],
    });
    vi.mocked(acpCredentialProfileUsage).mockResolvedValue({
      workspaces: [{ workspaceId: 'workspace-1', workspaceName: 'Annual Meeting' }],
    });
    Object.assign(window.electron, {
      showMessageBox: vi.fn().mockResolvedValue({ response: 1 }),
    });
    deleteCredentialProfile.mockResolvedValue(undefined);
    render(<CredentialProfileManagerDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Delete AFRL Anthropic' }));

    expect(window.electron.showMessageBox).toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringContaining('Annual Meeting') })
    );
    expect(deleteCredentialProfile).toHaveBeenCalledWith('profile-1', true);
  });

  it('reports provider test support without exposing a credential value', async () => {
    const user = userEvent.setup();
    const context = vi.mocked(useWorkspace)();
    vi.mocked(useWorkspace).mockReturnValue({
      ...context,
      credentialProfiles: [
        {
          id: 'profile-1',
          name: 'AFRL Anthropic',
          providerOrServiceId: 'anthropic',
          authKind: 'config_fields',
          configuredSecretFields: ['ANTHROPIC_API_KEY'],
          nonSecretFields: {},
          status: 'configured',
          source: 'workspace_secure_storage',
          createdAt: '2026-07-18T00:00:00Z',
          updatedAt: '2026-07-18T00:00:00Z',
        },
      ],
    });
    vi.mocked(acpTestCredentialProfile).mockResolvedValue({
      supported: false,
      status: 'configured',
    });
    render(<CredentialProfileManagerDialog open onOpenChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Test AFRL Anthropic' }));

    expect(acpTestCredentialProfile).toHaveBeenCalledWith('profile-1');
    expect(await screen.findByRole('status')).toHaveTextContent(
      'Live credential testing is not supported for anthropic. Secure profile status: configured.'
    );
    expect(screen.queryByText(/SENTINEL_WORKSPACE_SECRET/)).not.toBeInTheDocument();
  });
});
