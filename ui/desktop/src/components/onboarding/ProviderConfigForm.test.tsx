import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpReadProviderConfig } from '../../acp/providers';
import { IntlTestWrapper } from '../../i18n/test-utils';
import type { ProviderDetails } from '../../types/providers';
import ProviderConfigForm from './ProviderConfigForm';

vi.mock('../../acp/providers', () => ({
  acpAuthenticateProvider: vi.fn(),
  acpReadProviderConfig: vi.fn(),
}));

const openAi: ProviderDetails = {
  name: 'openai',
  is_configured: false,
  manages_own_context: false,
  provider_type: 'Preferred',
  metadata: {
    name: 'openai',
    display_name: 'OpenAI',
    description: 'GPT models, including OpenAI compatible ones',
    default_model: 'gpt-5',
    known_models: [],
    model_doc_link: '',
    config_keys: [
      { name: 'OPENAI_API_KEY', required: false, secret: true, oauth_flow: false, primary: true },
      {
        name: 'OPENAI_HOST',
        required: true,
        secret: false,
        oauth_flow: false,
        primary: true,
        default: 'https://api.openai.com',
      },
    ],
  },
};

describe('ProviderConfigForm', () => {
  beforeEach(() => {
    vi.mocked(acpReadProviderConfig).mockResolvedValue([]);
  });

  it('masks a typed API key while leaving non-secret fields readable', async () => {
    const user = userEvent.setup();
    render(<ProviderConfigForm provider={openAi} onConfigured={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    const key = await screen.findByPlaceholderText('Your API key');
    await user.type(key, 'sk-test-secret');

    expect(key).toHaveAttribute('type', 'password');
    expect(key).toHaveAttribute('autocomplete', 'off');
    expect(key).toHaveValue('sk-test-secret');
    expect(screen.getByDisplayValue('https://api.openai.com')).toHaveAttribute('type', 'text');
  });
});
