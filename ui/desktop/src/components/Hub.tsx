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
  selectResearchSessionExtensions,
  selectNextChatExtensions,
  type NextChatExtensionDraft,
} from '../utils/nextChatExtensions';
import { useWorkspace } from '../contexts/WorkspaceContext';
import { reconcileWorkspaceWorkingDir } from '../utils/workspaceWorkingDir';
import { useModelAndProvider } from './ModelAndProviderContext';
import { BookOpen, FolderOpen, Telescope } from 'lucide-react';
import {
  researchInitialInputCount,
  type ResearchInitialInputs,
  type ResearchTeamConfiguration,
  type SessionExperience,
} from '../types/sessionExperience';
import { ResearchInitialInputsDialog } from './research/ResearchInitialInputsDialog';
import { ResearchModelTeamSelector } from './research/ResearchModelTeamSelector';
import { addResearchInitialInputs, resolveSessionLibraryInputs } from '../acp/sessionLibraryInputs';
import { acpAppendSessionSystemPrompt, acpDeleteSession } from '../acp/sessions';
import {
  buildResearchScientificMethodPrompt,
  RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
} from '../prompts/researchScientificMethod';
import {
  buildResearchModelTeamPrompt,
  RESEARCH_MODEL_TEAM_PROMPT_KEY,
} from '../prompts/researchModelTeam';
import {
  buildResearchLibraryPrompt,
  RESEARCH_LIBRARY_PROMPT_KEY,
} from '../prompts/researchLibrary';

