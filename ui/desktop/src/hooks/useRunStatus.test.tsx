import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acpChatSessionActions,
  acpChatSessionStore,
  type AcpChatSessionSnapshot,
} from '../acp/chatSessionStore';
import {
  checkSessionRunStatus,
  RUN_STATUS_INTERVAL_MS,
  type SessionRunStatus,
} from '../acp/runStatus';
import { ChatState } from '../types/chatState';
import { useRunStatus } from './useRunStatus';

vi.mock('../acp/runStatus', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../acp/runStatus')>()),
  checkSessionRunStatus: vi.fn(),
}));

const sessionId = 'heartbeat-test';
function snapshot(): AcpChatSessionSnapshot {
  acpChatSessionActions.startPromptAttempt(sessionId, 'prompt-1');
  return acpChatSessionStore.getSnapshot(sessionId)!;
}
function checked(tasks: SessionRunStatus['tasks'] = []): SessionRunStatus {
  return { checkedAt: Date.now(), backendResponded: true, error: null, tasks };
}
async function flush() {
  await act(async () => {
    await Promise.resolve();
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date('2026-09-16T12:00:00Z'));
  vi.mocked(checkSessionRunStatus).mockImplementation(async () => checked());
});
afterEach(() => {
  vi.useRealTimers();
  vi.resetAllMocks();
  acpChatSessionActions.deleteSnapshot(sessionId);
});

describe('useRunStatus', () => {
  it('checks immediately, warns after 15 minutes without output, and clears the warning on fresh output', async () => {
    const current = snapshot();
    const { result, rerender } = renderHook((state) => useRunStatus(sessionId, state), {
      initialProps: current,
    });
    await flush();
    expect(checkSessionRunStatus).toHaveBeenCalledOnce();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(RUN_STATUS_INTERVAL_MS - 1);
    });
    expect(checkSessionRunStatus).toHaveBeenCalledOnce();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(checkSessionRunStatus).toHaveBeenCalledTimes(2);
    expect(result.current.quiet).toBe(true);
    rerender({ ...current, lastActivityAt: Date.now() });
    expect(result.current.quiet).toBe(false);
  });

  it('does not mistake a user approval pause for stalled work', async () => {
    const { result } = renderHook(() =>
      useRunStatus(sessionId, { ...snapshot(), chatState: ChatState.WaitingForUserInput })
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(RUN_STATUS_INTERVAL_MS);
    });
    expect(result.current.waitingForUser).toBe(true);
    expect(result.current.quiet).toBe(false);
  });

  it('monitors background agents after the foreground reply ends and stops polling on completion', async () => {
    const current = {
      ...snapshot(),
      activePromptAttemptId: null,
      chatState: ChatState.Idle,
      backgroundTasks: [
        { id: '20260916_1', description: 'Check sources', state: 'running' as const },
      ],
    };
    vi.mocked(checkSessionRunStatus).mockImplementation(async () =>
      checked([
        {
          ...current.backgroundTasks[0],
          state: 'completed',
          turns: 5,
          idleMs: null,
          checkedAt: Date.now(),
          error: null,
        },
      ])
    );
    const { result, unmount } = renderHook(() => useRunStatus(sessionId, current));
    await flush();
    expect(result.current.visible).toBe(true);
    expect(result.current.tasks[0]).toMatchObject({ state: 'completed', turns: 5 });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(RUN_STATUS_INTERVAL_MS * 2);
    });
    expect(checkSessionRunStatus).toHaveBeenCalledOnce();
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('shows an unavailable status when a check fails and recovers on Check now', async () => {
    vi.mocked(checkSessionRunStatus).mockRejectedValueOnce(new Error('Status check timed out'));
    const current = snapshot();
    const { result } = renderHook(() => useRunStatus(sessionId, current));
    await flush();
    expect(result.current.unavailable).toBe(true);
    await act(async () => {
      await result.current.checkNow();
    });
    expect(result.current.unavailable).toBe(false);
  });

  it('does not apply a late check to a different session', async () => {
    let resolve!: (value: SessionRunStatus) => void;
    vi.mocked(checkSessionRunStatus).mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      })
    );
    const current = snapshot();
    const { result, rerender, unmount } = renderHook(({ id, state }) => useRunStatus(id, state), {
      initialProps: { id: sessionId, state: current },
    });
    rerender({ id: 'other-session', state: { ...current, activePromptAttemptId: null } });
    await act(async () => {
      resolve(checked());
    });
    expect(result.current.result).toBeNull();
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('checks immediately after waking when the last heartbeat is overdue', async () => {
    const current = snapshot();
    renderHook(() => useRunStatus(sessionId, current));
    await flush();
    vi.setSystemTime(Date.now() + RUN_STATUS_INTERVAL_MS * 2);
    await act(async () => {
      window.dispatchEvent(new Event('focus'));
    });
    expect(checkSessionRunStatus).toHaveBeenCalledTimes(2);
  });

  it('queues a check for newly discovered agents without overlapping the current check', async () => {
    let resolve!: (value: SessionRunStatus) => void;
    vi.mocked(checkSessionRunStatus).mockReturnValueOnce(
      new Promise((done) => {
        resolve = done;
      })
    );
    const current = snapshot();
    const { rerender } = renderHook((state) => useRunStatus(sessionId, state), {
      initialProps: current,
    });
    const task = { id: '20260916_2', description: 'Verify a timeline', state: 'running' as const };
    rerender({ ...current, backgroundTasks: [task] });
    expect(checkSessionRunStatus).toHaveBeenCalledOnce();
    await act(async () => {
      resolve(checked());
    });
    expect(checkSessionRunStatus).toHaveBeenCalledTimes(2);
    expect(checkSessionRunStatus).toHaveBeenLastCalledWith(sessionId, [task]);
  });
});
