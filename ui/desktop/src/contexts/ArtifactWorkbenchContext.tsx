import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import type { SessionArtifactDto } from '@repo-makeover/gosling-sdk';
import { toast } from 'react-toastify';
import {
  artifactKindFromMetadata,
  artifactKindFromMimeType,
  artifactKindFromPath,
  artifactTitleFromPath,
  isArtifactKindPreviewableWithoutExtension,
  opensInExternalViewer,
} from '../components/artifacts/artifactUtils';
import type { ArtifactTab } from '../components/artifacts/types';
import { coalesceSessionArtifactAliases } from '../utils/sessionArtifactAliases';
import { backendStorageKey } from '../utils/backendStorage';

const STORAGE_KEY = 'gosling-artifact-workbench-v1';
const DEFAULT_SESSION_ID = '__no_session__';
const DEFAULT_WIDTH = 480;
const EMPTY_ARTIFACTS: SessionArtifactDto[] = [];

interface SessionPreviewState {
  activeTabId: string | null;
  tabs: ArtifactTab[];
  deletedArtifacts: Record<string, string>;
}

interface PersistedWorkbench {
  hideRepositoryFiles: boolean;
  isOpen: boolean;
  sessions: Record<string, SessionPreviewState>;
  tabs?: ArtifactTab[];
  activeTabId?: string | null;
  width: number;
}

interface OpenContentInput {
  content: string;
  encoding?: 'base64' | 'utf8';
  mimeType?: string;
  title: string;
  workspaceId?: string;
}

interface ArtifactWorkbenchValue {
  activeTab: ArtifactTab | null;
  activeTabId: string | null;
  artifacts: SessionArtifactDto[];
  trashedArtifacts: SessionArtifactDto[];
  closeTab: (id: string) => void;
  closeAllTabs: () => void;
  forgetTrashedFiles: (paths: string[]) => void;
  hideRepositoryFiles: boolean;
  isOpen: boolean;
  openArtifact: (artifact: SessionArtifactDto) => void;
  openContent: (input: OpenContentInput) => void;
  openFile: (path: string, baseDirectory?: string, workspaceId?: string) => void;
  resolveFilePath: (id: string, path: string) => void;
  setActiveTabId: (id: string) => void;
  setIsOpen: (isOpen: boolean) => void;
  setHideRepositoryFiles: (hide: boolean) => void;
  setVisibleSession: (sessionId: string | null, artifacts: SessionArtifactDto[]) => void;
  setWidth: (width: number) => void;
  tabs: ArtifactTab[];
  toggle: () => void;
  visibleSessionId: string | null;
  width: number;
}

const ArtifactWorkbenchContext = createContext<ArtifactWorkbenchValue | null>(null);

function createId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `artifact-${Date.now()}-${Math.random()}`;
}

function emptySessionState(): SessionPreviewState {
  return { activeTabId: null, tabs: [], deletedArtifacts: {} };
}

function validSessionState(value: Partial<SessionPreviewState> | undefined): SessionPreviewState {
  const tabs = Array.isArray(value?.tabs)
    ? value.tabs.flatMap((tab) => {
        if (tab?.source?.type !== 'file') return [];
        const pathKind = artifactKindFromPath(tab.source.path);
        if (pathKind !== 'unknown') return [{ ...tab, kind: pathKind }];
        return isArtifactKindPreviewableWithoutExtension(tab.kind) ? [tab] : [];
      })
    : [];
  return {
    deletedArtifacts: Object.fromEntries(
      Object.entries(value?.deletedArtifacts ?? {}).filter(
        ([, version]) => typeof version === 'string'
      )
    ),
    activeTabId: tabs.some((tab) => tab.id === value?.activeTabId)
      ? (value?.activeTabId ?? null)
      : (tabs[0]?.id ?? null),
    tabs,
  };
}

function persistableSessionState(state: SessionPreviewState): SessionPreviewState {
  return { ...state, tabs: state.tabs.filter((tab) => tab.source.type === 'file') };
}

