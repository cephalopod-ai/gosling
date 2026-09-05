import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ChatState } from '../../types/chatState';
import { createUserMessage } from '../../types/message';
import { acpChatSessionController } from '../chatSessionController';
import { acpChatSessionActions, acpChatSessionStore } from '../chatSessionStore';
import {
  cancelAcpPermissionRequestsForSession,
  isAcpPermissionRequestPending,
  requestAcpPermission,
  resolveAcpPermissionRequest,
} from '../permissionRequests';
import { acpCancelPrompt, acpPromptSession } from '../prompt';
import { acpListSessionArtifacts, acpLoadSession } from '../sessions';

vi.mock('../../utils/extensionErrorUtils', () => ({ showExtensionLoadResults: vi.fn() }));
vi.mock('../acpConnection', () => ({ getAcpConnectionGeneration: () => 1 }));
vi.mock('../prompt', () => ({ acpCancelPrompt: vi.fn(), acpPromptSession: vi.fn() }));
vi.mock('../sessions', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../sessions')>()),
  acpLoadSession: vi.fn(),
  acpListSessionArtifacts: vi.fn(),
  isAcpSessionLoadInFlight: () => false,
}));

const SESSION_ID = 'lifecycle-session';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function submit(onFinish = vi.fn()) {
  return acpChatSessionController.submitMessage(SESSION_ID, createUserMessage('Hello'), {
    getCurrentSnapshot: () => acpChatSessionStore.getSnapshot(SESSION_ID),
    onFinish,
  });
}

