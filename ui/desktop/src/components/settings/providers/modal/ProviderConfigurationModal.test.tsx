import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ProviderConfigurationModal from './ProviderConfigurationModal';
import { IntlTestWrapper } from '../../../../i18n/test-utils';
import { acpAuthenticateProvider, acpSaveProviderConfig } from '../../../../acp/providers';
import type { ProviderDetails } from '../../../../types/providers';

vi.mock('../../../../acp/providers', () => ({
  acpAuthenticateProvider: vi.fn(),
  acpDeleteCustomProvider: vi.fn(),
  acpDeleteProviderConfig: vi.fn(),
  acpReadProviderConfig: vi.fn().mockResolvedValue([]),
  acpSaveProviderConfig: vi.fn(),
}));

vi.mock('../../../ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({
    getCurrentModelAndProvider: vi.fn(),
  }),
}));

const provider: ProviderDetails = {
  name: 'test_oauth',
  is_configured: false,
  manages_own_context: false,
  provider_type: 'Builtin',
  metadata: {
    name: 'test_oauth',
    display_name: 'Test OAuth',
    description: 'Sign in with Test OAuth',
    default_model: 'test-model',
    model_doc_link: '',
    config_keys: [
      {
        name: 'TEST_OAUTH_TOKEN',
        required: true,
        secret: true,
        oauth_flow: true,
      },
    ],
    known_models: [],
  },
};

describe('ProviderConfigurationModal', () => {
  it('keeps Muse sign-in guidance while saving an optional executable path', async () => {
    const user = userEvent.setup();
    render(
      <ProviderConfigurationModal
        provider={{
          ...provider,
          name: 'muse-code',
          metadata: {
            ...provider.metadata,
            name: 'muse-code',
            display_name: 'Meta Muse Code (Subscription/CLI)',
            description: 'Use your signed-in Muse CLI.',
            setup_steps: ['Sign in with `muse login`'],
            config_keys: [
              {
                name: 'MUSE_CODE_COMMAND',
                required: false,
                secret: false,
                oauth_flow: false,
                primary: true,
              },
            ],
          },
        }}
        onClose={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByText('muse login')).toBeInTheDocument();
    expect(screen.queryByText(/Add your API key/)).toBeNull();
    await user.type(await screen.findByRole('textbox'), '/opt/muse/bin/muse');
    await user.click(screen.getByRole('button', { name: 'Submit' }));
    expect(acpSaveProviderConfig).toHaveBeenCalledWith('muse-code', [
      { key: 'MUSE_CODE_COMMAND', value: '/opt/muse/bin/muse' },
    ]);
  });

  it('shows the ACP error detail when OAuth sign-in fails', async () => {
    const user = userEvent.setup();
    vi.mocked(acpAuthenticateProvider).mockRejectedValue({
      error: {
        code: -32603,
        message: 'Internal error',
        data: 'Failed to authenticate provider: OAuth flow failed: access_denied',
      },
    });

    render(<ProviderConfigurationModal provider={provider} onClose={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: 'Sign in with Test OAuth' }));

    await waitFor(() => {
      expect(acpAuthenticateProvider).toHaveBeenCalledWith('test_oauth');
    });
    expect(
      await screen.findByText(
        'OAuth login failed: Internal error: Failed to authenticate provider: OAuth flow failed: access_denied'
      )
    ).toBeInTheDocument();
  });
});
