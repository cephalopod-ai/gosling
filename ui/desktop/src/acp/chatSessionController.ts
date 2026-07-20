import { v7 as uuidv7 } from 'uuid';
import type { GoslingExtension } from '@repo-makeover/gosling-sdk';
import { AppEvents } from '../constants/events';
import { ChatState } from '../types/chatState';
import type { Session } from '../types/session';
import { errorMessage } from '../utils/conversionUtils';
import { showExtensionLoadResults } from '../utils/extensionErrorUtils';
import { createUserMessage, getPendingToolConfirmationIds, type Message } from '../types/message';
import {
  acpChatSessionActions,
  acpChatSessionStore,
  type AcpChatSessionSnapshot,
} from './chatSessionStore';
import { cancelAcpElicitationRequestsForSession } from './elicitationRequests';
import {
  isAcpConnectionClosedError,
  parseAcpCreditsExhaustedError,
  type AcpCreditsExhaustedError,
} from './errors';
import { cancelAcpPermissionRequestsForSession } from './permissionRequests';
import { acpCancelPrompt, acpPromptSession } from './prompt';
import { getAcpConnectionGeneration } from './acpConnection';
import {
  acpForkSession,
  acpLoadSession,
  acpNewSession,
  acpTruncateSessionConversation,
  isAcpSessionLoadInFlight,
  sessionInfoToSession,
  type AcpWorkspaceLaunchOptions,
} from './sessions';

export interface AcpLoadSessionOptions {
  onSessionLoaded?: () => void;
  force?: boolean;
}

export interface AcpSnapshotOptions {
  getCurrentSnapshot(): AcpChatSessionSnapshot | undefined;
}

export interface AcpSubmitMessageOptions extends AcpSnapshotOptions {
  onFinish(error?: string): void | Promise<void>;
}

export interface AcpChatSessionController {
  createSession(
    cwd: string,
    goslingExtensions: GoslingExtension[],
    workspaceId?: string,
    workspaceLaunchOptions?: AcpWorkspaceLaunchOptions
  ): Promise<Session>;
  loadSession(sessionId: string, options?: AcpLoadSessionOptions): Promise<boolean>;
  submitMessage(
    sessionId: string,
    userMessage: Message,
    options: AcpSubmitMessageOptions
  ): Promise<void>;
  stop(sessionId: string): void;
  updateMessage(
    sessionId: string,
    messageId: string,
    newContent: string,
    editType: 'fork' | 'edit' | undefined,
    options: AcpSubmitMessageOptions
  ): Promise<void>;
}

function createAcpCreditsExhaustedMessage(error: AcpCreditsExhaustedError): Message {
  return {
    id: uuidv7(),
    role: 'assistant',
    created: Math.floor(Date.now() / 1000),
    content: [
      {
        type: 'systemNotification',
        notificationType: 'creditsExhausted',
        msg: error.message,
        ...(error.url ? { data: { top_up_url: error.url } } : {}),
      },
    ],
    metadata: { userVisible: true, agentVisible: false },
  };
}

function assertNoPendingPromptCancellation(sessionId: string): void {
  const snapshot = acpChatSessionStore.getSnapshot(sessionId);
  if (snapshot?.pendingCancelPromptAttemptId) {
    throw new Error('Cannot submit while prompt cancellation is pending');
  }
}

async function forkSessionWithEditedMessage(
  sessionId: string,
  message: Message,
  editedMessage: string
): Promise<void> {
  const targetSessionId = await acpForkSession(sessionId, message.created);

  const event = new CustomEvent(AppEvents.SESSION_FORKED, {
    detail: {
      newSessionId: targetSessionId,
      shouldStartAgent: true,
      editedMessage,
    },
  });
  window.dispatchEvent(event);
}

async function createSession(
  cwd: string,
  goslingExtensions: GoslingExtension[],
  workspaceId?: string,
  workspaceLaunchOptions?: AcpWorkspaceLaunchOptions
): Promise<Session> {
  const { sessionId, sessionInfo, meta } = await acpNewSession(
    cwd,
    goslingExtensions,
    workspaceId,
    workspaceLaunchOptions
  );
  const session = sessionInfoToSession(sessionInfo, meta);
  const connectionGeneration = getAcpConnectionGeneration();
  if (connectionGeneration === null) {
    throw new Error('ACP connection closed while creating the session');
  }

  showExtensionLoadResults(meta.extensionResults);
  window.dispatchEvent(
    new CustomEvent(AppEvents.SESSION_EXTENSIONS_LOADED, { detail: { sessionId } })
  );
  acpChatSessionActions.finishSessionLoad(sessionId, session, connectionGeneration);

  return session;
}

