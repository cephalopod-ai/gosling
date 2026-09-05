import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { acpChatSessionActions, acpChatSessionStore } from '../acp/chatSessionStore';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Message } from '../types/message';
import type { Session } from '../types/session';
import { useChatSession } from './useChatSession';
import { acpSteerSession } from '../acp/prompt';
import { resolveSessionLibraryInputs } from '../acp/sessionLibraryInputs';
import {
  clearSelectedSessionInputs,
  getSelectedSessionInputs,
  setSessionInputSelected,
} from '../acp/sessionInputSelection';

vi.mock('../acp/prompt', () => ({ acpSteerSession: vi.fn() }));
vi.mock('../acp/sessionLibraryInputs', () => ({ resolveSessionLibraryInputs: vi.fn() }));

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
    clearSelectedSessionInputs(SESSION_ID, getSelectedSessionInputs(SESSION_ID));
    acpChatSessionActions.deleteSnapshot(SESSION_ID);
  });

  function setActiveRun(activeRunId: string | null) {
    acpChatSessionActions.applyAcpSessionNotification({
      sessionId: SESSION_ID,
      update: { sessionUpdate: 'session_info_update', _meta: { gosling: { activeRunId } } },
    });
  }

  function renderActiveChat() {
    setActiveRun('run-1');
    setSessionInputSelected(SESSION_ID, 'notes', true);
    vi.mocked(acpSteerSession)
      .mockReset()
      .mockResolvedValue({ messageId: 'steer-1' } as never);
    vi.mocked(resolveSessionLibraryInputs)
      .mockReset()
      .mockResolvedValue({ assistantContext: 'Source notes', images: [] });
    return renderHook(() => useChatSession({ sessionId: SESSION_ID, onStreamFinish: vi.fn() }), {
      wrapper: IntlTestWrapper,
    });
  }

  it('includes selected inputs when Send now steers a running reply', async () => {
    const { result } = renderActiveChat();
    await act(async () => {
      expect(
        await result.current.onSteerQueuedMessage?.({ msg: 'Use the notes', images: [] })
      ).toBe(true);
    });
    expect(resolveSessionLibraryInputs).toHaveBeenCalledWith(SESSION_ID, ['notes']);
    expect(acpSteerSession).toHaveBeenCalledWith(
      SESSION_ID,
      expect.objectContaining({
        content: expect.arrayContaining([
          expect.objectContaining({
            type: 'text',
            text: 'Source notes',
            annotations: { audience: ['assistant'] },
          }),
          expect.objectContaining({ type: 'text', text: 'Use the notes' }),
        ]),
      }),
      'run-1'
    );
    expect(getSelectedSessionInputs(SESSION_ID)).toEqual([]);
  });

  it('preserves selected inputs when steering fails', async () => {
    const { result } = renderActiveChat();
    vi.mocked(acpSteerSession).mockRejectedValue(new Error('Run ended'));
    await act(async () => {
      expect(
        await result.current.onSteerQueuedMessage?.({ msg: 'Use the notes', images: [] })
      ).toBe(false);
    });
    expect(getSelectedSessionInputs(SESSION_ID)).toEqual(['notes']);
  });

  it('does not steer a different run when input preparation finishes late', async () => {
    const { result } = renderActiveChat();
    vi.mocked(resolveSessionLibraryInputs).mockImplementation(async () => {
      setActiveRun('run-2');
      return { assistantContext: 'Notes', images: [] };
    });
    await act(async () => {
      expect(
        await result.current.onSteerQueuedMessage?.({ msg: 'Use the notes', images: [] })
      ).toBe(false);
    });
    expect(acpSteerSession).not.toHaveBeenCalled();
    expect(getSelectedSessionInputs(SESSION_ID)).toEqual(['notes']);
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
