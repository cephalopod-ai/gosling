import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useConfig } from '../../ConfigContext';
import { SummarizerSection } from './SummarizerSection';

vi.mock('../../ConfigContext', () => ({
  useConfig: vi.fn(),
}));

vi.mock('../../../acp/providers', () => ({
  acpListSummarizerModels: vi.fn(async () => []),
}));

function renderSection(stored: Record<string, unknown>) {
  vi.mocked(useConfig).mockReturnValue({
    config: {},
    providersList: [],
    extensionsList: [],
    extensionWarnings: [],
    upsert: vi.fn(async () => undefined),
    read: vi.fn(async (key: string) => stored[key] ?? null),
    remove: vi.fn(),
    addExtension: vi.fn(),
    setExtensionEnabled: vi.fn(),
    removeExtension: vi.fn(),
    getProviders: vi.fn(),
    getExtensions: vi.fn(),
  });
  return render(<SummarizerSection />);
}

describe('SummarizerSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('names the mode radios and the endpoint, model and timeout fields', async () => {
    renderSection({
      GOSLING_SUMMARIZER: 'shadow',
      GOSLING_SUMMARIZER_ENDPOINT: 'http://localhost:11434/v1',
    });

    expect(await screen.findByRole('radio', { name: 'Shadow' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Off' })).not.toBeChecked();
    expect(screen.getByRole('radio', { name: 'On' })).not.toBeChecked();

    expect(
      screen.getByRole('textbox', { name: 'Endpoint (local OpenAI-compatible URL)' })
    ).toHaveValue('http://localhost:11434/v1');
    expect(screen.getByRole('textbox', { name: 'Model' })).toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: 'Timeout (ms)' })).toBeInTheDocument();
  });
});
