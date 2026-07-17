import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, type RenderOptions } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ExternalBackendSection from './ExternalBackendSection';
import { IntlTestWrapper } from '../../../i18n/test-utils';

const mockedGetSetting = vi.mocked(window.electron.getSetting);
const mockedSetSetting = vi.mocked(window.electron.setSetting);

const renderWithIntl = (ui: React.ReactElement, options?: RenderOptions) =>
  render(ui, { wrapper: IntlTestWrapper, ...options });

describe('ExternalBackendSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedGetSetting.mockResolvedValue({
      enabled: false,
      url: '',
      secret: '',
      certFingerprint: '',
    });
    mockedSetSetting.mockResolvedValue(undefined);
  });

  it('keeps the switch enabled after a successful save and shows no error', async () => {
    const user = userEvent.setup();
    renderWithIntl(<ExternalBackendSection />);

    const toggle = await screen.findByRole('switch');
    await user.click(toggle);

    await waitFor(() => {
      expect(mockedSetSetting).toHaveBeenCalledWith(
        'externalGoslingd',
        expect.objectContaining({ enabled: true })
      );
    });

    // Success is only reflected once the save has actually resolved.
    await waitFor(() => {
      expect(toggle).toHaveAttribute('data-state', 'checked');
    });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('reverts the switch and shows an error when the save fails, instead of a false success', async () => {
    const user = userEvent.setup();
    mockedSetSetting.mockRejectedValueOnce(new Error('disk full'));
    renderWithIntl(<ExternalBackendSection />);

    const toggle = await screen.findByRole('switch');
    expect(toggle).toHaveAttribute('data-state', 'unchecked');

    await user.click(toggle);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('disk full');

    // The switch must not appear to have saved the change that actually failed.
    await waitFor(() => {
      expect(toggle).toHaveAttribute('data-state', 'unchecked');
    });
  });

  it('reverts an unsaved URL edit and shows an error when persistence fails on blur', async () => {
    const user = userEvent.setup();
    mockedGetSetting.mockResolvedValue({
      enabled: true,
      url: 'http://old.example.com',
      secret: '',
      certFingerprint: '',
    });
    mockedSetSetting.mockRejectedValueOnce(new Error('network down'));

    renderWithIntl(<ExternalBackendSection />);

    const urlInput = await screen.findByLabelText('Backend Base URL');
    expect(urlInput).toHaveValue('http://old.example.com');

    await user.clear(urlInput);
    await user.type(urlInput, 'http://new.example.com');
    await user.tab();

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('network down');

    // The field should fall back to the last known-persisted value, not the
    // failed edit, so the UI does not appear saved.
    await waitFor(() => {
      expect(urlInput).toHaveValue('http://old.example.com');
    });
  });

  it('clears a previous save error once a later save succeeds', async () => {
    const user = userEvent.setup();
    mockedSetSetting.mockRejectedValueOnce(new Error('disk full'));
    renderWithIntl(<ExternalBackendSection />);

    const toggle = await screen.findByRole('switch');
    await user.click(toggle);
    await screen.findByRole('alert');

    mockedSetSetting.mockResolvedValueOnce(undefined);
    await user.click(toggle);

    await waitFor(() => {
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });
});