function sameStoredSessionState(local: SessionPreviewState, stored: SessionPreviewState): boolean {
  return (
    JSON.stringify(validSessionState(persistableSessionState(local))) === JSON.stringify(stored)
  );
}

/** A session state another window stored, keeping this window's unpersisted content tabs. */
function adoptStoredSessionState(
  stored: SessionPreviewState,
  local: SessionPreviewState | undefined
): SessionPreviewState {
  const contentTabs = local?.tabs.filter((tab) => tab.source.type === 'content') ?? [];
  if (contentTabs.length === 0) return stored;
  const tabs = [...stored.tabs, ...contentTabs];
  return {
    ...stored,
    tabs,
    activeTabId:
      stored.activeTabId ??
      (tabs.some((tab) => tab.id === local?.activeTabId) ? (local?.activeTabId ?? null) : null),
  };
}

function loadPersistedWorkbench(storageKey: string): PersistedWorkbench {
  try {
    const parsed = JSON.parse(
      localStorage.getItem(storageKey) ?? '{}'
    ) as Partial<PersistedWorkbench>;
    const sessions = Object.fromEntries(
      Object.entries(parsed.sessions ?? {}).map(([sessionId, state]) => [
        sessionId,
        validSessionState(state),
      ])
    );
    if (Array.isArray(parsed.tabs)) {
      sessions[DEFAULT_SESSION_ID] = validSessionState({
        tabs: parsed.tabs,
        activeTabId: parsed.activeTabId,
      });
    }
    return {
      hideRepositoryFiles: parsed.hideRepositoryFiles === true,
      isOpen: parsed.isOpen === true,
      sessions,
      width:
        typeof parsed.width === 'number'
          ? Math.min(720, Math.max(320, parsed.width))
          : DEFAULT_WIDTH,
    };
  } catch {
    return { hideRepositoryFiles: false, isOpen: false, sessions: {}, width: DEFAULT_WIDTH };
  }
}

