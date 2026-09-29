import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { useConfig } from '../../ConfigContext';
import ConfigSettings from './ConfigSettings';

vi.mock('../../ConfigContext', () => ({
  useConfig: vi.fn(),
}));

vi.mock('../../../toasts', () => ({
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

const mockedUseConfig = vi.mocked(useConfig);
const upsert = vi.fn();

const config = {
  GOSLING_PROVIDER: 'openai',
  GOSLING_MAX_TURNS: 50,
  providers: {
    openai: { model: 'playtest-model', api_key: 'sk-should-not-render' },
  },
};

function renderEditor() {
  mockedUseConfig.mockReturnValue({
    config,
    providersList: [],
    extensionsList: [],
    extensionWarnings: [],
    upsert,
    read: vi.fn(),
    remove: vi.fn(),
    addExtension: vi.fn(),
    setExtensionEnabled: vi.fn(),
    removeExtension: vi.fn(),
    getProviders: vi.fn(),
    getExtensions: vi.fn(),
  });
  render(<ConfigSettings />, { wrapper: IntlTestWrapper });
}

async function openEditor(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: 'Edit Configuration' }));
  return screen.findByRole('dialog');
}

describe('ConfigSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    upsert.mockResolvedValue(undefined);
  });

  it('shows a nested map read-only instead of an editable "[object Object]" string', async () => {
    const user = userEvent.setup();
    renderEditor();
    const dialog = await openEditor(user);

    expect(within(dialog).queryByDisplayValue('[object Object]')).not.toBeInTheDocument();
    expect(within(dialog).queryByText('[object Object]')).not.toBeInTheDocument();
    expect(within(dialog).getByText(/"model": "playtest-model"/)).toBeInTheDocument();
    expect(within(dialog).queryByText(/sk-should-not-render/)).not.toBeInTheDocument();
    expect(within(dialog).getByText('Edit this value in config.yaml.')).toBeInTheDocument();
    expect(
      within(dialog)
        .getAllByRole('button', { name: /^Save / })
        .map((button) => button.getAttribute('aria-label'))
    ).toEqual(['Save Provider', 'Save Gosling Max Turns']);
  });

  it('still edits and saves a scalar value', async () => {
    const user = userEvent.setup();
    renderEditor();
    const dialog = await openEditor(user);

    const input = within(dialog).getByDisplayValue('50');
    await user.clear(input);
    await user.type(input, '75');
    await user.click(within(dialog).getByRole('button', { name: 'Save Gosling Max Turns' }));

    await waitFor(() => expect(upsert).toHaveBeenCalledWith('GOSLING_MAX_TURNS', '75', false));
    expect(upsert).toHaveBeenCalledTimes(1);
  });
});
