/**
 * @vitest-environment jsdom
 */
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import SettingsView from './SettingsView';

vi.mock('../Layout/MainPanelLayout', () => ({
  MainPanelLayout: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('../ui/scroll-area', () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

vi.mock('./models/ModelsSection', () => ({ default: () => <div>Models settings</div> }));
vi.mock('./app/ExternalBackendSection', () => ({ default: () => <div>Session settings</div> }));
vi.mock('./app/AppSettingsSection', () => ({ default: () => <div>App settings</div> }));
vi.mock('./config/ConfigSettings', () => ({ default: () => <div>Configuration settings</div> }));
vi.mock('./PromptsSettingsSection', () => ({ default: () => <div>Prompt settings</div> }));
vi.mock('./chat/ChatSettingsSection', () => ({ default: () => <div>Chat settings</div> }));
vi.mock('./keyboard/KeyboardShortcutsSection', () => ({
  default: () => <div>Keyboard settings</div>,
}));
vi.mock('./auth/AuthSettingsSection', () => ({ default: () => <div>Auth settings</div> }));
vi.mock('../skills/SkillsView', () => ({
  default: ({ embedded }: { embedded?: boolean }) => (
    <div>Skills catalog {embedded ? 'embedded' : 'standalone'}</div>
  ),
}));
vi.mock('../extensions/ExtensionsView', () => ({
  default: ({ embedded }: { embedded?: boolean }) => (
    <div>Extensions catalog {embedded ? 'embedded' : 'standalone'}</div>
  ),
}));
vi.mock('../../utils/analytics', () => ({ trackSettingsTabViewed: vi.fn() }));

describe('SettingsView catalog tabs', () => {
  it('opens Skills from a settings section deep link', async () => {
    render(
      <SettingsView onClose={vi.fn()} setView={vi.fn()} viewOptions={{ section: 'skills' }} />,
      { wrapper: IntlTestWrapper }
    );

    await waitFor(() => expect(screen.getByText('Skills catalog embedded')).toBeVisible());
    expect(screen.getByTestId('settings-skills-tab')).toHaveAttribute('data-state', 'active');
  });

  it('opens Extensions as an embedded settings tab', async () => {
    const user = userEvent.setup();
    render(<SettingsView onClose={vi.fn()} setView={vi.fn()} viewOptions={{}} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByTestId('settings-extensions-tab'));

    expect(screen.getByText('Extensions catalog embedded')).toBeVisible();
  });
});
