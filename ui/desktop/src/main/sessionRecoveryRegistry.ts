import type { PendingSessionRecovery, Settings } from '../utils/settings';

type UpdateSettings = (modifier: (settings: Settings) => void) => void;

const MAX_PENDING_SESSION_RECOVERIES = 10;
const MAX_SESSION_ID_LENGTH = 256;
const MAX_WORKING_DIR_LENGTH = 4096;

export class SessionRecoveryRegistry {
  private readonly sessionIdsByWindow = new Map<number, Set<string>>();

  constructor(
    private readonly getSettings: () => Settings,
    private readonly updateSettings: UpdateSettings,
    private readonly now: () => number = Date.now
  ) {}

  setActive(windowId: number, sessionId: string, workingDir: string, active: boolean): boolean {
    const normalizedSessionId = sessionId.trim();
    if (
      !Number.isInteger(windowId) ||
      windowId <= 0 ||
      !normalizedSessionId ||
      normalizedSessionId.length > MAX_SESSION_ID_LENGTH
    ) {
      return false;
    }

    if (active) {
      if (!workingDir.trim() || workingDir.length > MAX_WORKING_DIR_LENGTH) {
        return false;
      }
      const sessionIds = this.sessionIdsByWindow.get(windowId) ?? new Set<string>();
      sessionIds.add(normalizedSessionId);
      this.sessionIdsByWindow.set(windowId, sessionIds);
      const recovery: PendingSessionRecovery = {
        sessionId: normalizedSessionId,
        workingDir,
        startedAt: this.now(),
      };
      this.updateSettings((settings) => {
        settings.pendingSessionRecoveries = [
          ...settings.pendingSessionRecoveries.filter(
            (candidate) => candidate.sessionId !== normalizedSessionId
          ),
          recovery,
        ].slice(-MAX_PENDING_SESSION_RECOVERIES);
      });
      return true;
    }

    this.detach(windowId, normalizedSessionId);
    if (!this.hasAttachedSession(normalizedSessionId)) {
      this.clearPersistedSession(normalizedSessionId);
    }
    return true;
  }

  attachPending(windowId: number, recovery: PendingSessionRecovery): void {
    const sessionIds = this.sessionIdsByWindow.get(windowId) ?? new Set<string>();
    sessionIds.add(recovery.sessionId);
    this.sessionIdsByWindow.set(windowId, sessionIds);
  }

  clearWindow(windowId: number): void {
    const sessionIds = this.sessionIdsByWindow.get(windowId);
    if (!sessionIds) return;

    this.sessionIdsByWindow.delete(windowId);
    for (const sessionId of sessionIds) {
      if (!this.hasAttachedSession(sessionId)) {
        this.clearPersistedSession(sessionId);
      }
    }
  }

  clearAll(): void {
    this.sessionIdsByWindow.clear();
    if (this.getSettings().pendingSessionRecoveries.length === 0) return;
    this.updateSettings((settings) => {
      settings.pendingSessionRecoveries = [];
    });
  }

  private detach(windowId: number, sessionId: string): void {
    const sessionIds = this.sessionIdsByWindow.get(windowId);
    if (!sessionIds) return;
    sessionIds.delete(sessionId);
    if (sessionIds.size === 0) {
      this.sessionIdsByWindow.delete(windowId);
    }
  }

  private hasAttachedSession(sessionId: string): boolean {
    return [...this.sessionIdsByWindow.values()].some((sessionIds) => sessionIds.has(sessionId));
  }

  private clearPersistedSession(sessionId: string): void {
    if (
      !this.getSettings().pendingSessionRecoveries.some(
        (recovery) => recovery.sessionId === sessionId
      )
    ) {
      return;
    }
    this.updateSettings((settings) => {
      settings.pendingSessionRecoveries = settings.pendingSessionRecoveries.filter(
        (recovery) => recovery.sessionId !== sessionId
      );
    });
  }
}
