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

/// Enter writes a newline, so a quick run of them is the second way to send.
export const CHAT_SUBMIT_ENTER_RUN = 3;
export const CHAT_SUBMIT_ENTER_WINDOW_MS = 1000;

/// `startedAt` anchors the window to the first Enter of the run, so "three within
/// a second" means exactly that rather than three gaps of up to a second each.
export interface EnterRun {
  count: number;
  startedAt: number;
}

export const NO_ENTER_RUN: EnterRun = { count: 0, startedAt: 0 };

export function isBareEnter(event: ChatSubmitKeyEvent): boolean {
  return (
    event.key === 'Enter' && !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey
  );
}

export function nextEnterRun(previous: EnterRun, now: number): EnterRun {
  const continues = previous.count > 0 && now - previous.startedAt <= CHAT_SUBMIT_ENTER_WINDOW_MS;
  return continues
    ? { count: previous.count + 1, startedAt: previous.startedAt }
    : { count: 1, startedAt: now };
}

export function isEnterRunSubmit(run: EnterRun): boolean {
  return run.count >= CHAT_SUBMIT_ENTER_RUN;
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
