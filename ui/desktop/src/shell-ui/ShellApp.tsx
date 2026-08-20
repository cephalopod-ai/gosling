import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { COPY, lifecycleCopy } from './copy';
import { Composer } from './components/Composer';
import { ContextBar, IdentityBadge, StatusPill } from './components/ContextBar';
import { DesktopShellNavigation } from './components/DesktopShellNavigation';
import { FailureBanner, type RecoveryHandlers } from './components/FailureBanner';
import { HandoffDialog } from './components/HandoffDialog';
import { InteractionDock } from './components/InteractionDock';
import { LifecycleFailureScreen } from './components/LifecycleFailureScreen';
import { ModulesStrip } from './components/ModulesStrip';
import { OutputsPanel } from './components/OutputsPanel';
import { LibraryPanel } from './components/LibraryPanel';
import { ExtensionsPanel } from './components/ExtensionsPanel';
import { CredentialPicker, CredentialProblem, DirectoryPrompt } from './components/Pickers';
import { ShellButton, ShellButtonRow, ShellCentered, ShellNotice } from './components/primitives';
import { SessionPicker } from './components/SessionPicker';
import { TranscriptView } from './components/Transcript';
import { canChangeDirectory, canHandOff, isDeclared, selectRoute } from './state/route';
import type { ShellStore } from './state/store';

const RELINK_CAPABILITY = 'credential.relink';

export interface ShellAppProps {
  store: ShellStore;
  productName: string;
}

function useShellState(store: ShellStore) {
  return useSyncExternalStore(store.subscribe, store.getState, store.getState);
}