describe('ACP chat session lifecycle', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    acpChatSessionActions.deleteSnapshot(SESSION_ID);
    vi.mocked(window.electron.setWakelockActive).mockResolvedValue(true);
    vi.mocked(acpCancelPrompt).mockResolvedValue(undefined);
    vi.mocked(acpPromptSession).mockResolvedValue({ stopReason: 'end_turn' });
    vi.mocked(acpLoadSession).mockResolvedValue({
      sessionInfo: { sessionId: SESSION_ID, cwd: '/tmp', title: 'Test session' },
      response: {},
      meta: {},
    });
    vi.mocked(acpListSessionArtifacts).mockResolvedValue([]);
  });

  afterEach(() => {
    cancelAcpPermissionRequestsForSession(SESSION_ID);
    acpChatSessionActions.deleteSnapshot(SESSION_ID);
  });

  it('does not send a stopped prompt after waiting for the wakelock', async () => {
    const wakelock = deferred<boolean>();
    vi.mocked(window.electron.setWakelockActive).mockReturnValueOnce(wakelock.promise);
    const onFinish = vi.fn();
    const submission = submit(onFinish);

    acpChatSessionController.stop(SESSION_ID);
    wakelock.resolve(true);
    await submission;

    expect(acpPromptSession).not.toHaveBeenCalled();
    expect(acpCancelPrompt).not.toHaveBeenCalled();
    expect(onFinish).not.toHaveBeenCalled();
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)).toMatchObject({
      activePromptAttemptId: null,
      pendingCancelPromptAttemptId: null,
      chatState: ChatState.Idle,
    });
    expect(window.electron.setWakelockActive).toHaveBeenLastCalledWith(SESSION_ID, false);
  });

  it('restores a running prompt and its unanswered permission when cancellation fails', async () => {
    const response = deferred<Awaited<ReturnType<typeof acpPromptSession>>>();
    vi.mocked(acpPromptSession).mockReturnValueOnce(response.promise);
    const submission = submit();
    await vi.waitFor(() => expect(acpPromptSession).toHaveBeenCalledOnce());
    const attemptId = acpChatSessionStore.getSnapshot(SESSION_ID)?.activePromptAttemptId;
    const permission = requestAcpPermission({
      sessionId: SESSION_ID,
      toolCall: { toolCallId: 'tool-1', title: 'Read file' },
      options: [{ optionId: 'allow-once', name: 'Allow once', kind: 'allow_once' }],
    });
    vi.mocked(acpCancelPrompt).mockRejectedValueOnce(new Error('Cancel could not be sent'));

    acpChatSessionController.stop(SESSION_ID);
    await vi.waitFor(() => {
      expect(acpChatSessionStore.getSnapshot(SESSION_ID)).toMatchObject({
        activePromptAttemptId: attemptId,
        pendingCancelPromptAttemptId: null,
        chatState: ChatState.WaitingForUserInput,
      });
    });

    expect(isAcpPermissionRequestPending(SESSION_ID, 'tool-1')).toBe(true);
    expect(resolveAcpPermissionRequest(SESSION_ID, 'tool-1', 'allow_once')).toBe(true);
    await expect(permission).resolves.toEqual({
      outcome: { outcome: 'selected', optionId: 'allow-once' },
    });
    response.resolve({ stopReason: 'end_turn' });
    await submission;
  });

  it('joins a session load while its artifact inventory is still loading', async () => {
    const artifacts = deferred<Awaited<ReturnType<typeof acpListSessionArtifacts>>>();
    vi.mocked(acpListSessionArtifacts).mockReturnValue(artifacts.promise);
    const firstOnLoaded = vi.fn();
    const secondOnLoaded = vi.fn();
    const firstLoad = acpChatSessionController.loadSession(SESSION_ID, {
      onSessionLoaded: firstOnLoaded,
    });
    await vi.waitFor(() => expect(acpListSessionArtifacts).toHaveBeenCalledOnce());
    const replayedMessage = createUserMessage('Persisted history');
    acpChatSessionActions.setMessages(SESSION_ID, [replayedMessage]);

    const secondLoad = acpChatSessionController.loadSession(SESSION_ID, {
      onSessionLoaded: secondOnLoaded,
    });
    artifacts.resolve([]);
    await expect(Promise.all([firstLoad, secondLoad])).resolves.toEqual([true, true]);

    expect(acpLoadSession).toHaveBeenCalledOnce();
    expect(acpListSessionArtifacts).toHaveBeenCalledOnce();
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.messages).toEqual([replayedMessage]);
    expect(firstOnLoaded).toHaveBeenCalledOnce();
    expect(secondOnLoaded).toHaveBeenCalledOnce();
  });

  it('keeps cancellation pending until the original prompt settles', async () => {
    const response = deferred<Awaited<ReturnType<typeof acpPromptSession>>>();
    vi.mocked(acpPromptSession).mockReturnValueOnce(response.promise);
    const onFinish = vi.fn();
    const submission = submit(onFinish);
    await vi.waitFor(() => expect(acpPromptSession).toHaveBeenCalledOnce());
    const attemptId = acpChatSessionStore.getSnapshot(SESSION_ID)?.activePromptAttemptId;

    acpChatSessionController.stop(SESSION_ID);
    await vi.waitFor(() => expect(acpCancelPrompt).toHaveBeenCalledOnce());

    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.pendingCancelPromptAttemptId).toBe(
      attemptId
    );
    await expect(submit()).rejects.toThrow('prompt cancellation is pending');
    response.resolve({ stopReason: 'cancelled' });
    await submission;
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.pendingCancelPromptAttemptId).toBeNull();
    expect(onFinish).not.toHaveBeenCalled();
  });

  it('does not cancel a newer prompt permission when an earlier cancellation finishes late', async () => {
    const response = deferred<Awaited<ReturnType<typeof acpPromptSession>>>();
    const cancellation = deferred<void>();
    vi.mocked(acpPromptSession).mockReturnValueOnce(response.promise);
    vi.mocked(acpCancelPrompt).mockReturnValueOnce(cancellation.promise);
    const submission = submit();
    await vi.waitFor(() => expect(acpPromptSession).toHaveBeenCalledOnce());
    const previousPermission = requestAcpPermission({
      sessionId: SESSION_ID,
      toolCall: { toolCallId: 'tool-1', title: 'Read file' },
      options: [{ optionId: 'allow-once', name: 'Allow once', kind: 'allow_once' }],
    });

    acpChatSessionController.stop(SESSION_ID);
    response.resolve({ stopReason: 'end_turn' });
    await submission;
    expect(isAcpPermissionRequestPending(SESSION_ID, 'tool-1')).toBe(false);
    await expect(previousPermission).resolves.toEqual({ outcome: { outcome: 'cancelled' } });

    const nextResponse = deferred<Awaited<ReturnType<typeof acpPromptSession>>>();
    vi.mocked(acpPromptSession).mockReturnValueOnce(nextResponse.promise);
    const nextSubmission = submit();
    await vi.waitFor(() => expect(acpPromptSession).toHaveBeenCalledTimes(2));
    const permission = requestAcpPermission({
      sessionId: SESSION_ID,
      toolCall: { toolCallId: 'tool-2', title: 'Read another file' },
      options: [{ optionId: 'allow-once', name: 'Allow once', kind: 'allow_once' }],
    });

    cancellation.resolve();
    await cancellation.promise;
    expect(isAcpPermissionRequestPending(SESSION_ID, 'tool-2')).toBe(true);
    expect(resolveAcpPermissionRequest(SESSION_ID, 'tool-2', 'allow_once')).toBe(true);
    await permission;
    nextResponse.resolve({ stopReason: 'end_turn' });
    await nextSubmission;
  });

  it('keeps the wakelock when completion immediately starts the next prompt', async () => {
    const nextResponse = deferred<Awaited<ReturnType<typeof acpPromptSession>>>();
    vi.mocked(acpPromptSession)
      .mockResolvedValueOnce({ stopReason: 'end_turn' })
      .mockReturnValueOnce(nextResponse.promise);
    let nextSubmission: Promise<void> | undefined;

    await submit(
      vi.fn(() => {
        nextSubmission = submit();
      })
    );

    await vi.waitFor(() => expect(acpPromptSession).toHaveBeenCalledTimes(2));
    expect(window.electron.setWakelockActive).not.toHaveBeenCalledWith(SESSION_ID, false);
    nextResponse.resolve({ stopReason: 'end_turn' });
    await nextSubmission;
    expect(window.electron.setWakelockActive).toHaveBeenLastCalledWith(SESSION_ID, false);
  });

  it('allows retry after a shared session load fails', async () => {
    const artifacts = deferred<Awaited<ReturnType<typeof acpListSessionArtifacts>>>();
    vi.mocked(acpListSessionArtifacts).mockReturnValueOnce(artifacts.promise);
    const onLoaded = vi.fn();
    const firstLoad = acpChatSessionController.loadSession(SESSION_ID, {
      onSessionLoaded: onLoaded,
    });
    const secondLoad = acpChatSessionController.loadSession(SESSION_ID);
    await vi.waitFor(() => expect(acpListSessionArtifacts).toHaveBeenCalledOnce());

    artifacts.reject(new Error('Inventory unavailable'));
    await expect(Promise.all([firstLoad, secondLoad])).resolves.toEqual([false, false]);
    expect(onLoaded).not.toHaveBeenCalled();
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.sessionLoadError).toContain(
      'Inventory unavailable'
    );

    await expect(acpChatSessionController.loadSession(SESSION_ID, { force: true })).resolves.toBe(
      true
    );
    expect(acpLoadSession).toHaveBeenCalledTimes(2);
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.sessionLoadError).toBeUndefined();
  });
});
