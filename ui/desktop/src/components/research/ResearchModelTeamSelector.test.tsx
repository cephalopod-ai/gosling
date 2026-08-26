import { useState } from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpListProviderDetails, acpListProviderModels } from '../../acp/providers';
import type { ResearchTeamConfiguration } from '../../types/sessionExperience';
import { ResearchModelTeamSelector } from './ResearchModelTeamSelector';

vi.mock('../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListProviderModels: vi.fn(),
}));

const provider = (name: string, displayName: string) => ({
  name,
  is_configured: true,
  manages_own_context: false,
  provider_type: 'Builtin' as const,
  metadata: {
    name,
    display_name: displayName,
    description: '',
    default_model: '',
    model_doc_link: '',
    model_selection_hint: null,
    config_keys: [],
    known_models: [],
    setup_steps: [],
  },
});

function Harness() {
  const [configuration, setConfiguration] = useState<ResearchTeamConfiguration>({
    mode: 'solo',
    models: [],
  });
  const [validation, setValidation] = useState<string | null>(null);
  return (
    <>
      <ResearchModelTeamSelector
        currentProvider="codex"
        currentModel="gpt-5.6-sol"
        value={configuration}
        onChange={setConfiguration}
        onValidationChange={setValidation}
      />
      <output data-testid="configuration">{JSON.stringify(configuration)}</output>
      <output data-testid="validation">{validation ?? 'valid'}</output>
    </>
  );
}

describe('ResearchModelTeamSelector', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(acpListProviderDetails).mockResolvedValue([
      provider('codex', 'Codex'),
      provider('claude', 'Anthropic'),
      provider('groq', 'Groq'),
    ]);
    vi.mocked(acpListProviderModels).mockImplementation(async (providerId) => {
      if (providerId === 'codex') return [{ id: 'gpt-5.6-sol' }];
      if (providerId === 'claude') return [{ id: 'claude-opus-5' }];
      return [{ id: 'llama-4' }];
    });
  });

  it('defaults to optional Solo mode and fills exact distinct seats for Dual and Trio', async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await waitFor(() => expect(screen.getByText('Codex — gpt-5.6-sol')).toBeInTheDocument());
    expect(screen.getByLabelText('Lead model')).toHaveValue('');
    expect(screen.getByTestId('validation')).toHaveTextContent('valid');

    await user.click(screen.getByRole('radio', { name: /Dual/ }));
    await waitFor(() =>
      expect(JSON.parse(screen.getByTestId('configuration').textContent ?? '{}')).toMatchObject({
        mode: 'dual',
        models: [
          { provider: 'codex', model: 'gpt-5.6-sol' },
          { provider: 'claude', model: 'claude-opus-5' },
        ],
      })
    );
    expect(screen.getByTestId('validation')).toHaveTextContent('valid');
    const researcherSelect = screen.getByLabelText('Researcher 2') as HTMLSelectElement;
    expect(
      Array.from(researcherSelect.options).find(
        (option) => option.textContent === 'Codex — gpt-5.6-sol'
      )
    ).toBeDisabled();

    await user.click(screen.getByRole('radio', { name: /Trio/ }));
    await waitFor(() =>
      expect(JSON.parse(screen.getByTestId('configuration').textContent ?? '{}')).toMatchObject({
        mode: 'trio',
        models: [
          { provider: 'codex', model: 'gpt-5.6-sol' },
          { provider: 'claude', model: 'claude-opus-5' },
          { provider: 'groq', model: 'llama-4' },
        ],
      })
    );
  });

  it('keeps unavailable team sizes disabled when too few models load', async () => {
    vi.mocked(acpListProviderDetails).mockResolvedValue([provider('codex', 'Codex')]);
    vi.mocked(acpListProviderModels).mockResolvedValue([{ id: 'gpt-5.6-sol' }]);

    render(<Harness />);

    await waitFor(() => expect(screen.getByRole('radio', { name: /Dual/ })).toBeDisabled());
    expect(screen.getByRole('radio', { name: /Trio/ })).toBeDisabled();
    expect(screen.getByText(/Configure at least two models/)).toBeInTheDocument();
    expect(screen.getByTestId('validation')).toHaveTextContent('valid');
  });
});
