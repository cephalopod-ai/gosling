import { StrictMode } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import ModelsSection from './ModelsSection';

const mocks = vi.hoisted(() => ({
  acpReadDefaults: vi.fn(),
  acpListProviderDetails: vi.fn(),
  getCurrentModelDisplayName: vi.fn(),
  getCurrentProviderDisplayName: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock('../../../acp/providers', () => ({
  acpReadDefaults: mocks.acpReadDefaults,
  acpListProviderDetails: mocks.acpListProviderDetails,
  acpClearDefaults: vi.fn(),
}));

vi.mock('../../../toasts', () => ({ toastError: mocks.toastError }));

vi.mock('../../ModelAndProviderContext', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../ModelAndProviderContext')>()),
  useModelAndProvider: () => ({
    currentModel: null,
    currentProvider: null,
    getCurrentModelDisplayName: mocks.getCurrentModelDisplayName,
    getCurrentProviderDisplayName: mocks.getCurrentProviderDisplayName,
  }),
}));

vi.mock('./subcomponents/SwitchModelModal', () => ({ SwitchModelModal: () => null }));

function renderModels() {
  const setView = vi.fn();
  render(
    <StrictMode>
      <IntlTestWrapper>
        <ModelsSection setView={setView} />
      </IntlTestWrapper>
    </StrictMode>
  );
  return setView;
}

describe('ModelsSection provider state', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.acpReadDefaults.mockResolvedValue({ providerId: null, modelId: null });
    mocks.acpListProviderDetails.mockResolvedValue([
      { name: 'openai', metadata: { display_name: 'OpenAI' } },
    ]);
    mocks.getCurrentModelDisplayName.mockResolvedValue('');
    mocks.getCurrentProviderDisplayName.mockResolvedValue('');
  });

  it.each([null, ''])(
    'offers setup without a warning for an absent provider (%s)',
    async (providerId) => {
      mocks.acpReadDefaults.mockResolvedValue({ providerId, modelId: null });
      const setView = renderModels();

      expect(await screen.findByText('No provider configured')).toBeInTheDocument();
      expect(mocks.toastError).not.toHaveBeenCalled();
      expect(mocks.acpListProviderDetails).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole('button', { name: 'Configure providers' }));
      expect(setView).toHaveBeenCalledWith('ConfigureProviders');
    }
  );

  it('retains a configured provider display name', async () => {
    mocks.getCurrentModelDisplayName.mockResolvedValue('GPT-5');
    mocks.getCurrentProviderDisplayName.mockResolvedValue('OpenAI');
    renderModels();

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(screen.getByText('GPT-5')).toBeInTheDocument();
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it('resolves a configured provider through the metadata fallback', async () => {
    mocks.acpReadDefaults.mockResolvedValue({ providerId: 'openai', modelId: 'gpt-5' });
    renderModels();

    expect(await screen.findByText('OpenAI')).toBeInTheDocument();
    expect(mocks.toastError).not.toHaveBeenCalled();
  });

  it('warns once for an unknown configured provider during effect replay', async () => {
    mocks.acpReadDefaults.mockResolvedValue({
      providerId: 'unknown-playtest-provider',
      modelId: 'test',
    });
    renderModels();

    expect(await screen.findByText('unknown-playtest-provider')).toBeInTheDocument();
    await waitFor(() => expect(mocks.toastError).toHaveBeenCalledTimes(1));
    expect(mocks.toastError).toHaveBeenCalledWith({
      title: 'Provider name lookup',
      msg: 'Unknown provider in config -- please inspect your config.yaml',
    });
  });
});
