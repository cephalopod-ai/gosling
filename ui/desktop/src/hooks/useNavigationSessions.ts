import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { useNavigate, useLocation, useSearchParams } from 'react-router-dom';
import { useChatContext } from '../contexts/ChatContext';
import { getSessionDisplayName } from '../sessions';
import { AppEvents } from '../constants/events';
import type { Session } from '../types/session';
import {
  acpGetSessionListItem,
  acpListRecentSessions,
  type SessionListItem,
} from '../acp/sessions';
import { useWorkspace } from '../contexts/WorkspaceContext';

const MAX_RECENT_SESSIONS = 25;

interface SessionRefreshState {
  workspaceId: string | null;
  active: boolean;
  dirty: boolean;
  inFlight: Promise<void> | null;
}

// Session ids reach us from the ACP server and can carry characters that are meaningful in a
// query string, so they are encoded rather than interpolated.
export function pairSessionPath(sessionId: string): string {
  return `/pair?resumeSessionId=${encodeURIComponent(sessionId)}`;
}

export function createWorkspaceSessionFilter(workspaceId: string | null) {
  return workspaceId ? { workspaceId } : undefined;
}

export function matchesWorkspaceSessionFilter(
  session: Pick<SessionListItem, 'workspaceId'>,
  workspaceId: string | null
): boolean {
  return workspaceId === null || session.workspaceId === workspaceId;
}

function filterSessionsForWorkspace(
  sessions: SessionListItem[],
  workspaceId: string | null
): SessionListItem[] {
  return sessions.filter((session) => matchesWorkspaceSessionFilter(session, workspaceId));
}

export function prependUnique(
  prev: SessionListItem[],
  session: SessionListItem
): SessionListItem[] {
  if (prev.some((s) => s.id === session.id)) return prev;
  return [session, ...prev].slice(0, MAX_RECENT_SESSIONS);
}

function mergeWithEmptyLocals(
  prev: SessionListItem[],
  listed: SessionListItem[],
  workspaceId: string | null
): SessionListItem[] {
  const emptyLocals = prev.filter(
    (local) =>
      local.messageCount === 0 &&
      matchesWorkspaceSessionFilter(local, workspaceId) &&
      !listed.some((s) => s.id === local.id)
  );
  return [...emptyLocals, ...filterSessionsForWorkspace(listed, workspaceId)].slice(
    0,
    MAX_RECENT_SESSIONS
  );
}

export function activeSessionsOnly(sessions: SessionListItem[]): SessionListItem[] {
  return sessions.filter((session) => !session.archivedAt);
}

export function sessionToListItem(s: Session): SessionListItem {
  return {
    id: s.id,
    name: getSessionDisplayName(s),
    workingDir: s.working_dir,
    updatedAt: s.updated_at,
    messageCount: s.message_count,
    lastMessageAt: s.last_message_at ?? undefined,
    createdAt: s.created_at,
    archivedAt: s.archived_at ?? undefined,
    projectId: s.project_id ?? undefined,
    providerId: s.provider_name ?? undefined,
    modelId: s.model_config?.model_name ?? undefined,
    userSetName: s.user_set_name ?? undefined,
    workspaceId: s.workspace_id ?? undefined,
    workspaceName: s.workspace_name ?? undefined,
  };
}