async function loadSession(
  sessionId: string,
  options: AcpLoadSessionOptions = {}
): Promise<boolean> {
  const cached = acpChatSessionStore.getSnapshot(sessionId);
  const connectionGeneration = getAcpConnectionGeneration();
  if (
    !options.force &&
    cached?.session &&
    connectionGeneration !== null &&
    cached.connectionGeneration === connectionGeneration
  ) {
    window.dispatchEvent(
      new CustomEvent(AppEvents.SESSION_EXTENSIONS_LOADED, { detail: { sessionId } })
    );
    options.onSessionLoaded?.();
    return true;
  }

  if (!isAcpSessionLoadInFlight(sessionId)) {
    acpChatSessionActions.startSessionLoad(sessionId);
  }

  try {
    const { sessionInfo, meta } = await acpLoadSession(sessionId);
    const loadedConnectionGeneration = getAcpConnectionGeneration();
    if (loadedConnectionGeneration === null) {
      throw new Error('ACP connection closed while loading the session');
    }

    showExtensionLoadResults(meta.extensionResults);
    window.dispatchEvent(
      new CustomEvent(AppEvents.SESSION_EXTENSIONS_LOADED, { detail: { sessionId } })
    );
    acpChatSessionActions.finishSessionLoad(
      sessionId,
      sessionInfoToSession(sessionInfo, meta),
      loadedConnectionGeneration
    );
    if (meta.historyLoad?.mode === 'compacted') {
      acpChatSessionActions.setHistoryPageState(sessionId, {
        cursor: meta.historyLoad.nextBeforeCursor ?? null,
        hasMore: meta.historyLoad.nextBeforeCursor != null,
        loading: false,
        totalCount: meta.historyLoad.totalCount ?? null,
      });
    }
    options.onSessionLoaded?.();
    return true;
  } catch (error) {
    console.error('Failed to load ACP session:', error);
    acpChatSessionActions.failSessionLoad(sessionId, errorMessage(error));
    return false;
  }
}

async function submitMessage(
  sessionId: string,
  userMessage: Message,
  options: AcpSubmitMessageOptions
): Promise<void> {
  assertNoPendingPromptCancellation(sessionId);

  const snapshot = acpChatSessionStore.getSnapshot(sessionId);
  if (snapshot?.activePromptAttemptId) {
    return;
  }

  const promptAttemptId = uuidv7();
  acpChatSessionActions.startPromptAttempt(sessionId, promptAttemptId);
  await window.electron.setWakelockActive(sessionId, true).catch(() => false);

  try {
    await acpPromptSession(sessionId, userMessage);
    if (acpChatSessionActions.clearPromptCancellation(sessionId, promptAttemptId)) {
      return;
    }
    if (acpChatSessionActions.finishPromptAttemptIfCurrent(sessionId, promptAttemptId)) {
      void options.onFinish();
    }
  } catch (error) {
    if (acpChatSessionActions.clearPromptCancellation(sessionId, promptAttemptId)) {
      return;
    }

    const creditsExhaustedError = parseAcpCreditsExhaustedError(error);
    if (creditsExhaustedError) {
      if (!acpChatSessionActions.isCurrentPromptAttempt(sessionId, promptAttemptId)) {
        return;
      }

      const messages = [
        ...(options.getCurrentSnapshot()?.messages ?? []),
        createAcpCreditsExhaustedMessage(creditsExhaustedError),
      ];
      acpChatSessionActions.setMessages(sessionId, messages);
      if (acpChatSessionActions.finishPromptAttemptIfCurrent(sessionId, promptAttemptId)) {
        void options.onFinish();
      }
      return;
    }

    const submitError = {
      message: 'Submit error: ' + errorMessage(error),
      connectionLost: isAcpConnectionClosedError(error),
    };
    if (
      acpChatSessionActions.finishPromptAttemptIfCurrent(sessionId, promptAttemptId, submitError)
    ) {
      void options.onFinish(submitError.message);
    }
  } finally {
    await window.electron.setWakelockActive(sessionId, false).catch(() => false);
  }
}

