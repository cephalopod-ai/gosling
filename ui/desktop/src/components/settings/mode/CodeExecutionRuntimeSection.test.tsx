import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { useConfig } from '../../ConfigContext';
import { CodeExecutionRuntimeSection } from './CodeExecutionRuntimeSection';

vi.mock('../../ConfigContext', () => ({
  useConfig: vi.fn(),
}));

const mockedUseConfig = vi.mocked(useConfig);
const upsert = vi.fn();

describe('CodeExecutionRuntimeSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    upsert.mockResolvedValue(undefined);
  });

  it('defaults to enabled when the setting is absent', () => {
    mockedUseConfig.mockReturnValue({
      config: {},
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

    render(<CodeExecutionRuntimeSection />, { wrapper: IntlTestWrapper });

    expect(screen.getByText('Code execution runtime')).toBeInTheDocument();
    expect(screen.getByText('Enabled')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Enabled/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
  });

  it('displays disabled for an invalid stored value', () => {
    mockedUseConfig.mockReturnValue({
      config: { GOSLING_CODE_EXECUTION_RUNTIME: 'invalid' },
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

    render(<CodeExecutionRuntimeSection />, { wrapper: IntlTestWrapper });

    expect(screen.getByRole('button', { name: /Disabled/ })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
  });

  it('persists disabled and shows restart-required copy', async () => {
    const user = userEvent.setup();
    mockedUseConfig.mockReturnValue({
      config: { GOSLING_CODE_EXECUTION_RUNTIME: 'enabled' },
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

    render(<CodeExecutionRuntimeSection />, { wrapper: IntlTestWrapper });

    await user.click(screen.getByRole('button', { name: /Disabled/ }));

    await waitFor(() => {
      expect(upsert).toHaveBeenCalledWith('GOSLING_CODE_EXECUTION_RUNTIME', 'disabled', false);
    });
    expect(
      screen.getByText('Restart Gosling or the configured backend for this change to take effect.')
    ).toBeInTheDocument();
  });
});
