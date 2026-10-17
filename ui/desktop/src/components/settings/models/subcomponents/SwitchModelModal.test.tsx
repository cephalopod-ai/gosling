import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpListProviderDetails, acpListProviderModels } from '../../../../acp/providers';
import { IntlTestWrapper } from '../../../../i18n/test-utils';
import type { ProviderDetails } from '../../../../types/providers';
import { SwitchModelModal } from './SwitchModelModal';

const changeModel = vi.fn();
const promo = 'meta-models/Muse-Spark-Open-816B-A42B-promo';
const llama = 'meta-llama/Llama-3.3-70B-Instruct-Turbo';

vi.mock('../../../ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({
    changeModel,
    currentModel: llama,
    currentProvider: 'together',
  }),
}));

vi.mock('../../../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListProviderModels: vi.fn(),
  acpReadThinkingEffort: vi.fn().mockResolvedValue(null),
  acpSaveThinkingEffort: vi.fn().mockResolvedValue(undefined),
  acpPreviewSessionHandoff: vi.fn(),
}));

vi.mock('../predefinedModelsUtils', () => ({
  shouldShowPredefinedModels: () => false,
  getPredefinedModelsFromEnv: () => [],
}));

function provider(name: string, displayName: string): ProviderDetails {
  return {
    name,
    is_configured: true,
    manages_own_context: false,
    provider_type: 'Declarative',
    metadata: {
      name,
      display_name: displayName,
      description: '',
      default_model: '',
      known_models: [],
      model_doc_link: '',
      config_keys: [],
    },
  };
}

function openModal() {
  return render(<SwitchModelModal sessionId={null} onClose={vi.fn()} setView={vi.fn()} />, {
    wrapper: IntlTestWrapper,
  });
}

describe('SwitchModelModal model search', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    changeModel.mockResolvedValue(false);
    vi.mocked(acpListProviderDetails).mockResolvedValue([
      provider('together', 'Together AI'),
      provider('openai', 'OpenAI'),
      { ...provider('custom_local', 'Local'), provider_type: 'Custom' },
    ]);
    vi.mocked(acpListProviderModels).mockImplementation(async (name) =>
      (name === 'together'
        ? [llama, promo, 'deepseek-ai/DeepSeek-V3']
        : name === 'openai'
          ? ['gpt-4o', 'gpt-5.5']
          : ['local/Alpha', 'local/Beta']
      ).map((id) => ({ id, reasoning: false }))
    );
  });

  it('filters case-insensitively without changing the selection until a model is chosen', async () => {
    const user = userEvent.setup();
    openModal();
    const search = await screen.findByRole('searchbox', { name: 'Search models' });
    await waitFor(() => expect(search).toBeEnabled());
    await user.type(search, 'mUsE');
    expect(changeModel).not.toHaveBeenCalled();
    expect(screen.getByText(llama)).toBeInTheDocument();

    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    expect(screen.queryByRole('option', { name: llama })).not.toBeInTheDocument();
    await user.click(screen.getByRole('option', { name: promo }));
    await user.click(screen.getByRole('button', { name: 'Select model' }));
    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(
        null,
        expect.objectContaining({ name: promo, provider: 'together' }),
        expect.any(Object)
      )
    );
  });

  it('restores the full list when cleared and resets search across cached provider changes', async () => {
    const user = userEvent.setup();
    openModal();
    const search = await screen.findByRole('searchbox', { name: 'Search models' });
    await waitFor(() => expect(search).toBeEnabled());
    await user.type(search, 'muse');
    await user.clear(search);
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    expect(screen.getByRole('option', { name: llama })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: promo })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await user.type(search, 'muse');

    await user.click(screen.getByRole('combobox', { name: 'Provider, type to search' }));
    await user.click(screen.getByRole('option', { name: 'OpenAI' }));
    expect(search).toHaveValue('');
    await waitFor(() => expect(search).toBeEnabled());
    await user.type(search, '5.5');
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    expect(screen.getByRole('option', { name: 'gpt-5.5' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'gpt-4o' })).not.toBeInTheDocument();
    await user.keyboard('{Escape}');

    await user.click(screen.getByRole('combobox', { name: 'Provider, type to search' }));
    await user.click(screen.getByRole('option', { name: 'Together AI' }));
    expect(search).toHaveValue('');
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    expect(screen.getByRole('option', { name: llama })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: promo })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'gpt-5.5' })).not.toBeInTheDocument();
  });

  it('keeps explicit entry of an unlisted model available after an unmatched search', async () => {
    const user = userEvent.setup();
    openModal();
    const search = await screen.findByRole('searchbox', { name: 'Search models' });
    await waitFor(() => expect(search).toBeEnabled());
    await user.type(search, 'private/new-model');
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    await user.click(screen.getByRole('option', { name: 'Use: "private/new-model"' }));
    await user.click(screen.getByRole('button', { name: 'Select model' }));
    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(
        null,
        expect.objectContaining({ name: 'private/new-model', provider: 'together' }),
        expect.any(Object)
      )
    );
  });

  it('searches custom-provider models and allows keyboard selection', async () => {
    const user = userEvent.setup();
    openModal();
    const search = await screen.findByRole('searchbox', { name: 'Search models' });
    await waitFor(() => expect(search).toBeEnabled());
    await user.click(screen.getByRole('combobox', { name: 'Provider, type to search' }));
    await user.click(screen.getByRole('option', { name: 'Local' }));
    await waitFor(() => expect(search).toBeEnabled());
    await user.type(search, 'BETA');
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    expect(screen.getByRole('option', { name: 'local/Beta' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'local/Alpha' })).not.toBeInTheDocument();
    await user.keyboard('{ArrowDown}{Enter}');
    await user.click(screen.getByRole('button', { name: 'Select model' }));
    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(
        null,
        expect.objectContaining({ name: 'local/Beta', provider: 'custom_local' }),
        expect.any(Object)
      )
    );
  });
});