export const ShellApp = ({ store, productName }: ShellAppProps) => {
  const state = useShellState(store);
  const actions = store.actions;
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const generationRef = useRef<number | null>(null);
  const [navigationExpanded, setNavigationExpanded] = useState(true);
  const route = selectRoute(state);
  const snapshot = state.snapshot;

  const displayName = snapshot?.identity?.displayName ?? productName;

  useEffect(() => {
    const theme = state.settings?.appearance.theme ?? 'system';
    const scale = state.settings?.appearance.textScale ?? 1;
    const root = document.documentElement;
    root.setAttribute('data-gsh-theme', theme);
    root.style.setProperty('--gsh-text-scale', String(scale));
  }, [state.settings]);

  // A-4: a generation bump destroys runtime state, so focus returns to the composer and the draft
  // the user still has is left untouched.
  useEffect(() => {
    const generation = snapshot?.generation ?? null;
    const previous = generationRef.current;
    generationRef.current = generation;
    if (previous !== null && generation !== null && generation > previous) {
      composerRef.current?.focus();
    }
  }, [snapshot?.generation]);

  useEffect(() => {
    if (
      snapshot?.lifecycleState === 'ready' &&
      snapshot.directory.state === 'selected' &&
      state.sessions.status === 'idle' &&
      isDeclared(state, 'session.list')
    ) {
      void actions.listSessions();
    }
  }, [snapshot, state.sessions.status, state, actions]);

  useEffect(() => {
    if (
      route === 'S-06' &&
      state.outputs.status === 'idle' &&
      isDeclared(state, 'session.artifacts.read')
    ) {
      void actions.readOutputs();
    }
  }, [route, state.outputs.status, state, actions]);

  useEffect(() => {
    if (
      route === 'S-06' &&
      state.library.status === 'idle' &&
      isDeclared(state, 'session.library.read')
    ) {
      void actions.readLibrary();
    }
  }, [route, state.library.status, state, actions]);

  useEffect(() => {
    if (
      route === 'S-06' &&
      state.extensions.status === 'idle' &&
      isDeclared(state, 'session.extensions.read')
    ) {
      void actions.readExtensions();
    }
  }, [route, state.extensions.status, state, actions]);

  const recoveryHandlers = useMemo<RecoveryHandlers>(
    () => ({
      retry: () => void actions.retryRuntime(),
      refresh: () => void actions.refreshRuntime(),
      chooseDirectory: () => void actions.selectDirectory(),
      selectCredential: () => actions.setView('workspace'),
      reviewSession: () => actions.setView('workspace'),
      resetSettings: () => void actions.resetSettings(),
      openGosling: () => void actions.prepareHandoff(COPY.handoffQuestion, RELINK_CAPABILITY),
      saveDiagnostics: () => void actions.saveDiagnostics(),
      restart: () => void actions.retryRuntime(),
    }),
    [actions]
  );

  if (!snapshot) {
    const copy = lifecycleCopy('booting', productName);
    return (
      <div className="gsh-app">
        <ShellCentered heading={copy.heading} detail={copy.detail} />
      </div>
    );
  }

  const blockedByInteraction = state.interactions.length > 0;
  const dock = (
    <InteractionDock
      interactions={state.interactions}
      productName={displayName}
      onPermission={(actionId, allowOnce) => void actions.respondPermission(actionId, allowOnce)}
      onElicitation={(actionId, action, fields) =>
        void actions.respondElicitation(actionId, action, fields)
      }
      onConfirmation={(actionId, approve) => void actions.respondConfirmation(actionId, approve)}
    />
  );

  const failure = state.failure ? (
    <FailureBanner failure={state.failure} handlers={recoveryHandlers} />
  ) : null;

  const header = (
    <header className="gsh-titlebar">
      <IdentityBadge identity={snapshot.identity} fallbackName={productName} />
      <StatusPill lifecycleState={snapshot.lifecycleState} compatibility={snapshot.compatibility} />
    </header>
  );

  if (
    route === 'S-01' ||
    route === 'S-02' ||
    route === 'S-14' ||
    route === 'S-15' ||
    route === 'S-16' ||
    route === 'S-17' ||
    route === 'S-18' ||
    route === 'S-19' ||
    route === 'S-20'
  ) {
    const copy = lifecycleCopy(snapshot.lifecycleState, productName);
    const startup = route === 'S-01' || route === 'S-02' || route === 'S-19';
    return (
      <div className="gsh-app">
        {header}
        {startup ? (
          <ShellCentered heading={copy.heading} detail={copy.detail}>
            <ShellButtonRow>
              {snapshot.allowedActions.includes('diagnostics') ? (
                <ShellButton
                  label={COPY.saveDiagnostics}
                  onClick={() => void actions.saveDiagnostics()}
                />
              ) : null}
              {snapshot.allowedActions.includes('stop') ? (
                <ShellButton label={COPY.quit} onClick={() => void actions.stopRuntime()} />
              ) : null}
            </ShellButtonRow>
          </ShellCentered>
        ) : (
          <LifecycleFailureScreen
            snapshot={snapshot}
            productName={productName}
            savedDiagnosticsFile={state.savedDiagnosticsFile}
            canHandOff={canHandOff(state)}
            onRetry={() => void actions.retryRuntime()}
            onStop={() => void actions.stopRuntime()}
            onSaveDiagnostics={() => void actions.saveDiagnostics()}
            onHandoff={recoveryHandlers.openGosling}
            onRestart={() => void actions.retryRuntime()}
          />
        )}
        {failure}
      </div>
    );
  }

  const contextBar = (
    <ContextBar
      directory={snapshot.directory}
      credentials={snapshot.credentials}
      session={snapshot.session}
      canChangeDirectory={canChangeDirectory(state)}
      canSelectDirectory={isDeclared(state, 'directory.select')}
      canSelectCredential={isDeclared(state, 'credential.select')}
      onChooseDirectory={() => void actions.selectDirectory()}
      onChooseAccount={() => actions.setView('workspace')}
    />
  );

  const body = (() => {
    switch (route) {
      case 'S-28':
        return state.handoff ? (
          <HandoffDialog
            handoff={state.handoff}
            productName={displayName}
            onConfirm={() => void actions.confirmHandoff()}
            onCancel={() => store.dispatch({ type: 'handoff/cleared' })}
          />
        ) : null;

      case 'S-23':
      case 'S-03':
        return (
          <DirectoryPrompt
            directory={snapshot.directory}
            productName={productName}
            canSelect={isDeclared(state, 'directory.select')}
            cancelled={state.directoryCancelled}
            onChoose={() => void actions.selectDirectory()}
            onSaveDiagnostics={() => void actions.saveDiagnostics()}
          />
        );

      case 'S-24':
        return (
          <CredentialProblem
            credentials={snapshot.credentials}
            productName={productName}
            canSelect={isDeclared(state, 'credential.select')}
            canHandOff={canHandOff(state)}
            onOpenGosling={recoveryHandlers.openGosling}
            onChooseAnother={() => void actions.selectCredential(null)}
          />
        );

      case 'S-04':
        return (
          <CredentialPicker
            credentials={snapshot.credentials}
            productName={productName}
            canSelect={isDeclared(state, 'credential.select')}
            onSelect={(profileId) => void actions.selectCredential(profileId)}
            onRetry={() => void actions.retryRuntime()}
            onSaveDiagnostics={() => void actions.saveDiagnostics()}
          />
        );

      case 'S-05':
        return (
          <SessionPicker
            sessions={state.sessions}
            directory={snapshot.directory}
            credentials={snapshot.credentials}
            canCreate={isDeclared(state, 'session.create')}
            canResume={isDeclared(state, 'session.resume')}
            canList={isDeclared(state, 'session.list')}
            canSelectDirectory={canChangeDirectory(state) && isDeclared(state, 'directory.select')}
            canSelectCredential={
              snapshot.credentials.catalogStatus === 'available' &&
              isDeclared(state, 'credential.select')
            }
            onCreate={() => void actions.createSession()}
            onResume={(sessionId) => void actions.resumeSession(sessionId)}
            onRefresh={() => void actions.listSessions()}
            onChooseDirectory={() => void actions.selectDirectory()}
            onChooseCredential={() => void actions.selectCredential(null)}
          />
        );

      default:
        return (
          <div className="gsh-workspace">
            <TranscriptView
              transcript={state.transcript}
              onRepair={() => void actions.repairTranscript()}
              canRepair={isDeclared(state, 'session.transcript.read')}
            />
            {isDeclared(state, 'session.library.read') ||
            isDeclared(state, 'session.artifacts.read') ||
            isDeclared(state, 'session.extensions.read') ? (
              <aside className="gsh-reference-sidebar">
                {isDeclared(state, 'session.library.read') ? (
                  <LibraryPanel
                    library={state.library}
                    canWrite={isDeclared(state, 'session.library.write')}
                    busy={state.pending === 'session.library.write'}
                    onScopeChange={actions.setLibraryScope}
                    onToggle={actions.toggleLibraryItem}
                    onAddText={actions.addLibraryText}
                    onAddImage={actions.addLibraryImage}
                    onLinkFile={actions.linkLibraryFile}
                    onRemove={actions.removeLibraryItem}
                  />
                ) : null}
                {isDeclared(state, 'session.extensions.read') ? (
                  <ExtensionsPanel
                    extensions={state.extensions}
                    canWrite={isDeclared(state, 'session.extensions.write')}
                    busy={state.pending === 'session.extensions.write'}
                    onAdd={actions.addSessionExtension}
                    onRemove={actions.removeSessionExtension}
                  />
                ) : null}
                {isDeclared(state, 'session.artifacts.read') ? (
                  <OutputsPanel outputs={state.outputs} />
                ) : null}
              </aside>
            ) : null}
          </div>
        );
    }
  })();

  const hasActiveSession = Boolean(snapshot.session && snapshot.session.status !== 'none');
  const canOpenNewTask =
    isDeclared(state, 'session.create') &&
    (!hasActiveSession || isDeclared(state, 'session.detach'));

  const openNewTask = async () => {
    actions.setView('workspace');
    if (hasActiveSession && !(await actions.detachSession())) return;
    await actions.createSession();
  };

  return (
    <div className="gsh-app gsh-app--desktop">
      <DesktopShellNavigation
        expanded={navigationExpanded}
        productName={displayName}
        snapshot={snapshot}
        sessions={state.sessions}
        canCreate={canOpenNewTask}
        canResume={isDeclared(state, 'session.resume')}
        canChangeDirectory={canChangeDirectory(state) && isDeclared(state, 'directory.select')}
        onToggle={() => setNavigationExpanded((expanded) => !expanded)}
        onNewTask={() => void openNewTask()}
        onOpenSessions={() => actions.setView('sessions')}
        onChooseDirectory={() => void actions.selectDirectory()}
        onResume={(sessionId) => void actions.resumeSession(sessionId)}
      />
      <section className="gsh-desktop-main">
        <header className="gsh-desktop-main__titlebar">
          <IdentityBadge identity={snapshot.identity} fallbackName={productName} />
          <StatusPill
            lifecycleState={snapshot.lifecycleState}
            compatibility={snapshot.compatibility}
          />
        </header>
        {contextBar}
        {snapshot.provisioningIssues.length > 0 ? (
          <ShellNotice tone="warn" message={COPY.provisioningHeading} live>
            <ShellButton
              label={COPY.saveDiagnostics}
              onClick={() => void actions.saveDiagnostics()}
              emphasis="ghost"
            />
          </ShellNotice>
        ) : null}
        <main className="gsh-main">{body}</main>
        {dock}
        {failure}
        {route === 'S-06' ? (
          <Composer
            ref={composerRef}
            draft={state.draft}
            session={snapshot.session}
            blockedByInteraction={blockedByInteraction}
            canSubmit={isDeclared(state, 'prompt.submit')}
            canCancel={isDeclared(state, 'prompt.cancel')}
            attachmentCount={state.library.selectedItemIds.length}
            onDraftChange={actions.setDraft}
            onSubmit={() => void actions.submitPrompt()}
            onCancel={() => void actions.cancelPrompt()}
          />
        ) : null}
        <ModulesStrip
          modules={snapshot.modules}
          adapter={snapshot.adapter}
          domainDeclared={isDeclared(state, 'domain.action')}
          onSaveDiagnostics={() => void actions.saveDiagnostics()}
        />
      </section>
    </div>
  );
};
