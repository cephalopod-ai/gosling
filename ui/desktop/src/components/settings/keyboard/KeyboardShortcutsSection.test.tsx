/**
 * @vitest-environment jsdom
 */
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { defaultKeyboardShortcuts } from '../../../utils/settings';
import KeyboardShortcutsSection from './KeyboardShortcutsSection';

vi.mock('../../../utils/analytics', () => ({ trackSettingToggled: vi.fn() }));

const rowFor = (label: string): HTMLElement =>
  screen.getByRole('heading', { name: label }).parentElement!.parentElement!;

const startRecording = async (label: string) => {
  const user = userEvent.setup();
  await user.click(within(rowFor(label)).getByRole('button', { name: `Change ${label} shortcut` }));
  return screen.getByText('Press shortcut...').parentElement!;
};

const shortcutLabels = [
  'Focus Gosling Window',
  'Quick Launcher',
  'New Chat',
  'New Chat Window',
  'Open Directory',
  'Settings',
  'Find',
  'Find Next',
  'Find Previous',
  'Always on Top',
  'Toggle Navigation',
];

describe('KeyboardShortcutsSection', () => {
  beforeEach(() => {
    vi.mocked(window.electron.getSetting).mockImplementation(async (key: string) =>
      key === 'keyboardShortcuts' ? { ...defaultKeyboardShortcuts } : undefined
    );
    vi.mocked(window.electron.setSetting).mockClear();
    vi.mocked(window.electron.showMessageBox).mockClear();
  });

  it('refuses reserved and modifier-less shortcuts', async () => {
    render(<KeyboardShortcutsSection />, { wrapper: IntlTestWrapper });
    await screen.findByRole('heading', { name: 'New Chat' });

    const recorder = await startRecording('New Chat');
    fireEvent.keyDown(recorder, { key: 'q', code: 'KeyQ', metaKey: true });
    expect(screen.getByRole('alert')).toHaveTextContent('reserved by the system');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();

    fireEvent.click(screen.getByText('⌘+Q'));
    fireEvent.keyDown(screen.getByText('Press shortcut...').parentElement!, {
      key: 'j',
      code: 'KeyJ',
    });
    expect(screen.getByRole('alert')).toHaveTextContent('must include Command, Control');
    expect(screen.getByRole('button', { name: 'Save' })).toBeDisabled();
    expect(window.electron.setSetting).not.toHaveBeenCalled();
  });

  it('saves a valid shortcut and notes that menu shortcuts apply after restart', async () => {
    render(<KeyboardShortcutsSection />, { wrapper: IntlTestWrapper });
    await screen.findByRole('heading', { name: 'Toggle Navigation' });

    const recorder = await startRecording('Toggle Navigation');
    fireEvent.keyDown(recorder, { key: 'k', code: 'KeyK', metaKey: true, shiftKey: true });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Save' }));

    expect(window.electron.setSetting).toHaveBeenCalledWith(
      'keyboardShortcuts',
      expect.objectContaining({ toggleNavigation: 'CommandOrControl+Shift+K' })
    );
    expect(await screen.findByText('Restart Required')).toBeInTheDocument();
  });

  it('names each binding Change button and enable switch after the binding', async () => {
    render(<KeyboardShortcutsSection />, { wrapper: IntlTestWrapper });
    await screen.findByRole('heading', { name: 'New Chat' });

    for (const label of shortcutLabels) {
      const row = rowFor(label);
      expect(within(row).getByRole('button', { name: `Change ${label} shortcut` })).toBeVisible();
      expect(within(row).getByRole('switch', { name: `Enable ${label} shortcut` })).toBeChecked();
    }
    expect(screen.getAllByRole('switch')).toHaveLength(shortcutLabels.length);
  });

  it('cancels recording on Escape instead of recording Esc', async () => {
    render(<KeyboardShortcutsSection />, { wrapper: IntlTestWrapper });
    await screen.findByRole('heading', { name: 'New Chat' });

    const recorder = await startRecording('New Chat');
    fireEvent.keyDown(recorder, { key: 'Escape', code: 'Escape' });

    expect(screen.queryByText('Press shortcut...')).not.toBeInTheDocument();
    expect(screen.queryByText('Esc')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(
      within(rowFor('New Chat')).getByRole('button', { name: 'Change New Chat shortcut' })
    ).toBeVisible();
    expect(within(rowFor('New Chat')).getByText('⌘+T')).toBeVisible();
    expect(window.electron.setSetting).not.toHaveBeenCalled();
  });

  it('resets a single changed binding to its default', async () => {
    vi.mocked(window.electron.getSetting).mockImplementation(async (key: string) =>
      key === 'keyboardShortcuts'
        ? { ...defaultKeyboardShortcuts, newChat: 'CommandOrControl+Shift+J' }
        : undefined
    );
    render(<KeyboardShortcutsSection />, { wrapper: IntlTestWrapper });
    await screen.findByRole('heading', { name: 'New Chat' });

    expect(
      within(rowFor('Find')).queryByRole('button', { name: 'Reset Find shortcut to default' })
    ).not.toBeInTheDocument();

    await userEvent
      .setup()
      .click(
        within(rowFor('New Chat')).getByRole('button', {
          name: 'Reset New Chat shortcut to default',
        })
      );

    expect(window.electron.showMessageBox).not.toHaveBeenCalled();
    expect(window.electron.setSetting).toHaveBeenCalledWith('keyboardShortcuts', {
      ...defaultKeyboardShortcuts,
    });
    expect(await within(rowFor('New Chat')).findByText('⌘+T')).toBeVisible();
    expect(
      within(rowFor('New Chat')).queryByRole('button', {
        name: 'Reset New Chat shortcut to default',
      })
    ).not.toBeInTheDocument();
  });
});
