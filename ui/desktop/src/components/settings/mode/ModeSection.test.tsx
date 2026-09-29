import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { useConfig } from '../../ConfigContext';
import { ModeSection } from './ModeSection';

vi.mock('../../ConfigContext', () => ({
  useConfig: vi.fn(),
}));

vi.mock('../permission/PermissionRulesModal', () => ({
  default: () => null,
}));

const mockedUseConfig = vi.mocked(useConfig);

function renderSection(stored: Record<string, unknown>, contextConfig = stored) {
  mockedUseConfig.mockReturnValue({
    config: contextConfig,
    providersList: [],
    extensionsList: [],
    extensionWarnings: [],
    upsert: vi.fn(),
    read: vi.fn(async (key: string) => stored[key] ?? null),
    remove: vi.fn(),
    addExtension: vi.fn(),
    setExtensionEnabled: vi.fn(),
    removeExtension: vi.fn(),
    getProviders: vi.fn(),
    getExtensions: vi.fn(),
  });
  return render(<ModeSection />, { wrapper: IntlTestWrapper });
}

const maxTurnsInput = () => screen.getByRole('spinbutton') as HTMLInputElement;

describe('ModeSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows an invalid Max Turns value from config.yaml instead of an empty field', async () => {
    renderSection({ GOSLING_MAX_TURNS: 'plenty' });

    expect(
      await screen.findByText(
        'config.yaml has an invalid Max Turns value: "plenty". Gosling uses the default (1000) until you enter a whole number.'
      )
    ).toBeInTheDocument();
    expect(maxTurnsInput()).toHaveAttribute('aria-invalid', 'true');
    expect(maxTurnsInput()).toHaveAttribute('placeholder', 'plenty');
  });

  it('shows a valid Max Turns value without a warning', async () => {
    renderSection({ GOSLING_MAX_TURNS: 50 });

    await waitFor(() => expect(maxTurnsInput().value).toBe('50'));
    expect(maxTurnsInput()).not.toHaveAttribute('aria-invalid', 'true');
    expect(screen.queryByText(/invalid Max Turns value/)).not.toBeInTheDocument();
  });

  it('selects the mode stored in config.yaml even when the loaded config is stale', async () => {
    const { container } = renderSection({ GOSLING_MODE: 'approve' }, { GOSLING_MODE: 'auto' });

    const radio = (mode: string) =>
      container.querySelector(`input[name="modes"][value="${mode}"]`) as HTMLInputElement;
    await waitFor(() => expect(radio('approve').checked).toBe(true));
    expect(radio('auto').checked).toBe(false);
  });
});
