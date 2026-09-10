import { describe, expect, it } from 'vitest';
import { getChatSubmitShortcutText, isChatSubmitShortcut } from './keyboardShortcuts';

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
