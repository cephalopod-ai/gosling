import type { ShellSessionSummary } from '../../shell/acpRuntime';
import { COPY } from '../copy';
import { ShellButton, ShellButtonRow } from './primitives';

export interface SessionPickerProps {
  sessions: { status: 'idle' | 'loading' | 'loaded'; items: ShellSessionSummary[] };
  canCreate: boolean;
  canResume: boolean;
  canList: boolean;
  onCreate: () => void;
  onResume: (sessionId: string) => void;
  onRefresh: () => void;
}

/** The 20-session cap is stated in the heading so the list never implies a full archive. */
export const SessionPicker = ({
  sessions,
  canCreate,
  canResume,
  canList,
  onCreate,
  onResume,
  onRefresh,
}: SessionPickerProps) => (
  <div className="gsh-sessions">
    <div className="gsh-sessions__head">
      <h2 className="gsh-sessions__heading">{COPY.sessionsHeading}</h2>
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
              <ShellButton label={COPY.resumeTask} onClick={() => onResume(session.sessionId)} />
            ) : null}
          </li>
        ))}
      </ul>
    )}
    <ShellButtonRow>
      {canCreate ? (
        <ShellButton label={COPY.startNewTask} onClick={onCreate} emphasis="primary" />
      ) : null}
    </ShellButtonRow>
  </div>
);
