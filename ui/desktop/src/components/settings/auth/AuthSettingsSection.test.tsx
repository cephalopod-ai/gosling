import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, type RenderOptions } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AuthSettingsSection from './AuthSettingsSection';
import {
  acpAuthenticateProvider,
  acpDeleteProviderSecret,
  acpListProviderSecrets,
  type ProviderSecretDto,
} from '../../../acp/providers';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { toast } from 'react-toastify';

vi.mock('../../../acp/providers', () => ({
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
const mockedToast = vi.mocked(toast);
const mockedGetSetting = vi.mocked(window.electron.getSetting);
const mockedSetSetting = vi.mocked(window.electron.setSetting);

const renderWithIntl = (ui: React.ReactElement, options?: RenderOptions) =>
  render(ui, { wrapper: IntlTestWrapper, ...options });

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
    mockedGetSetting.mockResolvedValue([]);
    mockedSetSetting.mockResolvedValue(undefined);
  });

  it('renders an empty state when no credentials are stored', async () => {
    renderWithIntl(<AuthSettingsSection />);

    expect(screen.getByText('Loading credentials...')).toBeInTheDocument();
    expect(
      await screen.findByText('No locally stored provider credentials were found.')
    ).toBeInTheDocument();
  });

  it('renders provider credentials with storage and expiry status', async () => {
    mockedListProviderSecrets.mockResolvedValue([
      {
        ...providerSecret,
        expiresAt: '2027-01-01T12:00:00Z',
        status: 'valid',
      },
    ]);

    renderWithIntl(<AuthSettingsSection />);

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('OPENAI_API_KEY')).toBeInTheDocument();
    expect(screen.getByText('Secret store')).toBeInTheDocument();
    expect(screen.getByText(/Expires/)).toBeInTheDocument();
  });

  it('does not render an expiry badge when expiry is unknown', async () => {
    mockedListProviderSecrets.mockResolvedValue([providerSecret]);

    renderWithIntl(<AuthSettingsSection />);

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('Secret store')).toBeInTheDocument();
    expect(screen.queryByText('Expiry unknown')).not.toBeInTheDocument();
    expect(screen.queryByText(/Expires/)).not.toBeInTheDocument();
  });

  it('deletes a credential after confirmation and refreshes the list', async () => {
    const user = userEvent.setup();
    mockedListProviderSecrets.mockResolvedValueOnce([providerSecret]).mockResolvedValueOnce([]);

    renderWithIntl(<AuthSettingsSection />);

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

  it('adds a VPS secret profile and saves it to settings', async () => {
    const user = userEvent.setup();

    renderWithIntl(<AuthSettingsSection />);

    expect(await screen.findByText('Local Secret Profiles')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'VPS server' }));

    expect(await screen.findByDisplayValue('VPS Server')).toBeInTheDocument();
    expect(screen.getByDisplayValue('VPS_SERVER_URL')).toBeInTheDocument();
    expect(screen.getByDisplayValue('VPS_SERVER_LOGIN')).toBeInTheDocument();
    expect(screen.getByDisplayValue('VPS_SERVER_PASSWORD')).toBeInTheDocument();
    expect(screen.getByText('Authentication + config')).toBeInTheDocument();
    expect(
      screen.getByDisplayValue('This is the password and login to manage the VPS server.')
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(mockedSetSetting).toHaveBeenCalledWith(
        'managedSecretProfiles',
        expect.arrayContaining([
          expect.objectContaining({
            name: 'VPS Server',
            template: 'vps',
            useFor: 'both',
            entries: expect.arrayContaining([
              expect.objectContaining({ key: 'VPS_SERVER_URL' }),
              expect.objectContaining({ key: 'VPS_SERVER_LOGIN' }),
              expect.objectContaining({ key: 'VPS_SERVER_PASSWORD' }),
            ]),
          }),
        ])
      );
    });
  });

  it('adds a Supabase project profile and saves it to settings', async () => {
    const user = userEvent.setup();

    renderWithIntl(<AuthSettingsSection />);

    expect(await screen.findByText('Local Secret Profiles')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Supabase project' }));

    expect(await screen.findByDisplayValue('Supabase Project')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SUPABASE_PROJECT_URL')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SUPABASE_PROJECT_REF')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SUPABASE_ANON_KEY')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SUPABASE_SERVICE_ROLE_KEY')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SUPABASE_DB_PASSWORD')).toBeInTheDocument();
    expect(screen.getByText('Authentication + config')).toBeInTheDocument();

    await waitFor(() => {
      expect(mockedSetSetting).toHaveBeenCalledWith(
        'managedSecretProfiles',
        expect.arrayContaining([
          expect.objectContaining({
            name: 'Supabase Project',
            template: 'supabase',
            useFor: 'both',
            entries: expect.arrayContaining([
              expect.objectContaining({ key: 'SUPABASE_PROJECT_URL' }),
              expect.objectContaining({ key: 'SUPABASE_PROJECT_REF' }),
              expect.objectContaining({ key: 'SUPABASE_ANON_KEY' }),
              expect.objectContaining({ key: 'SUPABASE_SERVICE_ROLE_KEY' }),
              expect.objectContaining({ key: 'SUPABASE_DB_PASSWORD' }),
            ]),
          }),
        ])
      );
    });
  });
});
