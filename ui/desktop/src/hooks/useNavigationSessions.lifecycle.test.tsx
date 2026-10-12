import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppEvents } from '../constants/events';
import { acpListRecentSessions, type SessionListItem } from '../acp/sessions';
import { useNavigationSessions } from './useNavigationSessions';

const workspace = vi.hoisted(() => ({ sessionWorkspaceFilterId: null as string | null }));
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
  useLocation: () => ({ pathname: '/' }),
  useSearchParams: () => [new URLSearchParams()],
}));
vi.mock('../contexts/ChatContext', () => ({ useChatContext: () => null }));
vi.mock('../contexts/WorkspaceContext', () => ({ useWorkspace: () => workspace }));
vi.mock('../sessions', () => ({ getSessionDisplayName: (s: { name: string }) => s.name }));
vi.mock('../acp/sessions', () => ({
  acpListRecentSessions: vi.fn(),
  acpGetSessionListItem: vi.fn(),
}));

function deferred() {
  let resolve!: (value: SessionListItem[]) => void;
  const promise = new Promise<SessionListItem[]>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

function session(id: string, workspaceId?: string): SessionListItem {
  return {
    id,
    name: id,
    workingDir: '/tmp',
    updatedAt: '',
    createdAt: '',
    messageCount: 1,
    workspaceId,
  };
}

beforeEach(() => {
  workspace.sessionWorkspaceFilterId = null;
  vi.useFakeTimers();
  vi.mocked(acpListRecentSessions).mockResolvedValue([]);
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});

describe('navigation session refresh lifecycle', () => {
  it('refreshes once on creation and receives names through notifications without polling', async () => {
    vi.mocked(acpListRecentSessions).mockResolvedValue([session('new-chat')]);
    const { result } = renderHook(() => useNavigationSessions());
    await act(async () => {
      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(acpListRecentSessions).toHaveBeenCalledOnce();
    act(() => {
      window.dispatchEvent(
        new CustomEvent(AppEvents.SESSION_RENAMED, {
          detail: { sessionId: 'new-chat', newName: 'Generated title' },
        })
      );
    });
    expect(result.current.recentSessions[0].name).toBe('Generated title');
    expect(acpListRecentSessions).toHaveBeenCalledOnce();
  });

  it('does not restart requests after unmounting during a refresh', async () => {
    const pending = deferred();
    vi.mocked(acpListRecentSessions).mockReturnValue(pending.promise);
    const { unmount } = renderHook(() => useNavigationSessions());
    act(() => {
      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
    });
    unmount();
    await act(async () => {
      pending.resolve([session('obsolete')]);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(acpListRecentSessions).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('ignores the old workspace response when the selected workspace changes', async () => {
    const old = deferred();
    workspace.sessionWorkspaceFilterId = 'old';
    vi.mocked(acpListRecentSessions).mockReturnValueOnce(old.promise);
    const { result, rerender } = renderHook(() => useNavigationSessions());
    act(() => {
      void result.current.fetchSessions();
    });
    workspace.sessionWorkspaceFilterId = 'new';
    rerender();
    vi.mocked(acpListRecentSessions).mockResolvedValueOnce([session('new-chat', 'new')]);
    await act(async () => {
      await result.current.fetchSessions();
    });
    await act(async () => {
      old.resolve([session('old-chat', 'old')]);
    });
    expect(result.current.recentSessions.map((s) => s.id)).toEqual(['new-chat']);
  });

  it('coalesces repeated invalidations and prevents a deleted chat from returning', async () => {
    const old = deferred();
    vi.mocked(acpListRecentSessions)
      .mockReturnValueOnce(old.promise)
      .mockResolvedValueOnce([session('remaining')]);
    const { result } = renderHook(() => useNavigationSessions());
    act(() => {
      void result.current.fetchSessions();
    });
    act(() => {
      window.dispatchEvent(
        new CustomEvent(AppEvents.SESSION_DELETED, { detail: { sessionId: 'deleted' } })
      );
      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
    });
    expect(acpListRecentSessions).toHaveBeenCalledOnce();
    await act(async () => {
      old.resolve([session('deleted')]);
    });
    expect(acpListRecentSessions).toHaveBeenCalledTimes(2);
    expect(result.current.recentSessions.map((s) => s.id)).toEqual(['remaining']);
  });
});