export function ArtifactWorkbenchProvider({ children }: { children: React.ReactNode }) {
  const [storageKey] = useState(() => backendStorageKey(STORAGE_KEY));
  const [initial] = useState(() => loadPersistedWorkbench(storageKey));
  const [visibleSessionId, setVisibleSessionId] = useState(DEFAULT_SESSION_ID);
  const [artifactsBySession, setArtifactsBySession] = useState<
    Record<string, SessionArtifactDto[]>
  >({});
  const [sessions, setSessions] = useState(initial.sessions);
  // Windows on the same backend share this storage key. Session states this window last wrote or
  // adopted; only the ones it has changed since are written back, merged over what is stored,
  // so one window's save no longer restores tabs another window closed.
  const syncedSessionsRef = useRef(initial.sessions);
  const sessionsRef = useRef(initial.sessions);
  const [isOpen, setIsOpen] = useState(initial.isOpen);
  const [hideRepositoryFiles, setHideRepositoryFiles] = useState(initial.hideRepositoryFiles);
  const [width, setWidthState] = useState(initial.width);
  const current = sessions[visibleSessionId] ?? emptySessionState();
  const deletedArtifacts = sessions[visibleSessionId]?.deletedArtifacts;
  const artifacts = useMemo(
    () =>
      (artifactsBySession[visibleSessionId] ?? EMPTY_ARTIFACTS).filter(
        (artifact) => deletedArtifacts?.[artifact.resolvedPath] !== artifact.lastSeenAt
      ),
    [artifactsBySession, visibleSessionId, deletedArtifacts]
  );

  const trashedArtifacts = useMemo(
    () =>
      (artifactsBySession[visibleSessionId] ?? EMPTY_ARTIFACTS).filter(
        (artifact) => deletedArtifacts?.[artifact.resolvedPath] === artifact.lastSeenAt
      ),
    [artifactsBySession, visibleSessionId, deletedArtifacts]
  );

  useEffect(() => {
    sessionsRef.current = sessions;
    const synced = syncedSessionsRef.current;
    const merged: Record<string, SessionPreviewState> = {
      ...loadPersistedWorkbench(storageKey).sessions,
      ...Object.fromEntries(
        Object.entries(sessions)
          .filter(([sessionId, state]) => synced[sessionId] !== state)
          .map(([sessionId, state]) => [sessionId, persistableSessionState(state)])
      ),
    };
    syncedSessionsRef.current = sessions;
    const fallback = merged[DEFAULT_SESSION_ID] ?? emptySessionState();
    const persisted: PersistedWorkbench = {
      hideRepositoryFiles,
      isOpen,
      sessions: merged,
      tabs: fallback.tabs,
      activeTabId: fallback.activeTabId,
      width,
    };
    localStorage.setItem(storageKey, JSON.stringify(persisted));
  }, [hideRepositoryFiles, isOpen, sessions, storageKey, width]);

  useEffect(() => {
    const adoptOtherWindowChanges = (event: StorageEvent) => {
      if (event.key !== storageKey) return;
      const local = sessionsRef.current;
      const adopted = Object.fromEntries(
        Object.entries(loadPersistedWorkbench(storageKey).sessions)
          .filter(
            ([sessionId, stored]) =>
              !local[sessionId] || !sameStoredSessionState(local[sessionId], stored)
          )
          .map(([sessionId, stored]) => [
            sessionId,
            adoptStoredSessionState(stored, local[sessionId]),
          ])
      );
      if (Object.keys(adopted).length === 0) return;
      syncedSessionsRef.current = { ...syncedSessionsRef.current, ...adopted };
      setSessions((all) => {
        const next = { ...all };
        for (const [sessionId, state] of Object.entries(adopted)) {
          // A change this window made after the event was read wins; it is written next.
          if (all[sessionId] === local[sessionId]) next[sessionId] = state;
        }
        return next;
      });
    };
    window.addEventListener('storage', adoptOtherWindowChanges);
    return () => window.removeEventListener('storage', adoptOtherWindowChanges);
  }, [storageKey]);

  const updateCurrent = useCallback(
    (update: (state: SessionPreviewState) => SessionPreviewState) => {
      setSessions((all) => ({
        ...all,
        [visibleSessionId]: update(all[visibleSessionId] ?? emptySessionState()),
      }));
    },
    [visibleSessionId]
  );

  /// Office, OpenDocument and PDF documents go to the system viewer rather than a
  /// pane tab: Gosling has no renderer for the Office formats, so a tab would have
  /// shown "no in-app preview", and `openFile` used to drop those clicks entirely.
  const launchInExternalViewer = useCallback((path: string, baseDirectory?: string) => {
    void window.electron
      .openArtifactFile(path, baseDirectory)
      .then((opened) => {
        if (!opened)
          toast.error(`No application is available to open ${artifactTitleFromPath(path)}`);
      })
      .catch(() => {
        toast.error(`Could not open ${artifactTitleFromPath(path)}`);
      });
  }, []);

  const openFile = useCallback(
    (path: string, baseDirectory?: string, workspaceId?: string) => {
      if (opensInExternalViewer(path)) {
        launchInExternalViewer(path, baseDirectory);
        return;
      }
      const kind = artifactKindFromPath(path);
      if (kind === 'unknown') return;
      updateCurrent((state) => {
        const existing = state.tabs.find(
          (tab) =>
            tab.source.type === 'file' &&
            tab.source.path === path &&
            tab.source.baseDirectory === baseDirectory &&
            tab.workspaceId === workspaceId
        );
        if (existing) return { ...state, activeTabId: existing.id };
        const tab: ArtifactTab = {
          id: createId(),
          kind,
          source: { type: 'file', path, baseDirectory },
          title: artifactTitleFromPath(path),
          workspaceId,
        };
        return { ...state, activeTabId: tab.id, tabs: [...state.tabs, tab] };
      });
      setIsOpen(true);
    },
    [launchInExternalViewer, updateCurrent]
  );

  const openArtifact = useCallback(
    (artifact: SessionArtifactDto) => {
      if (opensInExternalViewer(artifact.displayPath)) {
        launchInExternalViewer(artifact.displayPath, artifact.baseWorkingDir);
        return;
      }
      const kind = artifactKindFromMetadata(artifact.displayPath, artifact.mimeType);
      updateCurrent((state) => {
        const existing = state.tabs.find(
          (tab) =>
            tab.source.type === 'file' &&
            (tab.source.path === artifact.resolvedPath ||
              (tab.source.path === artifact.displayPath &&
                tab.source.baseDirectory === artifact.baseWorkingDir))
        );
        if (existing) return { ...state, activeTabId: existing.id };
        const tab: ArtifactTab = {
          id: createId(),
          kind,
          source: {
            type: 'file',
            path: artifact.displayPath,
            baseDirectory: artifact.baseWorkingDir,
          },
          title: artifactTitleFromPath(artifact.displayPath),
          workspaceId: artifact.workspaceId ?? undefined,
        };
        return { ...state, activeTabId: tab.id, tabs: [...state.tabs, tab] };
      });
      setIsOpen(true);
    },
    [launchInExternalViewer, updateCurrent]
  );

  const openContent = useCallback(
    (input: OpenContentInput) => {
      const mimeType = input.mimeType ?? 'text/plain';
      const tab: ArtifactTab = {
        id: createId(),
        kind: artifactKindFromMimeType(mimeType),
        source: {
          type: 'content',
          content: input.content,
          encoding: input.encoding ?? 'utf8',
          mimeType,
        },
        title: input.title,
        workspaceId: input.workspaceId,
      };
      updateCurrent((state) => ({ ...state, activeTabId: tab.id, tabs: [...state.tabs, tab] }));
      setIsOpen(true);
    },
    [updateCurrent]
  );

  const closeTab = useCallback(
    (id: string) => {
      updateCurrent((state) => {
        const index = state.tabs.findIndex((tab) => tab.id === id);
        const tabs = state.tabs.filter((tab) => tab.id !== id);
        return {
          ...state,
          tabs,
          activeTabId:
            state.activeTabId === id
              ? (tabs[Math.min(index, tabs.length - 1)]?.id ?? null)
              : state.activeTabId,
        };
      });
    },
    [updateCurrent]
  );

  const closeAllTabs = useCallback(() => {
    updateCurrent((state) => ({ ...state, tabs: [], activeTabId: null }));
  }, [updateCurrent]);

  const forgetTrashedFiles = useCallback(
    (paths: string[]) => {
      const removedPaths = new Set(paths);
      // Preserve discovery history; dismiss only the version actually selected for deletion.
      // This callback captures that inventory even if a different chat is active when Trash finishes.
      setSessions((all) =>
        Object.fromEntries(
          [...new Set([...Object.keys(all), ...Object.keys(artifactsBySession)])].map(
            (sessionId) => {
              const state = all[sessionId] ?? emptySessionState();
              const removedArtifacts = (artifactsBySession[sessionId] ?? []).filter((artifact) =>
                removedPaths.has(artifact.resolvedPath)
              );
              const tabs = state.tabs.filter(
                (tab) =>
                  tab.source.type !== 'file' ||
                  (!removedPaths.has(tab.source.path) &&
                    !removedArtifacts.some(
                      (artifact) =>
                        tab.source.type === 'file' &&
                        tab.source.path === artifact.displayPath &&
                        tab.source.baseDirectory === artifact.baseWorkingDir
                    ))
              );
              return [
                sessionId,
                {
                  ...state,
                  tabs,
                  activeTabId: tabs.some((tab) => tab.id === state.activeTabId)
                    ? state.activeTabId
                    : (tabs[0]?.id ?? null),
                  deletedArtifacts: {
                    ...state.deletedArtifacts,
                    ...Object.fromEntries(
                      removedArtifacts.map((artifact) => [
                        artifact.resolvedPath,
                        artifact.lastSeenAt,
                      ])
                    ),
                  },
                },
              ];
            }
          )
        )
      );
    },
    [artifactsBySession]
  );

  const resolveFilePath = useCallback(
    (id: string, path: string) => {
      updateCurrent((state) => ({
        ...state,
        tabs: state.tabs.map((tab) =>
          tab.id === id && tab.source.type === 'file'
            ? {
                ...tab,
                kind: artifactKindFromPath(path),
                source: { type: 'file', path },
                title: artifactTitleFromPath(path),
              }
            : tab
        ),
      }));
    },
    [updateCurrent]
  );

  const setVisibleSession = useCallback(
    (sessionId: string | null, nextArtifacts: SessionArtifactDto[]) => {
      const key = sessionId ?? DEFAULT_SESSION_ID;
      const coalesced = coalesceSessionArtifactAliases(nextArtifacts);
      setVisibleSessionId(key);
      setArtifactsBySession((currentArtifacts) => ({
        ...currentArtifacts,
        [key]: coalesced.artifacts,
      }));
      if (coalesced.aliases.length > 0) {
        setSessions((all) => {
          const state = all[key];
          if (!state) return all;
          const tabs = state.tabs.map((tab) => {
            if (tab.source.type !== 'file') return tab;
            const alias = coalesced.aliases.find(
              ({ artifact }) =>
                tab.source.type === 'file' &&
                (tab.source.path === artifact.resolvedPath ||
                  (tab.source.path === artifact.displayPath &&
                    tab.source.baseDirectory === artifact.baseWorkingDir))
            );
            if (!alias) return tab;
            return {
              ...tab,
              kind: artifactKindFromMetadata(alias.target.displayPath, alias.target.mimeType),
              source: {
                type: 'file' as const,
                path: alias.target.displayPath,
                baseDirectory: alias.target.baseWorkingDir,
              },
              title: artifactTitleFromPath(alias.target.displayPath),
              workspaceId: alias.target.workspaceId ?? undefined,
            };
          });
          return tabs.some((tab, index) => tab !== state.tabs[index])
            ? { ...all, [key]: { ...state, tabs } }
            : all;
        });
      }
    },
    []
  );

  const setWidth = useCallback((nextWidth: number) => {
    setWidthState(Math.min(720, Math.max(320, nextWidth)));
  }, []);

  const setActiveTabId = useCallback(
    (id: string) => updateCurrent((state) => ({ ...state, activeTabId: id })),
    [updateCurrent]
  );
  const activeTab = current.tabs.find((tab) => tab.id === current.activeTabId) ?? null;
  const value = useMemo<ArtifactWorkbenchValue>(
    () => ({
      activeTab,
      activeTabId: current.activeTabId,
      artifacts,
      trashedArtifacts,
      closeTab,
      closeAllTabs,
      forgetTrashedFiles,
      hideRepositoryFiles,
      isOpen,
      openArtifact,
      openContent,
      openFile,
      resolveFilePath,
      setActiveTabId,
      setIsOpen,
      setHideRepositoryFiles,
      setVisibleSession,
      setWidth,
      tabs: current.tabs,
      toggle: () => setIsOpen((open) => !open),
      visibleSessionId: visibleSessionId === DEFAULT_SESSION_ID ? null : visibleSessionId,
      width,
    }),
    [
      activeTab,
      artifacts,
      trashedArtifacts,
      closeTab,
      closeAllTabs,
      forgetTrashedFiles,
      hideRepositoryFiles,
      current.activeTabId,
      current.tabs,
      isOpen,
      openArtifact,
      openContent,
      openFile,
      resolveFilePath,
      setActiveTabId,
      setVisibleSession,
      setWidth,
      visibleSessionId,
      width,
    ]
  );

  return (
    <ArtifactWorkbenchContext.Provider value={value}>{children}</ArtifactWorkbenchContext.Provider>
  );
}

export function useArtifactWorkbench(): ArtifactWorkbenchValue {
  const context = useContext(ArtifactWorkbenchContext);
  if (!context)
    throw new Error('useArtifactWorkbench must be used within ArtifactWorkbenchProvider');
  return context;
}
