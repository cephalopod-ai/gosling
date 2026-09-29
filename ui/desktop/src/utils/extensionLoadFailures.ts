import { useSyncExternalStore } from 'react';
import type { ExtensionLoadResult } from '../types/extensions';

/**
 * Extensions that failed to start in a chat, keyed by session and then by
 * `extensionFailureKey(name)`, with the backend's cause. Load results reach the
 * renderer only in the session create/load response, so without this record a
 * failed extension showed as an ordinary one once its toast was gone.
 */
export type ExtensionLoadFailures = ReadonlyMap<string, string>;

const MAX_TRACKED_SESSIONS = 20;
const EMPTY: ExtensionLoadFailures = new Map();

const failuresBySession = new Map<string, ExtensionLoadFailures>();
let currentSessionId: string | null = null;
const listeners = new Set<() => void>();

export function extensionFailureKey(name: string): string {
  return name.replace(/\s/g, '').toLowerCase();
}

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function setSessionFailures(sessionId: string, failures: ExtensionLoadFailures): void {
  failuresBySession.delete(sessionId);
  if (failures.size > 0) {
    failuresBySession.set(sessionId, failures);
    while (failuresBySession.size > MAX_TRACKED_SESSIONS) {
      const oldest = failuresBySession.keys().next().value;
      if (oldest === undefined) break;
      failuresBySession.delete(oldest);
    }
  }
  emit();
}

/** Replaces a chat's failures with the ones in its latest create/load response. */
export function recordExtensionLoadResults(
  sessionId: string,
  results: ExtensionLoadResult[] | null | undefined
): void {
  if (!results) {
    return;
  }
  const failures = new Map<string, string>();
  for (const result of results) {
    if (!result.success) {
      failures.set(extensionFailureKey(result.name), result.error?.trim() || 'Unknown error');
    }
  }
  setSessionFailures(sessionId, failures);
}

export function recordExtensionFailure(sessionId: string, name: string, cause: string): void {
  const failures = new Map(failuresBySession.get(sessionId) ?? EMPTY);
  failures.set(extensionFailureKey(name), cause.trim() || 'Unknown error');
  setSessionFailures(sessionId, failures);
}

export function clearExtensionFailure(sessionId: string, name: string): void {
  const existing = failuresBySession.get(sessionId);
  const key = extensionFailureKey(name);
  if (!existing?.has(key)) {
    return;
  }
  const failures = new Map(existing);
  failures.delete(key);
  setSessionFailures(sessionId, failures);
}

/** The chat most recently opened in this window; the Extensions page reports its failures. */
export function setCurrentExtensionSession(sessionId: string): void {
  if (currentSessionId === sessionId) {
    return;
  }
  currentSessionId = sessionId;
  emit();
}

export function getExtensionLoadFailures(sessionId: string | null): ExtensionLoadFailures {
  return (sessionId && failuresBySession.get(sessionId)) || EMPTY;
}

export function getCurrentExtensionSessionId(): string | null {
  return currentSessionId;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useExtensionLoadFailures(sessionId: string | null): ExtensionLoadFailures {
  return useSyncExternalStore(
    subscribe,
    () => getExtensionLoadFailures(sessionId),
    () => EMPTY
  );
}

export function useCurrentSessionExtensionLoadFailures(): ExtensionLoadFailures {
  const sessionId = useSyncExternalStore(subscribe, getCurrentExtensionSessionId, () => null);
  return useExtensionLoadFailures(sessionId);
}
