import { describe, expect, it } from 'vitest';
import {
  getChatSubmitShortcutText,
  getShortcutAcceleratorProblem,
  isBareEnter,
  isChatSubmitShortcut,
  isEnterRunSubmit,
  nextEnterRun,
  NO_ENTER_RUN,
} from './keyboardShortcuts';
import { defaultKeyboardShortcuts } from './settings';

const keyEvent = (
  overrides: Partial<Parameters<typeof isChatSubmitShortcut>[0]> = {}
): Parameters<typeof isChatSubmitShortcut>[0] => ({
  key: 'Enter',
  altKey: false,
  ctrlKey: false,
  metaKey: false,
  shiftKey: false,
  ...overrides,
});

describe('isChatSubmitShortcut', () => {
  it('leaves Enter and modified newline variants available for line breaks', () => {
    expect(isChatSubmitShortcut(keyEvent())).toBe(false);
    expect(isChatSubmitShortcut(keyEvent({ shiftKey: true }))).toBe(false);
    expect(isChatSubmitShortcut(keyEvent({ altKey: true }))).toBe(false);
  });

  it('accepts the platform submit modifiers', () => {
    expect(isChatSubmitShortcut(keyEvent({ metaKey: true }))).toBe(true);
    expect(isChatSubmitShortcut(keyEvent({ ctrlKey: true }))).toBe(true);
  });

  it('requires Enter without secondary modifiers', () => {
    expect(isChatSubmitShortcut(keyEvent({ key: 'N', ctrlKey: true }))).toBe(false);
    expect(isChatSubmitShortcut(keyEvent({ ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(isChatSubmitShortcut(keyEvent({ metaKey: true, altKey: true }))).toBe(false);
  });
});

describe('the Enter run that sends', () => {
  function run(gapsMs: number[]) {
    let state = NO_ENTER_RUN;
    let now = 1_000;
    const sent: boolean[] = [];
    for (const gap of [0, ...gapsMs]) {
      now += gap;
      state = nextEnterRun(state, now);
      sent.push(isEnterRunSubmit(state));
    }
    return sent;
  }

  it('sends on the third Enter within a second, not before', () => {
    expect(run([200, 200])).toEqual([false, false, true]);
  });

  it('does not send when the third Enter falls outside the window', () => {
    // Each gap is under a second, but the run spans more than one.
    expect(run([700, 700])).toEqual([false, false, false]);
  });

  it('starts a fresh run once the window lapses', () => {
    expect(run([1_500, 100, 100])).toEqual([false, false, false, true]);
  });

  it('counts only an unmodified Enter, leaving newline variants alone', () => {
    const base = { key: 'Enter', altKey: false, ctrlKey: false, metaKey: false, shiftKey: false };
    expect(isBareEnter(base)).toBe(true);
    expect(isBareEnter({ ...base, shiftKey: true })).toBe(false);
    expect(isBareEnter({ ...base, metaKey: true })).toBe(false);
    expect(isBareEnter({ ...base, ctrlKey: true })).toBe(false);
    expect(isBareEnter({ ...base, altKey: true })).toBe(false);
    expect(isBareEnter({ ...base, key: 'a' })).toBe(false);
  });
});

describe('getChatSubmitShortcutText', () => {
  it('uses the native macOS shortcut label', () => {
    expect(getChatSubmitShortcutText()).toBe('⌘Enter');
  });
});

describe('getShortcutAcceleratorProblem', () => {
  it('rejects accelerators without a Command, Control, or Alt modifier', () => {
    expect(getShortcutAcceleratorProblem('J')).toBe('missingModifier');
    expect(getShortcutAcceleratorProblem('Shift+J')).toBe('missingModifier');
  });

  it('rejects accelerators reserved for quit, window, and editing commands', () => {
    expect(getShortcutAcceleratorProblem('CommandOrControl+Q')).toBe('reserved');
    expect(getShortcutAcceleratorProblem('CommandOrControl+W')).toBe('reserved');
    expect(getShortcutAcceleratorProblem('CommandOrControl+C')).toBe('reserved');
  });

  it('accepts the default shortcuts and bare function keys', () => {
    for (const accelerator of Object.values(defaultKeyboardShortcuts)) {
      expect(getShortcutAcceleratorProblem(accelerator)).toBeNull();
    }
    expect(getShortcutAcceleratorProblem('CommandOrControl+Shift+Q')).toBeNull();
    expect(getShortcutAcceleratorProblem('Alt+J')).toBeNull();
    expect(getShortcutAcceleratorProblem('F5')).toBeNull();
  });
});
