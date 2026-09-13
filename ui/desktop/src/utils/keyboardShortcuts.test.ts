import { describe, expect, it } from 'vitest';
import {
  getChatSubmitShortcutText,
  getShortcutAcceleratorProblem,
  isChatSubmitShortcut,
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
