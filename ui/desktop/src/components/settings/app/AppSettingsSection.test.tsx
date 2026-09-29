import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import AppSettingsSection from './AppSettingsSection';

vi.mock('./UpdateSection', () => ({ default: () => null }));
vi.mock('./CrashRecoveryPolicySection', () => ({ default: () => null }));
vi.mock('./OutputFileExtensionsSection', () => ({ default: () => null }));
vi.mock('./ContextHistorySettings', () => ({ default: () => null }));
vi.mock('../../GoslingSidebar/ThemeSelector', () => ({ default: () => null }));
vi.mock('../../../utils/analytics', () => ({ trackSettingToggled: vi.fn() }));

describe('AppSettingsSection', () => {
  beforeEach(() => {
    Object.assign(window.electron, {
      getMenuBarIconState: vi.fn(async () => true),
      getWakelockState: vi.fn(async () => false),
      getDockIconState: vi.fn(async () => true),
      openNotificationsSettings: vi.fn(),
    });
    Object.defineProperty(window, 'appConfig', {
      configurable: true,
      writable: true,
      value: { get: vi.fn(() => '1.0.0') },
    });
  });

  it('names every switch after the setting it controls', async () => {
    render(<AppSettingsSection />, { wrapper: IntlTestWrapper });

    await waitFor(() =>
      expect(screen.getByRole('switch', { name: 'Prevent Sleep' })).not.toBeChecked()
    );
    for (const name of [
      'Task completion notifications',
      'Menu bar icon',
      'Dock icon',
      'Cost Tracking',
    ]) {
      expect(screen.getByRole('switch', { name })).toBeChecked();
    }
  });
});
