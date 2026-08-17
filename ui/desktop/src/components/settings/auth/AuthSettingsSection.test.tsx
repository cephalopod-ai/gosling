import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, type RenderOptions } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AuthSettingsSection from './AuthSettingsSection';
import {
  acpAddCustomProviderSecret,
  acpAuthenticateProvider,
  acpDeleteProviderSecret,
  acpListProviderSecrets,
  type ProviderSecretDto,
} from '../../../acp/providers';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { toast } from 'react-toastify';

vi.mock('../../../acp/providers', () => ({
  acpAddCustomProviderSecret: vi.fn(),
  acpAuthenticateProvider: vi.fn(),
  acpListProviderSecrets: vi.fn(),
  acpDeleteProviderSecret: vi.fn(),
}));

vi.mock('../../ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({
    currentProvider: 'openai',
  }),
}));

vi.mock('react-toastify', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

const mockedListProviderSecrets = vi.mocked(acpListProviderSecrets);
const mockedDeleteProviderSecret = vi.mocked(acpDeleteProviderSecret);
const mockedAcpAuthenticateProvider = vi.mocked(acpAuthenticateProvider);
const mockedAddCustomProviderSecret = vi.mocked(acpAddCustomProviderSecret);
const mockedToast = vi.mocked(toast);

const renderWithIntl = (ui: React.ReactElement, options?: RenderOptions) =>
  render(ui, { wrapper: IntlTestWrapper, ...options });

const renderSection = () => renderWithIntl(<AuthSettingsSection />);

const providerSecret: ProviderSecretDto = {
  id: 'secret_store:openai:OPENAI_API_KEY',
  provider: 'openai',
  providerDisplayName: 'OpenAI',
  name: 'OPENAI_API_KEY',
  storage: 'secret_store',
  expiresAt: null,
  status: 'unknown',
  configured: true,
  hasSecret: true,
  canDelete: true,
  canConfigure: false,
  configureProvider: null,
};

describe('AuthSettingsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedListProviderSecrets.mockResolvedValue([]);
    mockedDeleteProviderSecret.mockResolvedValue(undefined);
    mockedAcpAuthenticateProvider.mockResolvedValue(undefined);
    mockedAddCustomProviderSecret.mockResolvedValue(undefined);
  });

  it('renders an empty state when no credentials are stored', async () => {
    renderSection();

    expect(screen.getByText('Loading credentials...')).toBeInTheDocument();
    expect(
      await screen.findByText('No locally stored provider credentials were found.')
    ).toBeInTheDocument();
  });

  it('adds a generic name/value credential without navigating away', async () => {
    const user = userEvent.setup();
    mockedListProviderSecrets.mockResolvedValueOnce([]).mockResolvedValueOnce([
      {
        ...providerSecret,
        id: 'custom_secret:MY_API_KEY',
        provider: 'custom',
        providerDisplayName: 'Custom',
        name: 'MY_API_KEY',
      },
    ]);

    renderSection();
    await screen.findByText('No locally stored provider credentials were found.');

    await user.click(screen.getByRole('button', { name: 'Add credential' }));
    await user.type(screen.getByPlaceholderText('MY_API_KEY'), 'MY_API_KEY');
    await user.type(screen.getByPlaceholderText('Secret value'), 'shh-its-a-secret');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(mockedAddCustomProviderSecret).toHaveBeenCalledWith('MY_API_KEY', 'shh-its-a-secret');
    });
    await waitFor(() => {
      expect(mockedToast.success).toHaveBeenCalledWith('Credential added');
    });
    expect(await screen.findByText('MY_API_KEY')).toBeInTheDocument();
    // The form closes and does not leave the Auth tab.
    expect(screen.queryByPlaceholderText('MY_API_KEY')).not.toBeInTheDocument();
  });

  it('rejects a credential name with invalid characters before submitting', async () => {
    const user = userEvent.setup();
    renderSection();
    await screen.findByText('No locally stored provider credentials were found.');

    await user.click(screen.getByRole('button', { name: 'Add credential' }));
    await user.type(screen.getByPlaceholderText('MY_API_KEY'), 'has a space');
    await user.type(screen.getByPlaceholderText('Secret value'), 'value');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    expect(
      await screen.findByText('Name must contain only letters, numbers, "_", or "-".')
    ).toBeInTheDocument();
    expect(mockedAddCustomProviderSecret).not.toHaveBeenCalled();
  });

  it('renders provider credentials with storage and expiry status', async () => {
    mockedListProviderSecrets.mockResolvedValue([
      {
        ...providerSecret,
        expiresAt: '2027-01-01T12:00:00Z',
        status: 'valid',
      },
    ]);

    renderSection();

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('OPENAI_API_KEY')).toBeInTheDocument();
    expect(screen.getByText('Secret store')).toBeInTheDocument();
    expect(screen.getByText(/Expires/)).toBeInTheDocument();
  });

  it('does not render an expiry badge when expiry is unknown', async () => {
    mockedListProviderSecrets.mockResolvedValue([providerSecret]);

    renderSection();

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('Secret store')).toBeInTheDocument();
    expect(screen.queryByText('Expiry unknown')).not.toBeInTheDocument();
    expect(screen.queryByText(/Expires/)).not.toBeInTheDocument();
  });

  it('deletes a credential after confirmation and refreshes the list', async () => {
    const user = userEvent.setup();
    mockedListProviderSecrets.mockResolvedValueOnce([providerSecret]).mockResolvedValueOnce([]);

    renderSection();

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Delete credential' }));

    expect(
      screen.getByText('Delete the OPENAI_API_KEY credential for OpenAI?')
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        'This is the active provider. New requests may fail until you configure another credential.'
      )
    ).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(mockedDeleteProviderSecret).toHaveBeenCalledWith('secret_store:openai:OPENAI_API_KEY');
    });
    await waitFor(() => {
      expect(mockedToast.success).toHaveBeenCalledWith('Credential deleted');
    });
    expect(
      await screen.findByText('No locally stored provider credentials were found.')
    ).toBeInTheDocument();
  });
});
