import type { GoslingToolCallResponse_unstable } from '@repo-makeover/gosling-sdk';
import { getAcpClient } from './acpConnection';
import { isRecord } from './adapter/shared';
import { describeAcpError } from './errors';
import {
  toolResultText,
  type BackgroundTaskReference,
  type BackgroundTaskState,
} from './backgroundTasks';

export const RUN_STATUS_INTERVAL_MS = 15 * 60 * 1000;
export const RUN_STATUS_TIMEOUT_MS = 10 * 1000;

export interface BackgroundTaskStatus extends BackgroundTaskReference {
  turns: number | null;
  idleMs: number | null;
  checkedAt: number | null;
  error: string | null;
}

export interface SessionRunStatus {
  checkedAt: number;
  backendResponded: boolean;
  error: string | null;
  tasks: BackgroundTaskStatus[];
}

export function parseBackgroundTaskStatus(
  task: BackgroundTaskReference,
  response: GoslingToolCallResponse_unstable,
  checkedAt: number
): BackgroundTaskStatus {
  const text = toolResultText(response);
  const meta = isRecord(response._meta) ? response._meta : {};
  const status = meta.task_status;
  const idle = text.match(/^\*\*Idle:\*\* (\d+)([sm])$/m);
  const idleMs = idle ? Number(idle[1]) * (idle[2] === 'm' ? 60_000 : 1000) : null;
  let state: BackgroundTaskState = 'unknown';
  if (!response.isError) {
    if (status === 'running' && idleMs !== null) {
      state = idleMs >= RUN_STATUS_INTERVAL_MS ? 'quiet' : 'running';
    } else if (status === 'completed' || status === 'cancelled') {
      state = status;
    } else if (status === 'failed' || status === 'panicked') {
      state = 'failed';
    }
  }
  return {
    ...task,
    state,
    turns: typeof meta.turns_taken === 'number' ? meta.turns_taken : null,
    idleMs,
    checkedAt,
    error: state === 'unknown' ? text || 'No task status returned' : null,
  };
}

const pendingChecks = new Map<string, Promise<SessionRunStatus>>();

async function readSessionRunStatus(
  sessionId: string,
  tasks: BackgroundTaskReference[]
): Promise<SessionRunStatus> {
  const client = await getAcpClient();
  const results = await Promise.allSettled([
    client.gosling.sessionInfo_unstable({ sessionId }),
    ...tasks.map((task) =>
      client.gosling.toolsCall_unstable({
        sessionId,
        name: 'load',
        arguments: { source: task.id, peek: true },
      })
    ),
  ]);
  const checkedAt = Date.now();
  const backend = results[0];
  return {
    checkedAt,
    backendResponded: backend.status === 'fulfilled',
    error: backend.status === 'rejected' ? describeAcpError(backend.reason) : null,
    tasks: tasks.map((task, index) => {
      const result = results[index + 1];
      if (result.status === 'fulfilled') {
        return parseBackgroundTaskStatus(
          task,
          result.value as GoslingToolCallResponse_unstable,
          checkedAt
        );
      }
      return {
        ...task,
        state: 'unknown',
        turns: null,
        idleMs: null,
        checkedAt,
        error: describeAcpError(result.reason),
      };
    }),
  };
}

export async function checkSessionRunStatus(
  sessionId: string,
  tasks: BackgroundTaskReference[]
): Promise<SessionRunStatus> {
  let pending = pendingChecks.get(sessionId);
  if (!pending) {
    pending = readSessionRunStatus(sessionId, tasks);
    pendingChecks.set(sessionId, pending);
    const clearPending = () => {
      if (pendingChecks.get(sessionId) === pending) pendingChecks.delete(sessionId);
    };
    void pending.then(clearPending, clearPending);
  }
  let timeout: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      pending,
      new Promise<SessionRunStatus>((_, reject) => {
        timeout = setTimeout(
          () => reject(new Error('Status check timed out')),
          RUN_STATUS_TIMEOUT_MS
        );
      }),
    ]);
  } finally {
    clearTimeout(timeout);
  }
}
