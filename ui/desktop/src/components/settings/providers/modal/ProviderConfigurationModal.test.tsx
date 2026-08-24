import { describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ProviderConfigurationModal from './ProviderConfigurationModal';
import { IntlTestWrapper } from '../../../../i18n/test-utils';
import { acpAuthenticateProvider } from '../../../../acp/providers';
import type { ProviderDetails } from '../../../../types/providers';

vi.mock('../../../../acp/providers', () => ({
  acpAuthenticateProvider: vi.fn(),
  acpDeleteCustomProvider: vi.fn(),
  acpDeleteProviderConfig: vi.fn(),
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
