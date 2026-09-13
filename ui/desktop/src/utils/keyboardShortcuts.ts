import type { IntlShape } from 'react-intl';

function isMac(): boolean {
  return window.electron?.platform === 'darwin';
}

export interface ChatSubmitKeyEvent {
  key: string;
  altKey: boolean;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export function isChatSubmitShortcut(event: ChatSubmitKeyEvent): boolean {
  return (
    event.key === 'Enter' && (event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey
  );
}

export function getChatSubmitShortcutText(): string {
  return isMac() ? '⌘Enter' : 'Ctrl+Enter';
}

/**
 * Localised message for the "navigate messages with arrow keys" chat input placeholder.
 * Returns the legacy English string if no intl instance is supplied, so call sites that
 * run before the intl provider is available still get a sensible default.
 */
export function getNavigationShortcutText(intl?: IntlShape): string {
  const prefix = isMac() ? '⌘' : 'Ctrl+';
  if (intl) {
    return intl.formatMessage(
      {
        id: 'chatInput.navigationShortcut',
        defaultMessage: '{prefix}↑/{prefix}↓ to navigate messages',
      },
      { prefix }
    );
  }
  return `${prefix}↑/${prefix}↓ to navigate messages`;
}

export function getSearchShortcutText(): string {
  return isMac() ? '⌘F' : 'Ctrl+F';
}

export type ShortcutAcceleratorProblem = 'missingModifier' | 'reserved';

// Accelerators the OS or standard Edit/Window menus already own; rebinding them
// would shadow Quit, Close, Hide, Minimize, or clipboard/undo editing.
const RESERVED_ACCELERATORS = new Set([
  'CommandOrControl+Q',
  'CommandOrControl+W',
  'CommandOrControl+H',
  'CommandOrControl+M',
  'CommandOrControl+A',
  'CommandOrControl+C',
  'CommandOrControl+V',
  'CommandOrControl+X',
  'CommandOrControl+Z',
  'CommandOrControl+Shift+Z',
]);

export function getShortcutAcceleratorProblem(
  accelerator: string
): ShortcutAcceleratorProblem | null {
  const parts = accelerator.split('+');
  const key = parts[parts.length - 1];
  const hasPrimaryModifier = parts.includes('CommandOrControl') || parts.includes('Alt');
  if (!hasPrimaryModifier && !/^F\d{1,2}$/.test(key)) {
    return 'missingModifier';
  }
  if (RESERVED_ACCELERATORS.has(accelerator)) {
    return 'reserved';
  }
  return null;
}