function stop(sessionId: string): void {
  const storedPromptAttemptId = acpChatSessionStore.getSnapshot(sessionId)?.activePromptAttemptId;
  const hasStoredAcpPrompt = storedPromptAttemptId !== null && storedPromptAttemptId !== undefined;

  if (hasStoredAcpPrompt) {
    acpChatSessionActions.startPromptCancellation(sessionId, storedPromptAttemptId);
    cancelAcpPermissionRequestsForSession(sessionId);
    cancelAcpElicitationRequestsForSession(sessionId);
    acpCancelPrompt(sessionId).catch((error) => {
      console.warn('Failed to cancel ACP prompt:', error);
    });
    return;
  }

  acpChatSessionActions.setChatState(sessionId, ChatState.Idle);
}

async function updateMessage(
  sessionId: string,
  messageId: string,
  newContent: string,
  editType: 'fork' | 'edit' | undefined,
  options: AcpSubmitMessageOptions
): Promise<void> {
  assertNoPendingPromptCancellation(sessionId);

  const resolvedEditType = editType ?? 'fork';
  const currentSnapshot = options.getCurrentSnapshot();
  const storedSnapshot = acpChatSessionStore.getSnapshot(sessionId);
  const activePromptAttemptId = storedSnapshot?.activePromptAttemptId;
  const currentMessages = currentSnapshot?.messages ?? [];
  const message = currentMessages.find((m) => m.id === messageId);

  if (!message) {
    throw new Error(`Message with id ${messageId} not found in current messages`);
  }

  if (resolvedEditType === 'fork') {
    await forkSessionWithEditedMessage(sessionId, message, newContent);
    return;
  }

  const editSnapshot = currentSnapshot ?? storedSnapshot;
  const isPendingToolPermission =
    editSnapshot?.chatState === ChatState.WaitingForUserInput &&
    getPendingToolConfirmationIds(editSnapshot?.messages ?? []).size > 0;
  const isIdle = editSnapshot?.chatState === ChatState.Idle;
  const pendingToolPermissionPromptAttemptId = isPendingToolPermission
    ? activePromptAttemptId
    : undefined;
  const canEditInPlace = isIdle || pendingToolPermissionPromptAttemptId != null;

  if (!canEditInPlace) {
    return;
  }

  if (pendingToolPermissionPromptAttemptId != null) {
    const cancellation = acpChatSessionActions.startPromptCancellation(
      sessionId,
      pendingToolPermissionPromptAttemptId
    );
    if (!cancellation) {
      throw new Error('Cannot update message while prompt is active');
    }

    const promptCancellationSettled = acpChatSessionActions.waitForPromptCancellation(
      sessionId,
      pendingToolPermissionPromptAttemptId
    );

    try {
      await acpCancelPrompt(sessionId);
    } catch {
      acpChatSessionActions.restorePromptCancellation(
        sessionId,
        pendingToolPermissionPromptAttemptId
      );
      throw new Error('Cannot update message because the active prompt could not be cancelled');
    }

    cancelAcpPermissionRequestsForSession(sessionId);
    cancelAcpElicitationRequestsForSession(sessionId);
    await promptCancellationSettled;
  }

  acpChatSessionActions.setChatState(sessionId, ChatState.Thinking);

  try {
    await acpTruncateSessionConversation(sessionId, message.created);

    const truncatedMessages = currentMessages.filter((m) => m.created < message.created);
    const updatedUserMessage = createUserMessage(newContent);

    for (const content of message.content) {
      if (content.type === 'image') {
        updatedUserMessage.content.push(content);
      }
    }

    const messagesForUI = [...truncatedMessages, updatedUserMessage];
    acpChatSessionActions.setMessages(sessionId, messagesForUI);

    await submitMessage(sessionId, updatedUserMessage, options);
  } catch (error) {
    acpChatSessionActions.setChatState(sessionId, ChatState.Idle);
    throw error;
  }
}

export const acpChatSessionController: AcpChatSessionController = {
  createSession,
  loadSession,
  submitMessage,
  stop,
  updateMessage,
};
