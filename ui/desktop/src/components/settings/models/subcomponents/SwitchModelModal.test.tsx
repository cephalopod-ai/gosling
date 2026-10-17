import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acpListProviderDetails,
  acpListProviderModels,
  acpPreviewSessionHandoff,
  acpReadDefaults,
} from '../../../../acp/providers';
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
  acpReadDefaults: vi.fn(),
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
    vi.mocked(acpReadDefaults).mockResolvedValue({ providerId: 'together', modelId: llama });
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

  it('warns when the context window is unknown and submits a manual limit', async () => {
    const user = userEvent.setup();
    openModal();
    await screen.findByText(/context window could not be discovered/);
    const context = screen.getByRole('textbox', { name: 'Context window (tokens)' });
    await user.type(context, '1048576');
    await user.click(screen.getByRole('button', { name: 'Select model' }));
    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(
        null,
        expect.objectContaining({ context_limit: 1_048_576 }),
        expect.any(Object)
      )
    );
  });

  it.each(['0', '-1', '1.5', 'abc', '9007199254740992'])(
    'rejects invalid context window %s with an actionable error',
    async (value) => {
      const user = userEvent.setup();
      openModal();
      const context = await screen.findByRole('textbox', { name: 'Context window (tokens)' });
      await user.type(context, value);
      expect(screen.getByRole('alert')).toHaveTextContent('Enter a whole number of tokens');
      expect(screen.getByRole('button', { name: 'Select model' })).toBeDisabled();
      expect(changeModel).not.toHaveBeenCalled();
      await user.clear(context);
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Select model' })).toBeEnabled();
    }
  );

  it('uses discovered context metadata and clears overrides when the model changes', async () => {
    vi.mocked(acpListProviderModels).mockResolvedValue([
      { id: llama, contextLimit: 128_000 },
      { id: promo, contextLimit: 1_048_576 },
    ]);
    const user = userEvent.setup();
    openModal();
    const search = await screen.findByRole('searchbox', { name: 'Search models' });
    await waitFor(() => expect(search).toBeEnabled());
    const context = screen.getByRole('textbox', { name: 'Context window (tokens)' });
    await user.type(context, '64000');
    await user.click(screen.getByRole('combobox', { name: 'Select model' }));
    await user.click(screen.getByRole('option', { name: promo }));
    expect(context).toHaveValue('');
    expect(context).toHaveAttribute('placeholder', 'Automatic: 1,048,576 tokens');
    expect(screen.queryByText(/context window could not be discovered/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Select model' }));
    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(
        null,
        expect.objectContaining({ name: promo, context_limit: 1_048_576 }),
        expect.any(Object)
      )
    );
  });

  it('reloads a saved default override', async () => {
    vi.mocked(acpReadDefaults).mockResolvedValue({
      providerId: 'together',
      modelId: llama,
      contextLimit: 64_000,
    });
    openModal();
    await waitFor(() =>
      expect(screen.getByRole('textbox', { name: 'Context window (tokens)' })).toHaveValue('64000')
    );
  });

  it('requires a fresh checkpoint preview after editing the context window', async () => {
    vi.mocked(acpPreviewSessionHandoff).mockResolvedValue({
      snapshot: {
        continuityClass: 'summarized_handoff',
        coverage: { coveredMessageCount: 0, totalMessageCount: 0, sourceHash: 'hash' },
        omissions: [],
        truncations: [],
        redactions: [],
        redactionReport: { redactionCount: 0, truncatedItemCount: 0 },
        interruptedOperations: [],
        pendingApprovals: [],
      },
      expectedCurrentGeneration: 0,
      toolContinuity: { enabledExtensions: [], tools: [], goslingMode: 'auto' },
    } as unknown as Awaited<ReturnType<typeof acpPreviewSessionHandoff>>);
    const user = userEvent.setup();
    render(<SwitchModelModal sessionId="session-1" onClose={vi.fn()} setView={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });
    const context = await screen.findByRole('textbox', { name: 'Context window (tokens)' });
    await user.type(context, '64000');
    await user.click(screen.getByRole('button', { name: 'Review checkpoint' }));
    await waitFor(() =>
      expect(acpPreviewSessionHandoff).toHaveBeenCalledWith('session-1', 'together', llama, 64_000)
    );
    await user.clear(context);
    await user.type(context, '32000');
    await user.click(screen.getByRole('button', { name: 'Review checkpoint' }));
    await waitFor(() =>
      expect(acpPreviewSessionHandoff).toHaveBeenLastCalledWith(
        'session-1',
        'together',
        llama,
        32_000
      )
    );
    expect(changeModel).not.toHaveBeenCalled();
  });
});
