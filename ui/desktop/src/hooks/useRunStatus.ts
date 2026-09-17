import { useCallback, useEffect, useRef, useState } from 'react';
import type { AcpChatSessionSnapshot } from '../acp/chatSessionStore';
import { isBackgroundTaskActive } from '../acp/backgroundTasks';
import {
  checkSessionRunStatus,
  RUN_STATUS_INTERVAL_MS,
  type BackgroundTaskStatus,
  type SessionRunStatus,
} from '../acp/runStatus';
import { describeAcpError } from '../acp/errors';
import { ChatState } from '../types/chatState';

const EMPTY_TASKS: BackgroundTaskStatus[] = [];

export function useRunStatus(sessionId: string, snapshot: AcpChatSessionSnapshot | undefined) {
  const [result, setResult] = useState<SessionRunStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [now, setNow] = useState(Date.now);
  const snapshotRef = useRef(snapshot);
  const resultRef = useRef(result);
  const lifecycleRef = useRef({ active: true, sessionId });
  const checkingRef = useRef(false);
  const recheckRef = useRef(false);
  snapshotRef.current = snapshot;
  resultRef.current = result;

  const tasks = (snapshot?.backgroundTasks ?? []).map<BackgroundTaskStatus>((task) => {
    const checked = result?.tasks.find((status) => status.id === task.id);
    return {
      turns: null,
      idleMs: null,
      checkedAt: null,
      error: null,
      ...checked,
      ...task,
      state: isBackgroundTaskActive(task) ? (checked?.state ?? 'unknown') : task.state,
    };
  });
  const foregroundActive = snapshot?.activePromptAttemptId != null;
  const waitingForUser = snapshot?.chatState === ChatState.WaitingForUserInput;
  const monitoring = foregroundActive || tasks.some(isBackgroundTaskActive);

  const checkNow = useCallback(async () => {
    if (checkingRef.current) {
      recheckRef.current = true;
      return;
    }
    const lifecycle = lifecycleRef.current;
    const current = snapshotRef.current;
    const promptAttemptId = current?.activePromptAttemptId;
    const generation = current?.connectionGeneration;
    const activeTasks = (current?.backgroundTasks ?? []).filter((task) => {
      const checked = resultRef.current?.tasks.find((status) => status.id === task.id);
      return isBackgroundTaskActive(task) && (!checked || isBackgroundTaskActive(checked));
    });
    if (!promptAttemptId && activeTasks.length === 0) return;
    checkingRef.current = true;
    setChecking(true);
    try {
      const status = await checkSessionRunStatus(sessionId, activeTasks);
      if (
        lifecycle.active &&
        lifecycle === lifecycleRef.current &&
        snapshotRef.current?.connectionGeneration === generation
      ) {
        setResult((previous) => ({
          ...status,
          tasks: [
            ...(previous?.tasks ?? EMPTY_TASKS).filter(
              (task) => !status.tasks.some((update) => update.id === task.id)
            ),
            ...status.tasks,
          ],
        }));
      }
    } catch (error) {
      if (
        lifecycle.active &&
        lifecycle === lifecycleRef.current &&
        snapshotRef.current?.connectionGeneration === generation
      ) {
        setResult((previous) => ({
          checkedAt: Date.now(),
          backendResponded: false,
          error: describeAcpError(error),
          tasks: previous?.tasks ?? EMPTY_TASKS,
        }));
      }
    } finally {
      if (lifecycle.active && lifecycle === lifecycleRef.current) {
        checkingRef.current = false;
        setChecking(false);
        setNow(Date.now());
        if (recheckRef.current) {
          recheckRef.current = false;
          void checkNow();
        }
      }
    }
  }, [sessionId]);

  const taskIds = (snapshot?.backgroundTasks ?? [])
    .filter(isBackgroundTaskActive)
    .map((task) => task.id)
    .join(',');
  const promptAttemptId = snapshot?.activePromptAttemptId;

  useEffect(() => {
    const lifecycle = { active: true, sessionId };
    lifecycleRef.current = lifecycle;
    checkingRef.current = false;
    recheckRef.current = false;
    setResult(null);
    setChecking(false);
    return () => {
      lifecycle.active = false;
    };
  }, [sessionId]);

  useEffect(() => {
    if (!monitoring) return;
    void checkNow();
    const interval = setInterval(() => void checkNow(), RUN_STATUS_INTERVAL_MS);
    const onWake = () => {
      if (document.visibilityState !== 'visible') return;
      const checkedAt = resultRef.current?.checkedAt;
      if (checkedAt == null || Date.now() - checkedAt >= RUN_STATUS_INTERVAL_MS) void checkNow();
      setNow(Date.now());
    };
    document.addEventListener('visibilitychange', onWake);
    window.addEventListener('focus', onWake);
    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', onWake);
      window.removeEventListener('focus', onWake);
    };
  }, [monitoring, sessionId, checkNow, taskIds, promptAttemptId]);

  useEffect(() => {
    if (!monitoring) return;
    const interval = setInterval(() => setNow(Date.now()), 10_000);
    return () => clearInterval(interval);
  }, [monitoring]);

  const lastActivityAt = snapshot?.lastActivityAt ?? null;
  const quiet =
    (foregroundActive &&
      !waitingForUser &&
      snapshot?.promptStartedAt != null &&
      now - (lastActivityAt ?? snapshot.promptStartedAt) >= RUN_STATUS_INTERVAL_MS) ||
    tasks.some((task) => task.state === 'quiet');
  const stale = result !== null && now - result.checkedAt >= RUN_STATUS_INTERVAL_MS;
  const unavailable =
    stale ||
    result?.backendResponded === false ||
    (result !== null && tasks.some((task) => task.state === 'unknown'));

  return {
    visible: foregroundActive || tasks.length > 0,
    foregroundActive,
    waitingForUser,
    tasks,
    result,
    checking,
    quiet,
    unavailable,
    now,
    lastActivityAt,
    checkNow,
  };
}

export type RunStatus = ReturnType<typeof useRunStatus>;
