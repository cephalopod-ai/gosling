import type { ShellSessionSummary } from '../../shell/acpRuntime';
import type { ShellCredentialSnapshot } from '../../shell/credentialController';
import type { ShellDirectorySnapshot } from '../../shell/directoryController';
import { COPY } from '../copy';
import { ShellButton, ShellButtonRow } from './primitives';

export interface SessionPickerProps {
  sessions: { status: 'idle' | 'loading' | 'loaded'; items: ShellSessionSummary[] };
  directory: ShellDirectorySnapshot;
  credentials: ShellCredentialSnapshot;
  canCreate: boolean;
  canResume: boolean;
  canList: boolean;
  canSelectDirectory: boolean;
  canSelectCredential: boolean;
  onCreate: () => void;
  onResume: (sessionId: string) => void;
  onRefresh: () => void;
  onChooseDirectory: () => void;
  onChooseCredential: () => void;
}

export const SessionPicker = ({
  sessions,
  directory,
  credentials,
  canCreate,
  canResume,
  canList,
  canSelectDirectory,
  canSelectCredential,
  onCreate,
  onResume,
  onRefresh,
  onChooseDirectory,
  onChooseCredential,
}: SessionPickerProps) => {
  const showCredentials = canSelectCredential || credentials.selectionStatus !== 'none';
  const selectedCredential = credentials.profiles.find(
    (profile) => profile.id === credentials.selectedProfileId
  );

  return (
    <div className="gsh-dashboard">
      <nav className="gsh-dashboard__panel gsh-dashboard__panel--workspace" aria-label="Workspace">
        <h2 className="gsh-dashboard__heading">Workspace</h2>
        <dl className="gsh-dashboard__facts">
          <div>
            <dt>Folder</dt>
            <dd>{directory.label ?? 'No folder chosen'}</dd>
          </div>
          {showCredentials ? (
            <div>
              <dt>Account</dt>
              <dd>{selectedCredential?.name ?? 'No account chosen'}</dd>
            </div>
          ) : null}
        </dl>
        <ShellButtonRow>
          {canSelectDirectory ? (
            <ShellButton label="Change folder" onClick={onChooseDirectory} emphasis="ghost" />
          ) : null}
          {canSelectCredential ? (
            <ShellButton label="Change account" onClick={onChooseCredential} emphasis="ghost" />
          ) : null}
        </ShellButtonRow>
      </nav>

      <section
        className="gsh-dashboard__panel gsh-dashboard__panel--primary"
        aria-labelledby="tasks-heading"
      >
        <h1 id="tasks-heading" className="gsh-dashboard__title">
          Tasks
        </h1>
        <p className="gsh-dashboard__detail">
          Start a new task in this folder, or pick up a recent one.
        </p>
        <ShellButtonRow>
          {canCreate ? (
            <ShellButton label={COPY.startNewTask} onClick={onCreate} emphasis="primary" />
          ) : null}
        </ShellButtonRow>
      </section>

      <aside
        className="gsh-dashboard__panel gsh-dashboard__panel--sessions"
        aria-labelledby="recent-tasks-heading"
      >
        <div className="gsh-sessions__head">
          <h2 id="recent-tasks-heading" className="gsh-dashboard__heading">
            {COPY.sessionsHeading}
          </h2>
          {canList ? (
            <ShellButton
              label={COPY.refreshSessions}
              onClick={onRefresh}
              emphasis="ghost"
              disabled={sessions.status === 'loading'}
            />
          ) : null}
        </div>
        {sessions.status === 'loaded' && sessions.items.length === 0 ? (
          <p className="gsh-sessions__empty">{COPY.sessionsEmpty}</p>
        ) : (
          <ul className="gsh-sessions__list">
            {sessions.items.map((session) => (
              <li className="gsh-sessions__row" key={session.sessionId}>
                <span className="gsh-sessions__title">{session.title ?? 'Untitled task'}</span>
                <span className="gsh-sessions__meta">
                  {session.messageCount === null ? '' : `${session.messageCount} messages`}
                  {session.updatedAt ? ` · ${session.updatedAt}` : ''}
                </span>
                {canResume ? (
                  <ShellButton
                    label={COPY.resumeTask}
                    onClick={() => onResume(session.sessionId)}
                  />
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </aside>
    </div>
  );
};
