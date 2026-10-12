import type { GoslingServeExitSignal, GoslingServeResult, Logger } from './goslingServe';

export const GOSLING_SERVE_EXITED_USER_MESSAGE =
  "This window's Gosling backend stopped. Close this window and open a new chat to start a new backend. If this keeps happening, restart Gosling Desktop.";

export interface GoslingServeLease {
  acpUrl: string;
  secretKey: string;
  cleanup: () => Promise<void>;
  windowIds: Set<number>;
  cleanedUp: boolean;
  exited: boolean;
  exitCode: number | null;
  exitSignal: GoslingServeExitSignal;
}

export class GoslingServeLeaseRegistry {
  private leases = new Set<GoslingServeLease>();
  private leasesByWindowId = new Map<number, GoslingServeLease>();
  private cleanupPromises = new Map<GoslingServeLease, Promise<void>>();

  constructor(private readonly logger: Logger) {}

  create(result: GoslingServeResult, secretKey: string): GoslingServeLease {
    const lease: GoslingServeLease = {
      acpUrl: result.acpUrl,
      secretKey,
      cleanup: result.cleanup,
      windowIds: new Set<number>(),
      cleanedUp: false,
      exited: false,
      exitCode: null,
      exitSignal: null,
    };

    const markExited = ({
      code,
      signal,
      logUnexpected,
    }: {
      code?: number | null;
      signal?: GoslingServeExitSignal;
      logUnexpected: boolean;
    }) => {
      const firstExit = !lease.exited;
      lease.exited = true;
      if (code !== undefined) {
        lease.exitCode = code;
      }
      if (signal !== undefined) {
        lease.exitSignal = signal;
      }

      if (logUnexpected && firstExit && !lease.cleanedUp) {
        this.logger.error('Gosling ACP server exited unexpectedly', {
          code: lease.exitCode,
          signal: lease.exitSignal,
          windowIds: [...lease.windowIds],
        });
      }
    };

    result.process.once('exit', (code, signal) => {
      markExited({ code, signal, logUnexpected: true });
    });

    if (result.hasExited()) {
      const exitDetails = result.getExitDetails();
      markExited({ code: exitDetails.code, signal: exitDetails.signal, logUnexpected: false });
    }

    this.leases.add(lease);
    return lease;
  }

  createExternal(
    acpUrl: string,
    secretKey: string,
    cleanup: () => Promise<void> = async () => undefined
  ): GoslingServeLease {
    const lease = {
      acpUrl,
      secretKey,
      cleanup,
      windowIds: new Set<number>(),
      cleanedUp: false,
      exited: false,
      exitCode: null,
      exitSignal: null,
    };
    this.leases.add(lease);
    return lease;
  }

  get(windowId: number): GoslingServeLease | null {
    return this.leasesByWindowId.get(windowId) ?? null;
  }

  getAcpUrl(windowId: number): string | null {
    const lease = this.get(windowId);
    if (!lease) {
      return null;
    }
    if (lease.exited) {
      throw new Error(GOSLING_SERVE_EXITED_USER_MESSAGE);
    }
    return lease.acpUrl;
  }

  getSecretKey(windowId: number): string | null {
    const lease = this.get(windowId);
    if (!lease) {
      return null;
    }
    if (lease.exited) {
      throw new Error(GOSLING_SERVE_EXITED_USER_MESSAGE);
    }
    return lease.secretKey;
  }

  attachWindow(windowId: number, lease: GoslingServeLease) {
    lease.windowIds.add(windowId);
    this.leasesByWindowId.set(windowId, lease);
  }

  async releaseWindow(windowId: number) {
    const lease = this.leasesByWindowId.get(windowId);
    this.leasesByWindowId.delete(windowId);

    if (!lease) {
      return;
    }

    lease.windowIds.delete(windowId);
    if (lease.windowIds.size === 0) {
      await this.cleanupLease(lease);
    }
  }

  async cleanupLease(lease: GoslingServeLease) {
    const pending = this.cleanupPromises.get(lease);
    if (pending) return pending;
    if (lease.cleanedUp) {
      return;
    }

    lease.cleanedUp = true;
    this.leases.delete(lease);
    for (const windowId of lease.windowIds) {
      this.leasesByWindowId.delete(windowId);
    }
    lease.windowIds.clear();

    const cleanup = (async () => {
      try {
        await lease.cleanup();
      } catch (error) {
        this.logger.error('Failed to cleanup gosling serve backend:', error);
      }
    })().finally(() => this.cleanupPromises.delete(lease));
    this.cleanupPromises.set(lease, cleanup);
    await cleanup;
  }

  activeLeaseCount(): number {
    return this.leases.size;
  }

  async cleanupAll() {
    const pending = [...this.cleanupPromises.values()];
    await Promise.all([
      ...pending,
      ...this.uniqueLeases().map((lease) => this.cleanupLease(lease)),
    ]);
  }

  private uniqueLeases(): GoslingServeLease[] {
    return [...this.leases];
  }
}
