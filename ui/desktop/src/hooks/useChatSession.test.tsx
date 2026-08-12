import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { acpChatSessionActions, acpChatSessionStore } from '../acp/chatSessionStore';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Message } from '../types/message';
import type { Session } from '../types/session';
import { useChatSession } from './useChatSession';

const mocks = vi.hoisted(() => ({
  acpListSessionMessages: vi.fn(),
  loadSession: vi.fn(() => Promise.resolve(true)),
  toastError: vi.fn(),
}));

vi.mock('../acp/chatSessionController', () => ({
  acpChatSessionController: {
    loadSession: mocks.loadSession,
    stop: vi.fn(),
    submitMessage: vi.fn(),
    updateMessage: vi.fn(),
  },
}));

vi.mock('../acp/sessions', () => ({
  acpListSessionMessages: mocks.acpListSessionMessages,
}));

vi.mock('../toasts', () => ({
  toastError: mocks.toastError,
}));

const SESSION_ID = 'thread-navigation-session';

function session(): Session {
  return {
    id: SESSION_ID,
    name: 'Thread navigation',
    message_count: 1,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    working_dir: '/tmp',
    extension_data: { active: [], installed: [] },
  } as Session;
}

function message(id: string): Message {
  return {
    id,
    role: 'user',
    content: [{ type: 'text', text: id }],
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

describe('useChatSession history navigation', () => {
  beforeEach(() => {
    mocks.acpListSessionMessages.mockReset();
    mocks.loadSession.mockClear();
    mocks.toastError.mockClear();
    acpChatSessionActions.finishSessionLoad(SESSION_ID, session(), 1);
    acpChatSessionActions.setMessages(SESSION_ID, [message('current')]);
    acpChatSessionActions.setHistoryPageState(SESSION_ID, {
      cursor: 'cursor-2',
      hasMore: true,
      loading: false,
    });
  });

  afterEach(() => {
    acpChatSessionActions.deleteSnapshot(SESSION_ID);
  });

  it('reports success only after reaching the oldest history page', async () => {
    mocks.acpListSessionMessages
      .mockResolvedValueOnce({
        messages: [message('older-2')],
        nextBeforeCursor: 'cursor-1',
        totalCount: 3,
      })
      .mockResolvedValueOnce({
        messages: [message('older-1')],
        nextBeforeCursor: null,
        totalCount: 3,
      });
    const { result } = renderHook(
      () =>
        useChatSession({
          sessionId: SESSION_ID,
          onStreamFinish: vi.fn(),
        }),
      { wrapper: IntlTestWrapper }
    );

    let reachedStart = false;
    await act(async () => {
      reachedStart = await result.current.loadAllOlderMessages();
    });

    expect(reachedStart).toBe(true);
    expect(mocks.acpListSessionMessages).toHaveBeenCalledTimes(2);
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.historyHasMore).toBe(false);
  });

  it('reports failure and preserves the older-history cursor when loading fails', async () => {
    mocks.acpListSessionMessages.mockRejectedValueOnce(new Error('offline'));
    const { result } = renderHook(
      () =>
        useChatSession({
          sessionId: SESSION_ID,
          onStreamFinish: vi.fn(),
        }),
      { wrapper: IntlTestWrapper }
    );

    let reachedStart = true;
    await act(async () => {
      reachedStart = await result.current.loadAllOlderMessages();
    });

    expect(reachedStart).toBe(false);
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.historyHasMore).toBe(true);
    expect(acpChatSessionStore.getSnapshot(SESSION_ID)?.historyCursor).toBe('cursor-2');
    expect(mocks.toastError).toHaveBeenCalledWith({
      title: 'Failed to load older messages',
      msg: 'offline',
    });
  });
});
