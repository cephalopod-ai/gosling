import { render as rtlRender, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AuthenticationResponse_unstable as AuthenticationResponse } from '@repo-makeover/gosling-sdk';
import {
  acpReadAuthentication,
  acpSetExtensionAuthentication,
  acpSetProviderAuthentication,
} from '../../acp/authentication';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { AuthenticationDialog } from './AuthenticationDialog';
import { IntlTestWrapper } from '../../i18n/test-utils';

function render(ui: Parameters<typeof rtlRender>[0]) {
  return rtlRender(ui, { wrapper: IntlTestWrapper });
}

vi.mock('../../acp/authentication', () => ({
  acpReadAuthentication: vi.fn(),
  acpSetExtensionAuthentication: vi.fn(),
  acpSetProviderAuthentication: vi.fn(),
}));
vi.mock('../../contexts/WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
vi.mock('./CredentialProfileManagerDialog', () => ({ CredentialProfileManagerDialog: () => null }));

const target = { type: 'session' as const, id: 'chat-1' };
const response: AuthenticationResponse = {
  providerId: 'openai',
  credentialProfileId: 'profile-1',
  credentialProfileName: 'Team',
  settings: { providerDisconnected: false, extensions: {} },
  extensions: [
    { key: 'examplemcp', name: 'Example MCP', supportsOauth: true, secretFields: ['API_TOKEN'] },
  ],
};
const refreshWorkspaces = vi.fn();
const broadcastWorkspaceChange = vi.fn();

describe('AuthenticationDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(acpReadAuthentication).mockResolvedValue(response);
    vi.mocked(acpSetProviderAuthentication).mockResolvedValue(response);
    vi.mocked(acpSetExtensionAuthentication).mockResolvedValue(response);
    vi.mocked(useWorkspace).mockReturnValue({
      credentialProfiles: [
        { id: 'profile-1', name: 'Team', providerOrServiceId: 'openai', status: 'configured' },
        { id: 'profile-2', name: 'Personal', providerOrServiceId: 'openai', status: 'configured' },
        {
          id: 'different',
          name: 'Anthropic',
          providerOrServiceId: 'anthropic',
          status: 'configured',
        },
        {
          id: 'missing',
          name: 'Unconfigured',
          providerOrServiceId: 'openai',
          status: 'needs_authentication',
        },
      ].map((profile) => ({
        ...profile,
        authKind: 'config_fields' as const,
        source: 'workspace_secure_storage' as const,
        createdAt: '',
        updatedAt: '',
        status:
          profile.status === 'configured'
            ? ('configured' as const)
            : ('needs_authentication' as const),
      })),
      refreshWorkspaces,
      workspaces: [],
      activeWorkspace: null,
      activeWorkspaceId: null,
      defaultWorkspaceId: null,
      loading: false,
      error: null,
      sessionWorkspaceFilterId: null,
      setSessionWorkspaceFilterId: vi.fn(),
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
    Object.assign(window.electron, { broadcastWorkspaceChange });
  });

  it('connects only compatible configured profiles to the selected chat', async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    render(
      <AuthenticationDialog open target={target} onOpenChange={vi.fn()} onChanged={onChanged} />
    );
    await screen.findByRole('button', { name: 'Connect profile' });
    expect(acpReadAuthentication).toHaveBeenCalledWith(target);
    expect(screen.queryByRole('option', { name: 'Anthropic' })).not.toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'Unconfigured' })).not.toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText('Credential profile'), 'profile-2');
    await user.click(screen.getByRole('button', { name: 'Connect profile' }));
    expect(acpSetProviderAuthentication).toHaveBeenCalledWith(target, 'profile-2');
    await waitFor(() => expect(onChanged).toHaveBeenCalledWith(response));
    expect(refreshWorkspaces).not.toHaveBeenCalled();
  });

  it('disconnects explicitly and shows disconnection instead of app defaults', async () => {
    const user = userEvent.setup();
    vi.mocked(acpSetProviderAuthentication).mockResolvedValue({
      ...response,
      credentialProfileId: null,
      credentialProfileName: null,
      settings: { providerDisconnected: true },
    });
    render(<AuthenticationDialog open target={target} onOpenChange={vi.fn()} />);
    await user.click(await screen.findByRole('button', { name: 'Disconnect provider' }));
    expect(acpSetProviderAuthentication).toHaveBeenCalledWith(target, null);
    expect(await screen.findByText('Disconnected · openai')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Disconnect provider' })).toBeDisabled();
  });

  it('updates workspace defaults and refreshes workspace metadata without changing chats', async () => {
    const user = userEvent.setup();
    const workspaceTarget = { type: 'workspace' as const, id: 'workspace-2' };
    render(<AuthenticationDialog open target={workspaceTarget} onOpenChange={vi.fn()} />);
    expect(
      screen.getByText(/Changes apply to new chats.*Existing chats keep their authentication/)
    ).toBeInTheDocument();
    await user.click(await screen.findByRole('button', { name: 'Disconnect provider' }));
    expect(acpSetProviderAuthentication).toHaveBeenCalledWith(workspaceTarget, null);
    await waitFor(() => expect(broadcastWorkspaceChange).toHaveBeenCalledOnce());
    expect(refreshWorkspaces).toHaveBeenCalledOnce();
  });

  it('sends secret inputs only to the selected extension and clears them after saving', async () => {
    const user = userEvent.setup();
    render(<AuthenticationDialog open target={target} onOpenChange={vi.fn()} />);
    const input = await screen.findByLabelText('API_TOKEN');
    expect(input).toHaveAttribute('type', 'password');
    await user.type(input, 'fixture-only-token');
    await user.click(screen.getByRole('button', { name: 'Save credentials' }));
    expect(acpSetExtensionAuthentication).toHaveBeenCalledWith({
      target,
      name: 'Example MCP',
      connected: true,
      signIn: false,
      secretFields: [{ key: 'API_TOKEN', value: 'fixture-only-token' }],
    });
    await waitFor(() => expect(input).toHaveValue(''));
    expect(acpSetProviderAuthentication).not.toHaveBeenCalled();
  });

  it('disconnects an MCP account without supplying secret fields or signing in', async () => {
    const user = userEvent.setup();
    render(<AuthenticationDialog open target={target} onOpenChange={vi.fn()} />);
    await user.type(await screen.findByLabelText('API_TOKEN'), 'unsaved-value');
    await user.click(screen.getByRole('button', { name: 'Disconnect Example MCP' }));
    expect(acpSetExtensionAuthentication).toHaveBeenCalledWith({
      target,
      name: 'Example MCP',
      connected: false,
      signIn: false,
      secretFields: [],
    });
  });

  it('reads persisted state after a failed reconnect and keeps the error visible', async () => {
    const user = userEvent.setup();
    const onChanged = vi.fn();
    vi.mocked(acpSetExtensionAuthentication).mockRejectedValue(
      new Error('Authentication was saved, but the extension could not connect')
    );
    render(
      <AuthenticationDialog open target={target} onOpenChange={vi.fn()} onChanged={onChanged} />
    );
    await user.click(await screen.findByRole('button', { name: 'Reconnect Example MCP' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Authentication was saved');
    await waitFor(() => expect(acpReadAuthentication).toHaveBeenCalledTimes(2));
    expect(onChanged).toHaveBeenCalledWith(response);
  });

  it('prevents closing and concurrent mutations during an OAuth request', async () => {
    const user = userEvent.setup();
    let resolve: (value: AuthenticationResponse) => void = () => {};
    vi.mocked(acpSetExtensionAuthentication).mockReturnValue(
      new Promise((done) => {
        resolve = done;
      })
    );
    const onOpenChange = vi.fn();
    render(<AuthenticationDialog open target={target} onOpenChange={onOpenChange} />);
    await user.click(await screen.findByRole('button', { name: 'Sign in to Example MCP' }));
    expect(acpSetExtensionAuthentication).toHaveBeenCalledWith({
      target,
      name: 'Example MCP',
      connected: true,
      signIn: true,
      secretFields: [],
    });
    expect(screen.getAllByRole('button', { name: 'Close' })[0]).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Disconnect provider' })).toBeDisabled();
    await user.keyboard('{Escape}');
    expect(onOpenChange).not.toHaveBeenCalled();
    resolve(response);
    await waitFor(() => expect(screen.getAllByRole('button', { name: 'Close' })[0]).toBeEnabled());
  });
});
