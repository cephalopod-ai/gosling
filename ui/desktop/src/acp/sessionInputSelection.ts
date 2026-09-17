import { useSyncExternalStore } from 'react';

export const MAX_SESSION_INPUT_SELECTION = 128;

const selections = new Map<string, string[]>();
const listeners = new Set<() => void>();
const EMPTY_SELECTION: string[] = [];

export function getSelectedSessionInputs(sessionId: string): string[] {
  return selections.get(sessionId) ?? EMPTY_SELECTION;
}

export function setSessionInputSelected(sessionId: string, itemId: string, selected: boolean) {
  const current = getSelectedSessionInputs(sessionId);
  if (selected && (current.includes(itemId) || current.length >= MAX_SESSION_INPUT_SELECTION)) {
    return;
  }
  const next = selected ? [...current, itemId] : current.filter((id) => id !== itemId);
  if (next.length > 0) selections.set(sessionId, next);
  else selections.delete(sessionId);
  listeners.forEach((listener) => listener());
}

export function clearSelectedSessionInputs(sessionId: string, itemIds: string[]) {
  itemIds.forEach((id) => setSessionInputSelected(sessionId, id, false));
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSelectedSessionInputs(sessionId: string | null): string[] {
  return useSyncExternalStore(subscribe, () =>
    sessionId ? getSelectedSessionInputs(sessionId) : EMPTY_SELECTION
  );
}
