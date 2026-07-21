import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Check, FolderDot, FolderOpen, GitBranch, Plus } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/Tooltip';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { toast } from 'react-toastify';
import { defineMessages, useIntl } from '../../i18n';

const i18n = defineMessages({
  failedToUpdateWorkingDir: {
    id: 'dirSwitcher.failedToUpdateWorkingDir',
    defaultMessage: 'Failed to update working directory',
  },
  currentDirectory: {
    id: 'dirSwitcher.currentDirectory',
    defaultMessage: 'Current directory',
  },
  gitWorktrees: {
    id: 'dirSwitcher.gitWorktrees',
    defaultMessage: 'Git worktrees',
  },
  recentDirectories: {
    id: 'dirSwitcher.recentDirectories',
    defaultMessage: 'Recent directories',
  },
  chooseDirectory: {
    id: 'dirSwitcher.chooseDirectory',
    defaultMessage: 'Choose directory…',
  },
  openInFinder: {
    id: 'dirSwitcher.openInFinder',
    defaultMessage: 'Open in file manager',
  },
  noWorktreesFound: {
    id: 'dirSwitcher.noWorktreesFound',
    defaultMessage: 'No worktrees found',
  },
});

interface DirSwitcherProps {
  className: string;
  sessionId: string | undefined;
  workingDir: string;
  onWorkingDirChange?: (newDir: string) => Promise<void> | void;
  renderChatInfo?: (close: () => void) => React.ReactNode;
  onRestartStart?: () => void;
  onRestartEnd?: () => void;
}