export function useNavigationSessions() {
  const navigate = useNavigate();
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const chatContext = useChatContext();
  const { sessionWorkspaceFilterId } = useWorkspace();
  const sessionFilter = useMemo(
    () => createWorkspaceSessionFilter(sessionWorkspaceFilterId),
    [sessionWorkspaceFilterId]
  );

  const [recentSessions, setRecentSessions] = useState<SessionListItem[]>([]);
  const lastSessionIdRef = useRef<string | null>(null);
  const refreshStateRef = useRef<SessionRefreshState>({
    workspaceId: sessionWorkspaceFilterId,
    active: true,
    dirty: false,
    inFlight: null,
  });

  useEffect(() => {
    const state: SessionRefreshState = {
      workspaceId: sessionWorkspaceFilterId,
      active: true,
      dirty: false,
      inFlight: null,
    };
    refreshStateRef.current = state;
    setRecentSessions([]);
    return () => {
      state.active = false;
    };
  }, [sessionWorkspaceFilterId]);

  const activeSessionId = searchParams.get('resumeSessionId') ?? undefined;
  const currentSessionId =
    location.pathname === '/pair' ? searchParams.get('resumeSessionId') : null;

  useEffect(() => {
    if (currentSessionId) {
      lastSessionIdRef.current = currentSessionId;
    }
  }, [currentSessionId]);

  const fetchSessions = useCallback((): Promise<void> => {
    const state = refreshStateRef.current;
    if (!state.active || state.workspaceId !== sessionWorkspaceFilterId) {
      return Promise.resolve();
    }
    state.dirty = true;
    if (state.inFlight) return state.inFlight;

    state.inFlight = (async () => {
      try {
        while (state.active && state.dirty) {
          state.dirty = false;
          try {
            const sessions = activeSessionsOnly(
              await acpListRecentSessions(MAX_RECENT_SESSIONS, 'active', sessionFilter)
            );
            if (state.active && !state.dirty) {
              setRecentSessions((prev) =>
                mergeWithEmptyLocals(prev, sessions, sessionWorkspaceFilterId)
              );
            }
          } catch (error) {
            if (state.active && !state.dirty) console.error('Failed to fetch sessions:', error);
          }
        }
      } finally {
        state.inFlight = null;
      }
    })();
    return state.inFlight;
  }, [sessionFilter, sessionWorkspaceFilterId]);

  const activeSessionListed = recentSessions.some((s) => s.id === activeSessionId);
  useEffect(() => {
    if (!activeSessionId) return;
    if (activeSessionListed) return;

    let canceled = false;

    acpGetSessionListItem(activeSessionId)
      .then((item) => {
        if (
          !canceled &&
          !item.archivedAt &&
          matchesWorkspaceSessionFilter(item, sessionWorkspaceFilterId)
        ) {
          setRecentSessions((prev) => prependUnique(prev, item));
        }
      })
      .catch((error) => {
        console.error('Failed to fetch active session:', error);
      });

    return () => {
      canceled = true;
    };
  }, [activeSessionId, activeSessionListed, sessionWorkspaceFilterId]);

  useEffect(() => {
    const handleSessionCreated = (event: Event) => {
      const { session } = (event as CustomEvent<{ session?: Session }>).detail || {};
      if (session) {
        const item = sessionToListItem(session);
        if (matchesWorkspaceSessionFilter(item, sessionWorkspaceFilterId)) {
          setRecentSessions((prev) => prependUnique(prev, item));
        }
      }

      void fetchSessions();
    };

    window.addEventListener(AppEvents.SESSION_CREATED, handleSessionCreated);
    return () => {
      window.removeEventListener(AppEvents.SESSION_CREATED, handleSessionCreated);
    };
  }, [fetchSessions, sessionWorkspaceFilterId]);

  useEffect(() => {
    const handleSessionDeleted = (event: Event) => {
      const { sessionId } = (event as CustomEvent<{ sessionId: string }>).detail;

      setRecentSessions((prev) => prev.filter((session) => session.id !== sessionId));

      if (lastSessionIdRef.current === sessionId) {
        lastSessionIdRef.current = null;
      }
      void fetchSessions();
    };

    const handleSessionRenamed = (event: Event) => {
      const { sessionId, newName, userInitiated } = (
        event as CustomEvent<{ sessionId: string; newName: string; userInitiated?: boolean }>
      ).detail;

      setRecentSessions((prev) =>
        prev.map((session) =>
          session.id === sessionId
            ? { ...session, name: newName, ...(userInitiated && { userSetName: true }) }
            : session
        )
      );
      if (refreshStateRef.current.inFlight) void fetchSessions();
    };

    const handleSessionArchived = (event: Event) => {
      const { sessionId } = (event as CustomEvent<{ sessionId: string }>).detail;

      setRecentSessions((prev) => prev.filter((session) => session.id !== sessionId));
      if (lastSessionIdRef.current === sessionId) {
        lastSessionIdRef.current = null;
      }
      void fetchSessions();
    };

    const handleSessionUnarchived = (event: Event) => {
      const { session } = (event as CustomEvent<{ sessionId: string; session?: SessionListItem }>)
        .detail;

      if (session && matchesWorkspaceSessionFilter(session, sessionWorkspaceFilterId)) {
        setRecentSessions((prev) => prependUnique(prev, session));
      }
      void fetchSessions();
    };

    window.addEventListener(AppEvents.SESSION_DELETED, handleSessionDeleted);
    window.addEventListener(AppEvents.SESSION_ARCHIVED, handleSessionArchived);
    window.addEventListener(AppEvents.SESSION_UNARCHIVED, handleSessionUnarchived);
    window.addEventListener(AppEvents.SESSION_RENAMED, handleSessionRenamed);

    return () => {
      window.removeEventListener(AppEvents.SESSION_DELETED, handleSessionDeleted);
      window.removeEventListener(AppEvents.SESSION_ARCHIVED, handleSessionArchived);
      window.removeEventListener(AppEvents.SESSION_UNARCHIVED, handleSessionUnarchived);
      window.removeEventListener(AppEvents.SESSION_RENAMED, handleSessionRenamed);
    };
  }, [fetchSessions, sessionWorkspaceFilterId]);

  const handleNavClick = useCallback(
    (path: string) => {
      if (path === '/pair') {
        const sessionId =
          currentSessionId || lastSessionIdRef.current || chatContext?.chat?.sessionId;
        if (sessionId && sessionId.length > 0) {
          navigate(pairSessionPath(sessionId));
        } else {
          navigate('/');
        }
      } else {
        navigate(path);
      }
    },
    [navigate, currentSessionId, chatContext?.chat?.sessionId]
  );

  const handleSessionClick = useCallback(
    (sessionId: string) => {
      navigate(pairSessionPath(sessionId));
    },
    [navigate]
  );

  return {
    recentSessions,
    activeSessionId,
    fetchSessions,
    handleNavClick,
    handleSessionClick,
  };
}
