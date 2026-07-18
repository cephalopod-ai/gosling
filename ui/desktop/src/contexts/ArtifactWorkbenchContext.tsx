import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import {
  artifactKindFromMimeType,
  artifactKindFromPath,
  artifactTitleFromPath,
} from '../components/artifacts/artifactUtils';
import type { ArtifactTab } from '../components/artifacts/types';

const STORAGE_KEY = 'gosling-artifact-workbench-v1';
const DEFAULT_WIDTH = 480;

interface PersistedWorkbench {
  activeTabId: string | null;
  isOpen: boolean;
  tabs: ArtifactTab[];
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
  closeTab: (id: string) => void;
  isOpen: boolean;
  openContent: (input: OpenContentInput) => void;
  openFile: (path: string, baseDirectory?: string, workspaceId?: string) => void;
  resolveFilePath: (id: string, path: string) => void;
  setActiveTabId: (id: string) => void;
  setIsOpen: (isOpen: boolean) => void;
  setWidth: (width: number) => void;
  tabs: ArtifactTab[];
  toggle: () => void;
  width: number;
}

const ArtifactWorkbenchContext = createContext<ArtifactWorkbenchValue | null>(null);

function createId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `artifact-${Date.now()}-${Math.random()}`;
}

function loadPersistedWorkbench(): PersistedWorkbench {
  try {
    const parsed = JSON.parse(
      localStorage.getItem(STORAGE_KEY) ?? '{}'
    ) as Partial<PersistedWorkbench>;
    const tabs = Array.isArray(parsed.tabs)
      ? parsed.tabs.filter((tab) => tab?.source?.type === 'file')
      : [];
    return {
      activeTabId: tabs.some((tab) => tab.id === parsed.activeTabId)
        ? (parsed.activeTabId ?? null)
        : (tabs[0]?.id ?? null),
      isOpen: parsed.isOpen === true && tabs.length > 0,
      tabs,
      width:
        typeof parsed.width === 'number'
          ? Math.min(720, Math.max(320, parsed.width))
          : DEFAULT_WIDTH,
    };
  } catch {
    return { activeTabId: null, isOpen: false, tabs: [], width: DEFAULT_WIDTH };
  }
}

export function ArtifactWorkbenchProvider({ children }: { children: React.ReactNode }) {
  const [initial] = useState(loadPersistedWorkbench);
  const [tabs, setTabs] = useState(initial.tabs);
  const [activeTabId, setActiveTabId] = useState<string | null>(initial.activeTabId);
  const [isOpen, setIsOpen] = useState(initial.isOpen);
  const [width, setWidthState] = useState(initial.width);

  useEffect(() => {
    const persisted: PersistedWorkbench = {
      activeTabId,
      isOpen,
      tabs: tabs.filter((tab) => tab.source.type === 'file'),
      width,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(persisted));
  }, [activeTabId, isOpen, tabs, width]);

  const openFile = useCallback((path: string, baseDirectory?: string, workspaceId?: string) => {
    setTabs((current) => {
      const existing = current.find(
        (tab) =>
          tab.source.type === 'file' &&
          tab.source.path === path &&
          tab.source.baseDirectory === baseDirectory &&
          tab.workspaceId === workspaceId
      );
      if (existing) {
        setActiveTabId(existing.id);
        return current;
      }
      const tab: ArtifactTab = {
        id: createId(),
        kind: artifactKindFromPath(path),
        source: { type: 'file', path, baseDirectory },
        title: artifactTitleFromPath(path),
        workspaceId,
      };
      setActiveTabId(tab.id);
      return [...current, tab];
    });
    setIsOpen(true);
  }, []);

  const openContent = useCallback((input: OpenContentInput) => {
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
    setTabs((current) => [...current, tab]);
    setActiveTabId(tab.id);
    setIsOpen(true);
  }, []);

  const closeTab = useCallback((id: string) => {
    setTabs((current) => {
      const index = current.findIndex((tab) => tab.id === id);
      const next = current.filter((tab) => tab.id !== id);
      setActiveTabId((active) =>
        active === id ? (next[Math.min(index, next.length - 1)]?.id ?? null) : active
      );
      if (next.length === 0) setIsOpen(false);
      return next;
    });
  }, []);

  const resolveFilePath = useCallback((id: string, path: string) => {
    setTabs((current) =>
      current.map((tab) =>
        tab.id === id && tab.source.type === 'file'
          ? {
              ...tab,
              kind: artifactKindFromPath(path),
              source: { type: 'file', path },
              title: artifactTitleFromPath(path),
            }
          : tab
      )
    );
  }, []);

  const setWidth = useCallback((nextWidth: number) => {
    setWidthState(Math.min(720, Math.max(320, nextWidth)));
  }, []);

  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? null;
  const value = useMemo<ArtifactWorkbenchValue>(
    () => ({
      activeTab,
      activeTabId,
      closeTab,
      isOpen,
      openContent,
      openFile,
      resolveFilePath,
      setActiveTabId,
      setIsOpen,
      setWidth,
      tabs,
      toggle: () => setIsOpen((current) => !current),
      width,
    }),
    [
      activeTab,
      activeTabId,
      closeTab,
      isOpen,
      openContent,
      openFile,
      resolveFilePath,
      setWidth,
      tabs,
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
