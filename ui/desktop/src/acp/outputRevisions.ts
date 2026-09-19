import { getAcpClient } from './acpConnection';
import type { OutputRevisionDto } from '@repo-makeover/gosling-sdk';

export async function getOutputHistory(sessionId: string, path: string, beforeVersion?: number) {
  const client = await getAcpClient();
  return client.gosling.sessionOutputsHistory_unstable({
    sessionId,
    path,
    beforeVersion,
    limit: 50,
  });
}

// A file list renders one row per output, and each row asks for its own
// latest revision on mount and again on every window focus. Coalescing those
// same-tick calls into one sessionOutputsLatestBatch_unstable request turns
// what used to be one round trip per visible row into one round trip per
// list. Callers still get a plain per-path promise from
// getLatestOutputRevision; batching is an implementation detail hidden
// behind it.
type LatestRevisionWaiter = {
  resolve: (revision: OutputRevisionDto | null) => void;
  reject: (reason: unknown) => void;
};

type PendingBatch = {
  sessionId: string;
  waitersByPath: Map<string, LatestRevisionWaiter[]>;
};

const pendingBatchesBySession = new Map<string, PendingBatch>();
let activeBatches = 0;
const waitingBatches: Array<() => void> = [];

async function flushBatch(batch: PendingBatch): Promise<void> {
  if (activeBatches >= 4) await new Promise<void>((resolve) => waitingBatches.push(resolve));
  activeBatches += 1;
  try {
    const client = await getAcpClient();
    const response = await client.gosling.sessionOutputsLatestBatch_unstable({
      sessionId: batch.sessionId,
      paths: [...batch.waitersByPath.keys()],
    });
    const revisionByPath = new Map(response.revisions.map((entry) => [entry.path, entry.revision]));
    for (const [path, waiters] of batch.waitersByPath) {
      if (revisionByPath.has(path)) {
        const revision = revisionByPath.get(path) ?? null;
        for (const waiter of waiters) waiter.resolve(revision);
      } else {
        // Absent from the response means the path failed authorization on
        // the backend (e.g. no longer a registered output) — the same
        // "unavailable" outcome a lone failed request would have produced.
        const error = new Error('History unavailable');
        for (const waiter of waiters) waiter.reject(error);
      }
    }
  } catch (error) {
    for (const waiters of batch.waitersByPath.values()) {
      for (const waiter of waiters) waiter.reject(error);
    }
  } finally {
    activeBatches -= 1;
    waitingBatches.shift()?.();
  }
}

export function getLatestOutputRevision(
  sessionId: string,
  path: string,
  signal: globalThis.AbortSignal
): Promise<OutputRevisionDto | null> {
  return new Promise<OutputRevisionDto | null>((resolve, reject) => {
    signal.throwIfAborted();
    let batch = pendingBatchesBySession.get(sessionId);
    if (!batch) {
      batch = { sessionId, waitersByPath: new Map() };
      pendingBatchesBySession.set(sessionId, batch);
      // Zero-delay macrotask: every OutputHistory row mounted (or refocused)
      // in the same commit/event registers its waiter synchronously before
      // this fires, so they all land in one batch.
      setTimeout(() => {
        pendingBatchesBySession.delete(sessionId);
        void flushBatch(batch!);
      }, 0);
    }
    const waiters = batch.waitersByPath.get(path);
    if (waiters) waiters.push({ resolve, reject });
    else batch.waitersByPath.set(path, [{ resolve, reject }]);
  });
}

export async function getOutputRevision(sessionId: string, path: string, version: number) {
  const client = await getAcpClient();
  return client.gosling.sessionOutputsRevision_unstable({ sessionId, path, version });
}

export async function restoreOutputRevision(
  sessionId: string,
  path: string,
  version: number,
  expectedCurrentHash: string
) {
  const client = await getAcpClient();
  return client.gosling.sessionOutputsRestore_unstable({
    sessionId,
    path,
    version,
    expectedCurrentHash,
  });
}
