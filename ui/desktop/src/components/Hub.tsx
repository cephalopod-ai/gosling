/**
 * Hub Component
 *
 * The empty-chat landing screen. Visually it's "Pair with no messages yet" —
 * a large time + greeting above a centered, narrower ChatInput. Submitting
 * creates a session and navigates to /pair so the rest of the chat lifecycle
 * lives there.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { defineMessages, useIntl } from '../i18n';
import { AppEvents } from '../constants/events';
import ChatInput from './ChatInput';
import { ChatInputCard } from './ChatInputCard';
import { ChatState } from '../types/chatState';
import 'react-toastify/dist/ReactToastify.css';
import { View, ViewOptions } from '../utils/navigationUtils';
import { useConfig } from './ConfigContext';
import { getInitialWorkingDir } from '../utils/workingDir';
import { createSession } from '../sessions';
import LoadingGosling from './LoadingGosling';
import { UserInput } from '../types/message';
import {
  createNextChatExtensionDraft,
  selectNextChatExtensions,
  type NextChatExtensionDraft,
} from '../utils/nextChatExtensions';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { reconcileWorkspaceWorkingDir } from '../utils/workspaceWorkingDir';

const i18n = defineMessages({
  goodMorning: { id: 'hub.goodMorning', defaultMessage: 'Good morning' },
  goodAfternoon: { id: 'hub.goodAfternoon', defaultMessage: 'Good afternoon' },
  goodEvening: { id: 'hub.goodEvening', defaultMessage: 'Good evening' },
});

function useClock(): { time: string; meridiem: string; hour: number } {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const interval = setInterval(() => setNow(new Date()), 30_000);
    return () => clearInterval(interval);
  }, []);

  const hour = now.getHours();
  const minutes = now.getMinutes();
  const meridiem = hour >= 12 ? 'PM' : 'AM';
  const displayHour = ((hour + 11) % 12) + 1;
  const time = `${displayHour}:${String(minutes).padStart(2, '0')}`;
  return { time, meridiem, hour };
}

export default function Hub({
  setView,
}: {
  setView: (view: View, viewOptions?: ViewOptions) => void;
}) {
  const intl = useIntl();
  const { extensionsList } = useConfig();
  const { workspaces, activeWorkspaceId, defaultWorkspaceId, loading, error } = useWorkspace();
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState(
    activeWorkspaceId ?? defaultWorkspaceId ?? ''
  );
  const selectedWorkspaceItem = useMemo(
    () => workspaces.find((item) => item.workspace.id === selectedWorkspaceId),
    [selectedWorkspaceId, workspaces]
  );
  const selectedWorkspace = selectedWorkspaceItem?.workspace;
  const workspaceStartIssue = useMemo(() => {
    if (!selectedWorkspaceItem || selectedWorkspaceItem.validation.validForSession) {
      return null;
    }

    return (
      selectedWorkspaceItem.validation.issues?.find((issue) => issue.severity === 'error')
        ?.message ??
      'This workspace cannot start a session. Relink its primary folder or credential profile.'
    );
  }, [selectedWorkspaceItem]);
  const [workingDir, setWorkingDir] = useState(
    selectedWorkspace?.workingFolder ?? getInitialWorkingDir()
  );
  const previousWorkspaceRef = useRef(
    selectedWorkspace
      ? { id: selectedWorkspace.id, workingFolder: selectedWorkspace.workingFolder }
      : undefined
  );
  const [isCreatingSession, setIsCreatingSession] = useState(false);
  const [nextChatExtensionDraft, setNextChatExtensionDraft] =
    useState<NextChatExtensionDraft | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const { time, meridiem, hour } = useClock();

  useEffect(() => {
    setSelectedWorkspaceId((current) => {
      if (workspaces.some((item) => item.workspace.id === current)) return current;
      return activeWorkspaceId ?? defaultWorkspaceId ?? workspaces[0]?.workspace.id ?? '';
    });
  }, [activeWorkspaceId, defaultWorkspaceId, workspaces]);

  useEffect(() => {
    if (selectedWorkspace) {
      const previous = previousWorkspaceRef.current;
      setWorkingDir((current) =>
        reconcileWorkspaceWorkingDir(current, previous, selectedWorkspace)
      );
      previousWorkspaceRef.current = {
        id: selectedWorkspace.id,
        workingFolder: selectedWorkspace.workingFolder,
      };
    }
  }, [selectedWorkspace]);

  const greeting = useMemo(() => {
    if (hour < 12) return intl.formatMessage(i18n.goodMorning);
    if (hour < 18) return intl.formatMessage(i18n.goodAfternoon);
    return intl.formatMessage(i18n.goodEvening);
  }, [intl, hour]);

  const draftForMenu = useMemo(
    () => nextChatExtensionDraft ?? createNextChatExtensionDraft(extensionsList),
    [extensionsList, nextChatExtensionDraft]
  );

  // rAF is more reliable than autoFocus across async render boundaries.
  useEffect(() => {
    const frameId = requestAnimationFrame(() => {
      inputRef.current?.focus();
    });
    return () => cancelAnimationFrame(frameId);
  }, []);

  const handleNextChatExtensionDraftChange = useCallback((draft: NextChatExtensionDraft) => {
    setNextChatExtensionDraft(draft);
  }, []);

  const handleSubmit = async (input: UserInput) => {
    const { msg: userMessage, images } = input;
    if (
      !(images.length > 0 || userMessage.trim()) ||
      isCreatingSession ||
      !selectedWorkspace ||
      !selectedWorkspaceItem.validation.validForSession
    ) {
      return;
    }

    setIsCreatingSession(true);

    try {
      const selectedExtensions = nextChatExtensionDraft
        ? selectNextChatExtensions(extensionsList, nextChatExtensionDraft)
        : [];
      const sessionOptions =
        selectedExtensions.length > 0
          ? { extensionConfigs: selectedExtensions, workspaceId: selectedWorkspace.id }
          : { allExtensions: extensionsList, workspaceId: selectedWorkspace.id };

      const session = await createSession(workingDir, sessionOptions);
      setNextChatExtensionDraft(null);

      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
      window.dispatchEvent(
        new CustomEvent(AppEvents.ADD_ACTIVE_SESSION, {
          detail: { sessionId: session.id, initialMessage: { msg: userMessage, images } },
        })
      );

      setView('pair', {
        disableAnimation: true,
        resumeSessionId: session.id,
        initialMessage: { msg: userMessage, images },
      });
    } catch (error) {
      console.error('Failed to create session:', error);
      setIsCreatingSession(false);
    }
  };

  return (
    <div className="flex flex-col h-full min-h-0 items-center justify-center px-6 relative">
      <div className="w-full max-w-2xl">
        <div className="flex items-baseline gap-2 mb-1">
          <span className="text-6xl font-light text-text-primary tracking-tight tabular-nums">
            {time}
          </span>
          <span className="text-2xl font-light text-text-secondary">{meridiem}</span>
        </div>
        <p className="text-xl text-text-secondary mb-6">{greeting}</p>

        <div className="mb-3 flex items-center gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-2">
          <label htmlFor="new-chat-workspace" className="text-sm font-medium text-text-primary">
            Workspace
          </label>
          <select
            id="new-chat-workspace"
            value={selectedWorkspaceId}
            onChange={(event) => setSelectedWorkspaceId(event.target.value)}
            disabled={loading || workspaces.length === 0}
            className="min-w-0 flex-1 rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {loading && <option value="">Loading workspaces…</option>}
            {!loading && workspaces.length === 0 && (
              <option value="">No workspace available</option>
            )}
            {workspaces.map((item) => (
              <option
                key={item.workspace.id}
                value={item.workspace.id}
                disabled={!item.validation.validForSession}
              >
                {item.workspace.name}
                {item.validation.validForSession ? '' : ' — needs attention'}
              </option>
            ))}
          </select>
          {selectedWorkspace && (
            <span className="max-w-56 truncate text-xs text-text-secondary" title={workingDir}>
              {workingDir}
            </span>
          )}
        </div>
        {error && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {error}
          </p>
        )}
        {workspaceStartIssue && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {workspaceStartIssue}
          </p>
        )}

        <ChatInputCard>
          <ChatInput
            sessionId={null}
            handleSubmit={handleSubmit}
            chatState={isCreatingSession ? ChatState.LoadingConversation : ChatState.Idle}
            onStop={() => {}}
            initialValue=""
            setView={setView}
            totalTokens={0}
            accumulatedInputTokens={0}
            accumulatedOutputTokens={0}
            droppedFiles={[]}
            onFilesProcessed={() => {}}
            messages={[]}
            disableAnimation={false}
            onWorkingDirChange={setWorkingDir}
            inputRef={inputRef}
            submitDisabled={Boolean(workspaceStartIssue)}
            submitDisabledReason={workspaceStartIssue ?? undefined}
            nextChatExtensionDraft={draftForMenu}
            onNextChatExtensionDraftChange={handleNextChatExtensionDraftChange}
          />
        </ChatInputCard>
      </div>

      {isCreatingSession && (
        <div className="absolute bottom-4 left-4 z-20 pointer-events-none">
          <LoadingGosling chatState={ChatState.LoadingConversation} />
        </div>
      )}
    </div>
  );
}
