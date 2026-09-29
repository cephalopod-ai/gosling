import type { PendingSessionRecovery, Settings } from '../utils/settings';

type UpdateSettings = (modifier: (settings: Settings) => void) => void;

const MAX_PENDING_SESSION_RECOVERIES = 10;
const MAX_SESSION_ID_LENGTH = 256;
const MAX_WORKING_DIR_LENGTH = 4096;

function recoveryKey(recovery: Pick<PendingSessionRecovery, 'backendId' | 'sessionId'>): string {
  return JSON.stringify([recovery.backendId, recovery.sessionId]);
}

export function recoverySessionIdForBackend(
  recovery: PendingSessionRecovery,
  backendId: string
): string | undefined {
  return recovery.backendId === backendId ? recovery.sessionId : undefined;
}

export class SessionRecoveryRegistry {
  private readonly sessionKeysByWindow = new Map<number, Set<string>>();

  constructor(
    private readonly getSettings: () => Settings,
    private readonly updateSettings: UpdateSettings,
    private readonly now: () => number = Date.now
  ) {}

  setActive(
    windowId: number,
    sessionId: string,
    workingDir: string,
    active: boolean,
    backendId: string
  ): boolean {
    const normalizedSessionId = sessionId.trim();
    if (
      !Number.isInteger(windowId) ||
      windowId <= 0 ||
      !normalizedSessionId ||
      normalizedSessionId.length > MAX_SESSION_ID_LENGTH ||
      !backendId
    ) {
      return false;
    }

    const key = recoveryKey({ backendId, sessionId: normalizedSessionId });
    if (active) {
      if (!workingDir.trim() || workingDir.length > MAX_WORKING_DIR_LENGTH) {
        return false;
      }
      const sessionKeys = this.sessionKeysByWindow.get(windowId) ?? new Set<string>();
      sessionKeys.add(key);
      this.sessionKeysByWindow.set(windowId, sessionKeys);
      const recovery: PendingSessionRecovery = {
        backendId,
        sessionId: normalizedSessionId,
        workingDir,
        startedAt: this.now(),
      };
      this.updateSettings((settings) => {
        settings.pendingSessionRecoveries = [
          ...settings.pendingSessionRecoveries.filter(
            (candidate) => recoveryKey(candidate) !== key
          ),
          recovery,
        ].slice(-MAX_PENDING_SESSION_RECOVERIES);
      });
      return true;
    }

    this.detach(windowId, key);
    if (!this.hasAttachedSession(key)) {
      this.clearPersistedSession(key);
    }
    return true;
  }

  attachPending(windowId: number, recovery: PendingSessionRecovery): void {
    if (!recovery.backendId) return;
    const sessionKeys = this.sessionKeysByWindow.get(windowId) ?? new Set<string>();
    sessionKeys.add(recoveryKey(recovery));
    this.sessionKeysByWindow.set(windowId, sessionKeys);
  }

  clearWindow(windowId: number): void {
    const sessionKeys = this.sessionKeysByWindow.get(windowId);
    if (!sessionKeys) return;

    this.sessionKeysByWindow.delete(windowId);
    for (const key of sessionKeys) {
      if (!this.hasAttachedSession(key)) {
        this.clearPersistedSession(key);
      }
    }
  }

  clearAll(): void {
    const attached = new Set([...this.sessionKeysByWindow.values()].flatMap((keys) => [...keys]));
    this.sessionKeysByWindow.clear();
    if (attached.size === 0) return;
    this.updateSettings((settings) => {
      settings.pendingSessionRecoveries = settings.pendingSessionRecoveries.filter(
        (recovery) => !attached.has(recoveryKey(recovery))
      );
    });
  }

  pendingForBackend(
    backendIdForDirectory: (workingDir: string) => string
  ): PendingSessionRecovery[] {
    // Never replay a legacy/unowned marker, or another backend's session with the same ID.
    return this.getSettings().pendingSessionRecoveries.filter(
      (recovery) =>
        recovery.backendId &&
        recoverySessionIdForBackend(recovery, backendIdForDirectory(recovery.workingDir)) !==
          undefined
    );
  }

  private detach(windowId: number, key: string): void {
    const sessionKeys = this.sessionKeysByWindow.get(windowId);
    if (!sessionKeys) return;
    sessionKeys.delete(key);
    if (sessionKeys.size === 0) {
      this.sessionKeysByWindow.delete(windowId);
    }
  }

  private hasAttachedSession(key: string): boolean {
    return [...this.sessionKeysByWindow.values()].some((keys) => keys.has(key));
  }

  private clearPersistedSession(key: string): void {
    if (
      !this.getSettings().pendingSessionRecoveries.some((recovery) => recoveryKey(recovery) === key)
    ) {
      return;
    }
    this.updateSettings((settings) => {
      settings.pendingSessionRecoveries = settings.pendingSessionRecoveries.filter(
        (recovery) => recoveryKey(recovery) !== key
      );
    });
  }
}