export const DirSwitcher: React.FC<DirSwitcherProps> = ({
  className,
  sessionId,
  workingDir,
  onWorkingDirChange,
  renderChatInfo,
  onRestartStart,
  onRestartEnd,
}) => {
  const intl = useIntl();
  const [isTooltipOpen, setIsTooltipOpen] = useState(false);
  const [isDirectoryChooserOpen, setIsDirectoryChooserOpen] = useState(false);
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [recentDirs, setRecentDirs] = useState<string[]>([]);
  const [worktreeDirs, setWorktreeDirs] = useState<string[]>([]);
  const refreshVersionRef = useRef(0);
  const chatInfoTriggerRef = useRef<HTMLButtonElement>(null);
  const chatInfoPanelRef = useRef<HTMLDivElement>(null);
  const [chatInfoPosition, setChatInfoPosition] = useState<{
    left: number;
    bottom: number;
  } | null>(null);

  const updateChatInfoPosition = useCallback(() => {
    const trigger = chatInfoTriggerRef.current;
    if (!trigger) return;

    const bounds = trigger.getBoundingClientRect();
    setChatInfoPosition({
      left: Math.max(16, Math.min(bounds.left, window.innerWidth - 356)),
      bottom: window.innerHeight - bounds.top + 4,
    });
  }, []);

  const refreshMenuData = useCallback(async () => {
    const version = ++refreshVersionRef.current;
    setRecentDirs([]);
    setWorktreeDirs([]);

    const [recent, worktrees] = await Promise.all([
      window.electron.listRecentDirs().catch(() => []),
      window.electron.listGitWorktreeDirs(workingDir).catch(() => []),
    ]);

    if (version !== refreshVersionRef.current) return;

    setRecentDirs(recent);
    setWorktreeDirs(worktrees);
  }, [workingDir]);

  useEffect(() => {
    if (!isMenuOpen || renderChatInfo) {
      return;
    }

    void refreshMenuData();
  }, [isMenuOpen, refreshMenuData, renderChatInfo]);

  useEffect(() => {
    if (!isMenuOpen || !renderChatInfo) return;

    updateChatInfoPosition();

    const dismissOnPointerDown = (event: globalThis.PointerEvent) => {
      const target = event.target;
      if (!(target instanceof window.Node)) return;
      if (
        !chatInfoTriggerRef.current?.contains(target) &&
        !chatInfoPanelRef.current?.contains(target)
      ) {
        setIsMenuOpen(false);
      }
    };
    const dismissOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') setIsMenuOpen(false);
    };

    window.document.addEventListener('pointerdown', dismissOnPointerDown);
    window.document.addEventListener('keydown', dismissOnEscape);
    window.addEventListener('resize', updateChatInfoPosition);
    window.addEventListener('scroll', updateChatInfoPosition, true);

    return () => {
      window.document.removeEventListener('pointerdown', dismissOnPointerDown);
      window.document.removeEventListener('keydown', dismissOnEscape);
      window.removeEventListener('resize', updateChatInfoPosition);
      window.removeEventListener('scroll', updateChatInfoPosition, true);
    };
  }, [isMenuOpen, renderChatInfo, updateChatInfoPosition]);

  const applyDirectoryChange = async (newDir: string) => {
    window.electron.addRecentDir(newDir);
    setRecentDirs((previous) => [newDir, ...previous.filter((dir) => dir !== newDir)].slice(0, 10));

    if (sessionId) {
      onRestartStart?.();

      try {
        await onWorkingDirChange?.(newDir);
      } catch (error) {
        console.error('[DirSwitcher] Failed to update working directory:', error);
        toast.error(intl.formatMessage(i18n.failedToUpdateWorkingDir));
      } finally {
        onRestartEnd?.();
      }
    } else {
      await onWorkingDirChange?.(newDir);
    }
  };

  const handleDirectoryChange = async () => {
    if (isDirectoryChooserOpen) return;
    setIsDirectoryChooserOpen(true);

    let result;
    try {
      result = await window.electron.directoryChooser();
    } finally {
      setIsDirectoryChooserOpen(false);
    }

    if (result.canceled || result.filePaths.length === 0) {
      return;
    }

    const newDir = result.filePaths[0];
    await applyDirectoryChange(newDir);
  };

  const handleSelectDirectory = async (newDir: string) => {
    if (newDir === workingDir) {
      setIsMenuOpen(false);
      return;
    }

    setIsMenuOpen(false);
    await applyDirectoryChange(newDir);
  };

  const handleDirectoryClick = async (event: React.MouseEvent) => {
    if (isDirectoryChooserOpen) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }

    const isCmdOrCtrlClick = event.metaKey || event.ctrlKey;

    if (isCmdOrCtrlClick) {
      event.preventDefault();
      event.stopPropagation();
      await window.electron.openDirectoryInExplorer(workingDir);
    }
  };

  const filteredWorktreeDirs = useMemo(
    () => worktreeDirs.filter((dir) => dir && dir !== workingDir),
    [worktreeDirs, workingDir]
  );

  const filteredRecentDirs = useMemo(
    () => recentDirs.filter((dir) => dir && dir !== workingDir),
    [recentDirs, workingDir]
  );

  if (renderChatInfo) {
    return (
      <TooltipProvider>
        <Tooltip
          open={isTooltipOpen && !isDirectoryChooserOpen && !isMenuOpen}
          onOpenChange={(open) => {
            if (!isDirectoryChooserOpen && !isMenuOpen) setIsTooltipOpen(open);
          }}
        >
          <TooltipTrigger asChild>
            <button
              ref={chatInfoTriggerRef}
              type="button"
              className={`z-[100] ${isDirectoryChooserOpen ? 'opacity-50' : 'hover:cursor-pointer hover:text-text-primary'} text-text-primary/70 text-xs flex items-center transition-colors pl-1 [&>svg]:size-4 ${className}`}
              onClick={(event) => {
                if (event.metaKey || event.ctrlKey || isDirectoryChooserOpen) {
                  void handleDirectoryClick(event);
                  return;
                }
                setIsMenuOpen((open) => !open);
              }}
              disabled={isDirectoryChooserOpen}
              aria-label={`Open chat information for ${workingDir.replace(/\/+$/, '').split('/').pop() || workingDir}`}
              aria-expanded={isMenuOpen}
              aria-controls={isMenuOpen ? 'chat-info-panel' : undefined}
            >
              <FolderDot className="mr-1" size={16} />
              <div className="max-w-[200px] truncate">
                {workingDir.replace(/\/+$/, '').split('/').pop() || workingDir}
              </div>
            </button>
          </TooltipTrigger>
          <TooltipContent side="top">Chat info · {workingDir}</TooltipContent>
        </Tooltip>
        {isMenuOpen &&
          chatInfoPosition &&
          createPortal(
            <div
              id="chat-info-panel"
              ref={chatInfoPanelRef}
              className="fixed z-[200] w-max max-w-[calc(100vw-2rem)]"
              style={{ left: chatInfoPosition.left, bottom: chatInfoPosition.bottom }}
            >
              {renderChatInfo(() => setIsMenuOpen(false))}
            </div>,
            window.document.body
          )}
      </TooltipProvider>
    );
  }

  return (
    <TooltipProvider>
      <Tooltip
        open={isTooltipOpen && !isDirectoryChooserOpen && !isMenuOpen}
        onOpenChange={(open) => {
          if (!isDirectoryChooserOpen && !isMenuOpen) setIsTooltipOpen(open);
        }}
      >
        <DropdownMenu open={isMenuOpen} onOpenChange={setIsMenuOpen} modal={false}>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <button
                className={`z-[100] ${isDirectoryChooserOpen ? 'opacity-50' : 'hover:cursor-pointer hover:text-text-primary'} text-text-primary/70 text-xs flex items-center transition-colors pl-1 [&>svg]:size-4 ${className}`}
                onClick={handleDirectoryClick}
                disabled={isDirectoryChooserOpen}
                aria-label={
                  renderChatInfo
                    ? `Open chat information for ${workingDir.replace(/\/+$/, '').split('/').pop() || workingDir}`
                    : undefined
                }
              >
                <FolderDot className="mr-1" size={16} />
                <div className="max-w-[200px] truncate">
                  {workingDir.replace(/\/+$/, '').split('/').pop() || workingDir}
                </div>
              </button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <DropdownMenuContent
            className={
              renderChatInfo
                ? 'z-[200] w-auto border-0 bg-transparent p-0 shadow-none'
                : 'w-80'
            }
            side="top"
            align="start"
          >
            {renderChatInfo ? (
              renderChatInfo(() => setIsMenuOpen(false))
            ) : (
              <>
                <DropdownMenuLabel>{intl.formatMessage(i18n.currentDirectory)}</DropdownMenuLabel>
            <DropdownMenuItem
              onSelect={() => void window.electron.openDirectoryInExplorer(workingDir)}
            >
              <FolderOpen className="mr-2 h-4 w-4" />
              <span className="truncate">{workingDir}</span>
              <Check className="ml-auto h-4 w-4" />
            </DropdownMenuItem>

            <DropdownMenuSeparator />
            <DropdownMenuLabel>{intl.formatMessage(i18n.gitWorktrees)}</DropdownMenuLabel>
            {filteredWorktreeDirs.length > 0 ? (
              filteredWorktreeDirs.map((dir) => (
                <DropdownMenuItem
                  key={`worktree-${dir}`}
                  onSelect={() => void handleSelectDirectory(dir)}
                >
                  <GitBranch className="mr-2 h-4 w-4" />
                  <span className="truncate">{dir}</span>
                </DropdownMenuItem>
              ))
            ) : (
              <DropdownMenuItem disabled>
                <GitBranch className="mr-2 h-4 w-4" />
                <span>{intl.formatMessage(i18n.noWorktreesFound)}</span>
              </DropdownMenuItem>
            )}

            {filteredRecentDirs.length > 0 && (
              <>
                <DropdownMenuSeparator />
                <DropdownMenuLabel>{intl.formatMessage(i18n.recentDirectories)}</DropdownMenuLabel>
                {filteredRecentDirs.map((dir) => (
                  <DropdownMenuItem
                    key={`recent-${dir}`}
                    onSelect={() => void handleSelectDirectory(dir)}
                  >
                    <FolderDot className="mr-2 h-4 w-4" />
                    <span className="truncate">{dir}</span>
                  </DropdownMenuItem>
                ))}
              </>
            )}

            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => void handleDirectoryChange()}>
              <Plus className="mr-2 h-4 w-4" />
              <span>{intl.formatMessage(i18n.chooseDirectory)}</span>
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() => void window.electron.openDirectoryInExplorer(workingDir)}
            >
              <FolderOpen className="mr-2 h-4 w-4" />
              <span>{intl.formatMessage(i18n.openInFinder)}</span>
            </DropdownMenuItem>
              </>
            )}
          </DropdownMenuContent>
        </DropdownMenu>
        <TooltipContent side="top">
          {renderChatInfo ? `Chat info · ${workingDir}` : workingDir}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
};