const i18n = defineMessages({
  goodMorning: { id: 'hub.goodMorning', defaultMessage: 'Good morning' },
  goodAfternoon: { id: 'hub.goodAfternoon', defaultMessage: 'Good afternoon' },
  goodEvening: { id: 'hub.goodEvening', defaultMessage: 'Good evening' },
  researchTitle: { id: 'hub.researchTitle', defaultMessage: 'Deep Research' },
  researchDescription: {
    id: 'hub.researchDescription',
    defaultMessage:
      'Start a focused research session with reports, links, notes, or a starting prompt.',
  },
  researchSessionTag: {
    id: 'hub.researchSessionTag',
    defaultMessage: 'Research session',
  },
  beginResearchWithInputs: {
    id: 'hub.beginResearchWithInputs',
    defaultMessage: 'Begin the research using the initial inputs I provided.',
  },
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

function sameResearchModel(
  left: ResearchTeamConfiguration['models'][number],
  right: ResearchTeamConfiguration['models'][number]
): boolean {
  return left.provider === right.provider && left.model === right.model;
}

export function replaceResearchLead(
  configuration: ResearchTeamConfiguration,
  nextLead: ResearchTeamConfiguration['models'][number]
): ResearchTeamConfiguration {
  const models = [...configuration.models];
  const previousLead = models[0];
  const previousSeat = models.findIndex(
    (model, index) => index > 0 && sameResearchModel(model, nextLead)
  );

  if (previousSeat > 0 && previousLead) {
    models[previousSeat] = previousLead;
  }
  models[0] = nextLead;
  return { ...configuration, models };
}

export default function Hub({
  setView,
  initialMessage,
  initialWorkspaceId,
  sessionExperience = 'chat',
}: {
  setView: (view: View, viewOptions?: ViewOptions) => void;
  initialMessage?: UserInput;
  initialWorkspaceId?: string;
  sessionExperience?: SessionExperience;
}) {
  const intl = useIntl();
  const { extensionsList } = useConfig();
  const { currentModel, currentProvider } = useModelAndProvider();
  const { workspaces, activeWorkspaceId, defaultWorkspaceId, credentialProfiles, loading, error } =
    useWorkspace();
  const preferredWorkspaceId = useMemo(() => {
    const availableWorkspaceIds = new Set(workspaces.map((item) => item.workspace.id));
    const configuredWorkspaceId = [initialWorkspaceId, activeWorkspaceId, defaultWorkspaceId].find(
      (workspaceId) => workspaceId && availableWorkspaceIds.has(workspaceId)
    );
    if (configuredWorkspaceId) return configuredWorkspaceId;

    return workspaces.reduce<(typeof workspaces)[number] | undefined>((latest, item) => {
      if (!latest) return item;
      return Date.parse(item.workspace.createdAt) > Date.parse(latest.workspace.createdAt)
        ? item
        : latest;
    }, undefined)?.workspace.id;
  }, [activeWorkspaceId, defaultWorkspaceId, initialWorkspaceId, workspaces]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState(preferredWorkspaceId ?? '');
  const [selectedCredentialProfileId, setSelectedCredentialProfileId] = useState('');
  const [additionalWorkspaceFolders, setAdditionalWorkspaceFolders] = useState<string[]>([]);
  const [isChoosingAdditionalFolder, setIsChoosingAdditionalFolder] = useState(false);
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
  const workspaceSelectionRequired = !loading && !selectedWorkspace;
  const isResearch = sessionExperience === 'research';
  const [researchTeamConfiguration, setResearchTeamConfiguration] =
    useState<ResearchTeamConfiguration>({ mode: 'solo', models: [] });
  const [researchTeamIssue, setResearchTeamIssue] = useState<string | null>(null);
  const researchLead = isResearch ? researchTeamConfiguration.models[0] : undefined;
  const handleResearchComposerModelChanged = useCallback(
    (selection: { model: string; provider: string }) => {
      if (!isResearch) return;
      setResearchTeamConfiguration((configuration) =>
        replaceResearchLead(configuration, selection)
      );
    },
    [isResearch]
  );
  const [workingDir, setWorkingDir] = useState(
    selectedWorkspace?.workingFolder ?? getInitialWorkingDir()
  );
  const previousWorkspaceRef = useRef(
    selectedWorkspace
      ? { id: selectedWorkspace.id, workingFolder: selectedWorkspace.workingFolder }
      : undefined
  );
  const [isCreatingSession, setIsCreatingSession] = useState(false);
  const [sessionCreationError, setSessionCreationError] = useState<string | null>(null);
  const [researchInitialInputs, setResearchInitialInputs] = useState<ResearchInitialInputs>({
    texts: [],
    files: [],
  });
  const [nextChatExtensionDraft, setNextChatExtensionDraft] =
    useState<NextChatExtensionDraft | null>(null);
  const [researchLibraryPath, setResearchLibraryPath] = useState<string | null>(null);
  const [researchLibraryError, setResearchLibraryError] = useState<string | null>(null);
  const [isChoosingResearchLibrary, setIsChoosingResearchLibrary] = useState(false);
  const researchSessionExtensions = useMemo(
    () =>
      isResearch
        ? selectResearchSessionExtensions(
            extensionsList,
            nextChatExtensionDraft,
            researchTeamConfiguration.mode
          )
        : null,
    [extensionsList, isResearch, nextChatExtensionDraft, researchTeamConfiguration.mode]
  );
  const researchExtensionIssue = researchSessionExtensions?.missingRequiredNames.length
    ? `Deep Research requires configured extension${researchSessionExtensions.missingRequiredNames.length === 1 ? '' : 's'}: ${researchSessionExtensions.missingRequiredNames.join(', ')}.`
    : null;
  const researchLibraryIssue = isResearch
    ? (researchLibraryError ?? (!researchLibraryPath ? 'Research Library is loading.' : null))
    : null;
  const submitDisabledReason =
    workspaceStartIssue ??
    researchExtensionIssue ??
    researchLibraryIssue ??
    (isResearch ? researchTeamIssue : null) ??
    (workspaceSelectionRequired
      ? `Choose a workspace before starting ${isResearch ? 'research' : 'a chat'}.`
      : undefined);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const { time, meridiem, hour } = useClock();

  useEffect(() => {
    if (!isResearch) return;
    let cancelled = false;
    setResearchLibraryError(null);
    void window.electron
      .getResearchLibraryPath()
      .then((libraryPath) => {
        if (!cancelled) setResearchLibraryPath(libraryPath);
      })
      .catch((error) => {
        if (!cancelled) {
          setResearchLibraryError(
            `Research Library is unavailable: ${error instanceof Error ? error.message : String(error)}`
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [isResearch]);

  useEffect(() => {
    setSelectedWorkspaceId((current) => {
      if (current && workspaces.some((item) => item.workspace.id === current)) return current;
      return preferredWorkspaceId ?? '';
    });
  }, [preferredWorkspaceId, workspaces]);

  const handleWorkspaceChange = useCallback(
    (workspaceId: string) => {
      const workspace = workspaces.find((item) => item.workspace.id === workspaceId)?.workspace;
      setSelectedWorkspaceId(workspaceId);
      setSelectedCredentialProfileId('');
      setAdditionalWorkspaceFolders([]);
      if (workspace) {
        setWorkingDir(workspace.workingFolder);
      }
    },
    [workspaces]
  );

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
    const frameId = window.requestAnimationFrame(() => {
      inputRef.current?.focus();
    });
    return () => window.cancelAnimationFrame(frameId);
  }, []);

  const handleNextChatExtensionDraftChange = useCallback((draft: NextChatExtensionDraft) => {
    setNextChatExtensionDraft(draft);
  }, []);

  const handleWorkingDirChange = useCallback((directory: string) => {
    setWorkingDir(directory);
    setAdditionalWorkspaceFolders((folders) => folders.filter((folder) => folder !== directory));
  }, []);

  const addAdditionalWorkspaceFolder = useCallback(async () => {
    if (isChoosingAdditionalFolder) return;
    setIsChoosingAdditionalFolder(true);
    try {
      const result = await window.electron.sessionDirectoryChooser();
      const folder = result.canceled ? undefined : result.filePaths[0];
      if (!folder || folder === workingDir) return;
      setAdditionalWorkspaceFolders((folders) =>
        folders.includes(folder) ? folders : [...folders, folder]
      );
    } finally {
      setIsChoosingAdditionalFolder(false);
    }
  }, [isChoosingAdditionalFolder, workingDir]);

  const chooseResearchLibrary = useCallback(async () => {
    if (isChoosingResearchLibrary) return;
    setIsChoosingResearchLibrary(true);
    setResearchLibraryError(null);
    try {
      const libraryPath = await window.electron.chooseResearchLibraryPath();
      if (libraryPath) setResearchLibraryPath(libraryPath);
    } catch (error) {
      setResearchLibraryError(
        `Could not change the Research Library: ${error instanceof Error ? error.message : String(error)}`
      );
    } finally {
      setIsChoosingResearchLibrary(false);
    }
  }, [isChoosingResearchLibrary]);

  const handleSubmit = async (input: UserInput) => {
    const { msg: userMessage, images } = input;
    const hasResearchInitialInputs =
      isResearch && researchInitialInputCount(researchInitialInputs) > 0;
    if (
      !(images.length > 0 || userMessage.trim() || hasResearchInitialInputs) ||
      isCreatingSession ||
      (isResearch && (researchTeamIssue || researchExtensionIssue || researchLibraryIssue)) ||
      !selectedWorkspace ||
      !selectedWorkspaceItem.validation.validForSession
    ) {
      return false;
    }

    setSessionCreationError(null);
    setIsCreatingSession(true);
    let createdSessionId: string | undefined;

    try {
      const selectedExtensions = nextChatExtensionDraft
        ? selectNextChatExtensions(extensionsList, nextChatExtensionDraft)
        : [];
      const sessionExtensions = isResearch
        ? (researchSessionExtensions?.extensionConfigs ?? [])
        : selectedExtensions;
      const enabledResearchExtensionNames = sessionExtensions.map((extension) => extension.name);
      const hasInitialInputs = hasResearchInitialInputs || images.length > 0;
      const researchTeamPrompt = isResearch
        ? buildResearchModelTeamPrompt(
            researchTeamConfiguration,
            enabledResearchExtensionNames,
            hasInitialInputs
          )
        : null;
      const selectedResearchLead = isResearch ? researchTeamConfiguration.models[0] : undefined;
      const sessionProvider = selectedResearchLead?.provider ?? currentProvider;
      const sessionModel = selectedResearchLead?.model ?? currentModel;
      const sessionAdditionalFolders = [
        ...new Set([
          ...additionalWorkspaceFolders,
          ...(isResearch && researchLibraryPath ? [researchLibraryPath] : []),
        ]),
      ];
      const sessionOptions =
        sessionExtensions.length > 0
          ? {
              extensionConfigs: sessionExtensions,
              workspaceId: selectedWorkspace.id,
              workspaceWorkingDir: workingDir,
              ...(sessionProvider ? { provider: sessionProvider } : {}),
              ...(sessionModel ? { model: sessionModel } : {}),
              ...(selectedCredentialProfileId
                ? { workspaceCredentialProfileId: selectedCredentialProfileId }
                : {}),
              ...(sessionAdditionalFolders.length
                ? { workspaceAdditionalFolders: sessionAdditionalFolders }
                : {}),
              ...(researchLibraryPath ? { researchLibraryPath } : {}),
            }
          : {
              allExtensions: extensionsList,
              workspaceId: selectedWorkspace.id,
              workspaceWorkingDir: workingDir,
              ...(sessionProvider ? { provider: sessionProvider } : {}),
              ...(sessionModel ? { model: sessionModel } : {}),
              ...(selectedCredentialProfileId
                ? { workspaceCredentialProfileId: selectedCredentialProfileId }
                : {}),
              ...(sessionAdditionalFolders.length
                ? { workspaceAdditionalFolders: sessionAdditionalFolders }
                : {}),
              ...(researchLibraryPath ? { researchLibraryPath } : {}),
            };

      const session = await createSession(workingDir, sessionOptions);
      createdSessionId = session.id;
      if (isResearch) {
        await acpAppendSessionSystemPrompt(
          session.id,
          RESEARCH_SCIENTIFIC_METHOD_PROMPT_KEY,
          buildResearchScientificMethodPrompt(hasInitialInputs)
        );
        if (researchLibraryPath) {
          await acpAppendSessionSystemPrompt(
            session.id,
            RESEARCH_LIBRARY_PROMPT_KEY,
            buildResearchLibraryPrompt(researchLibraryPath)
          );
        }
        if (researchTeamPrompt) {
          await acpAppendSessionSystemPrompt(
            session.id,
            RESEARCH_MODEL_TEAM_PROMPT_KEY,
            researchTeamPrompt
          );
        }
      }
      const libraryItemIds = hasResearchInitialInputs
        ? await addResearchInitialInputs(session.id, researchInitialInputs)
        : [];
      const resolvedLibraryInputs = await resolveSessionLibraryInputs(session.id, libraryItemIds);
      const initialUserInput: UserInput = {
        msg:
          userMessage.trim() ||
          (hasResearchInitialInputs ? intl.formatMessage(i18n.beginResearchWithInputs) : ''),
        images: [...images, ...resolvedLibraryInputs.images],
        ...(resolvedLibraryInputs.assistantContext
          ? { assistantContext: resolvedLibraryInputs.assistantContext }
          : {}),
      };
      setNextChatExtensionDraft(null);

      window.dispatchEvent(new CustomEvent(AppEvents.SESSION_CREATED));
      window.dispatchEvent(
        new CustomEvent(AppEvents.ADD_ACTIVE_SESSION, {
          detail: {
            sessionId: session.id,
            initialMessage: initialUserInput,
            sessionExperience,
          },
        })
      );

      setView('pair', {
        disableAnimation: true,
        resumeSessionId: session.id,
        initialMessage: initialUserInput,
        sessionExperience,
      });
      return true;
    } catch (error) {
      let cleanupFailure: string | null = null;
      if (createdSessionId) {
        try {
          await acpDeleteSession(createdSessionId);
        } catch (cleanupError) {
          console.error('Failed to remove incomplete research session:', cleanupError);
          cleanupFailure =
            cleanupError instanceof Error ? cleanupError.message : String(cleanupError);
        }
      }
      console.error('Failed to create session:', error);
      const detail = error instanceof Error ? error.message : String(error);
      setSessionCreationError(
        `Could not start the ${isResearch ? 'research session' : 'chat'}: ${detail}${
          cleanupFailure
            ? ` The incomplete session could not be removed: ${cleanupFailure}. Open Session History and remove it manually.`
            : ''
        }`
      );
      setIsCreatingSession(false);
      return false;
    }
  };

  return (
    <div
      className={`flex h-full min-h-0 flex-col items-center px-6 relative ${
        isResearch ? 'justify-start overflow-y-auto py-6' : 'justify-center'
      }`}
      data-session-experience={sessionExperience}
    >
      <div className={`w-full max-w-2xl ${isResearch ? 'my-auto' : ''}`}>
        <div className="flex items-baseline gap-2 mb-1">
          <span className="text-6xl font-light text-text-primary tracking-tight tabular-nums">
            {time}
          </span>
          <span className="text-2xl font-light text-text-secondary">{meridiem}</span>
        </div>
        <p className="text-xl text-text-secondary mb-6">{greeting}</p>

        {isResearch && (
          <section
            className="mb-4 rounded-xl border border-border-primary bg-background-secondary px-4 py-3"
            aria-labelledby="deep-research-title"
            data-research-intake="initial-inputs"
          >
            <div className="flex items-center gap-2">
              <Telescope className="h-5 w-5 text-text-primary" />
              <h1 id="deep-research-title" className="text-base font-medium text-text-primary">
                {intl.formatMessage(i18n.researchTitle)}
              </h1>
              <span className="rounded-full border border-border-primary px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide text-text-secondary">
                {intl.formatMessage(i18n.researchSessionTag)}
              </span>
            </div>
            <p className="mt-1 text-xs text-text-secondary">
              {intl.formatMessage(i18n.researchDescription)}
            </p>
            <ResearchInitialInputsDialog
              value={researchInitialInputs}
              onApply={setResearchInitialInputs}
            />
          </section>
        )}

        {isResearch && (
          <ResearchModelTeamSelector
            currentModel={currentModel}
            currentProvider={currentProvider}
            value={researchTeamConfiguration}
            onChange={setResearchTeamConfiguration}
            onValidationChange={setResearchTeamIssue}
          />
        )}

        {isResearch && (
          <section className="mb-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-3">
            <div className="flex items-center gap-2">
              <BookOpen className="h-4 w-4 text-text-secondary" />
              <div className="min-w-0 flex-1">
                <h2 className="text-sm font-medium text-text-primary">Research Library</h2>
                <p
                  className="mt-0.5 truncate text-xs text-text-secondary"
                  title={researchLibraryPath ?? undefined}
                >
                  {researchLibraryPath ?? 'Preparing the default Documents library…'}
                </p>
              </div>
              {researchLibraryPath && (
                <button
                  type="button"
                  className="rounded-md border border-border-primary px-2 py-1 text-xs text-text-primary hover:bg-background-primary"
                  onClick={() => void window.electron.openDirectoryInExplorer(researchLibraryPath)}
                >
                  <FolderOpen className="mr-1 inline h-3.5 w-3.5" />
                  Open
                </button>
              )}
              <button
                type="button"
                className="rounded-md border border-border-primary px-2 py-1 text-xs text-text-primary hover:bg-background-primary disabled:opacity-50"
                onClick={() => void chooseResearchLibrary()}
                disabled={isChoosingResearchLibrary}
              >
                {isChoosingResearchLibrary ? 'Choosing…' : 'Change'}
              </button>
            </div>
            <p className="mt-2 text-xs text-text-secondary">
              Finished reports and tutorials are saved here. Prior reports are optional context, not
              authoritative evidence.
            </p>
          </section>
        )}

        <div className="mb-3 flex items-center gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-2">
          <label
            htmlFor={`new-${sessionExperience}-workspace`}
            className="text-sm font-medium text-text-primary"
          >
            Workspace
          </label>
          <select
            id={`new-${sessionExperience}-workspace`}
            value={selectedWorkspaceId}
            onChange={(event) => handleWorkspaceChange(event.target.value)}
            disabled={loading || workspaces.length === 0}
            className="min-w-0 flex-1 rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {loading && <option value="">Loading workspaces…</option>}
            {!loading && workspaces.length === 0 && (
              <option value="">No workspace available</option>
            )}
            {!loading && workspaces.length > 0 && <option value="">Choose a workspace…</option>}
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
        {selectedWorkspace && (
          <div className="mb-3 grid gap-3 rounded-lg border border-border-primary bg-background-secondary px-3 py-3 sm:grid-cols-2">
            <div className="min-w-0">
              <label
                htmlFor={`new-${sessionExperience}-credential-profile`}
                className="mb-1 block text-sm font-medium text-text-primary"
              >
                Credential
              </label>
              <select
                id={`new-${sessionExperience}-credential-profile`}
                value={selectedCredentialProfileId}
                onChange={(event) => setSelectedCredentialProfileId(event.target.value)}
                className="w-full rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="">Use workspace default</option>
                {credentialProfiles.map((profile) => (
                  <option
                    key={profile.id}
                    value={profile.id}
                    disabled={profile.status !== 'configured'}
                  >
                    {profile.name}
                    {profile.status === 'configured' ? '' : ' — needs attention'}
                  </option>
                ))}
              </select>
              <p className="mt-1 text-xs text-text-secondary">
                This choice applies only to this new chat.
              </p>
            </div>
            <div className="min-w-0">
              <div className="mb-1 flex items-center justify-between gap-2">
                <span className="text-sm font-medium text-text-primary">Additional folders</span>
                <button
                  type="button"
                  onClick={() => void addAdditionalWorkspaceFolder()}
                  disabled={isChoosingAdditionalFolder}
                  className="rounded-md border border-border-primary px-2 py-1 text-xs text-text-primary hover:bg-background-primary disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {isChoosingAdditionalFolder ? 'Choosing…' : 'Add folder'}
                </button>
              </div>
              {additionalWorkspaceFolders.length === 0 ? (
                <p className="text-xs text-text-secondary">No additional session folders.</p>
              ) : (
                <ul className="space-y-1">
                  {additionalWorkspaceFolders.map((folder) => (
                    <li
                      key={folder}
                      className="flex items-center gap-2 text-xs text-text-secondary"
                    >
                      <span className="min-w-0 flex-1 truncate" title={folder}>
                        {folder}
                      </span>
                      <button
                        type="button"
                        aria-label={`Remove additional folder ${folder}`}
                        onClick={() =>
                          setAdditionalWorkspaceFolders((folders) =>
                            folders.filter((item) => item !== folder)
                          )
                        }
                        className="text-text-secondary hover:text-text-primary"
                      >
                        Remove
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </div>
        )}
        {error && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {error}
          </p>
        )}
        {sessionCreationError && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {sessionCreationError}
          </p>
        )}
        {workspaceStartIssue && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {workspaceStartIssue}
          </p>
        )}
        {researchExtensionIssue && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {researchExtensionIssue}
          </p>
        )}
        {researchLibraryError && (
          <p role="alert" className="mb-3 text-sm text-red-600">
            {researchLibraryError}
          </p>
        )}

        <ChatInputCard>
          <ChatInput
            sessionId={null}
            handleSubmit={handleSubmit}
            chatState={isCreatingSession ? ChatState.LoadingConversation : ChatState.Idle}
            onStop={() => {}}
            initialValue={initialMessage?.msg ?? ''}
            setView={setView}
            totalTokens={0}
            accumulatedInputTokens={0}
            accumulatedOutputTokens={0}
            droppedFiles={[]}
            onFilesProcessed={() => {}}
            messages={[]}
            disableAnimation={false}
            workingDir={workingDir}
            onWorkingDirChange={handleWorkingDirChange}
            inputRef={inputRef}
            sessionModel={researchLead?.model}
            sessionProvider={researchLead?.provider}
            onModelChanged={isResearch ? handleResearchComposerModelChanged : undefined}
            submitDisabled={Boolean(submitDisabledReason) || isCreatingSession}
            submitDisabledReason={submitDisabledReason}
            allowEmptySubmit={isResearch && researchInitialInputCount(researchInitialInputs) > 0}
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
