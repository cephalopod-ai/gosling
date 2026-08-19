import { Folder, History, Menu, MessageSquarePlus, PanelLeft, Plus } from 'lucide-react';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import type { ShellSessionSummary } from '../../shell/acpRuntime';
import { StatusPill } from './ContextBar';

export interface DesktopShellNavigationProps {
  expanded: boolean;
  productName: string;
  snapshot: ShellRuntimeSnapshot;
  sessions: { status: 'idle' | 'loading' | 'loaded'; items: ShellSessionSummary[] };
  canCreate: boolean;
  canResume: boolean;
  canChangeDirectory: boolean;
  onToggle: () => void;
  onNewTask: () => void;
  onOpenSessions: () => void;
  onChooseDirectory: () => void;
  onResume: (sessionId: string) => void;
}

export const DesktopShellNavigation = ({
  expanded,
  productName,
  snapshot,
  sessions,
  canCreate,
  canResume,
  canChangeDirectory,
  onToggle,
  onNewTask,
  onOpenSessions,
  onChooseDirectory,
  onResume,
}: DesktopShellNavigationProps) => {
  if (!expanded) {
    return (
      <button
        type="button"
        className="gsh-nav-toggle gsh-nav-toggle--collapsed"
        onClick={onToggle}
        aria-label="Open navigation"
        title="Open navigation"
      >
        <Menu aria-hidden="true" />
      </button>
    );
  }

  const folderLabel = snapshot.directory.label ?? 'Choose a folder';

  return (
    <aside className="gsh-desktop-nav" aria-label="Primary navigation">
      <div className="gsh-desktop-nav__titlebar">
        <button
          type="button"
          className="gsh-nav-toggle"
          onClick={onToggle}
          aria-label="Collapse navigation"
          title="Collapse navigation"
        >
          <PanelLeft aria-hidden="true" />
        </button>
      </div>

      <nav className="gsh-desktop-nav__links">
        <button
          type="button"
          className="gsh-desktop-nav__link"
          onClick={onNewTask}
          disabled={!canCreate}
        >
          <MessageSquarePlus aria-hidden="true" />
          <span>New Chat</span>
        </button>
        <button type="button" className="gsh-desktop-nav__link" onClick={onOpenSessions}>
          <History aria-hidden="true" />
          <span>Session History</span>
        </button>
      </nav>

      <section className="gsh-desktop-nav__section" aria-labelledby="gsh-workspace-heading">
        <div className="gsh-desktop-nav__section-heading">
          <span id="gsh-workspace-heading">Workspace</span>
          {canChangeDirectory ? (
            <button
              type="button"
              className="gsh-desktop-nav__icon-button"
              onClick={onChooseDirectory}
              aria-label="Choose workspace folder"
              title="Choose workspace folder"
            >
              <Plus aria-hidden="true" />
            </button>
          ) : null}
        </div>
        <button
          type="button"
          className="gsh-desktop-nav__workspace gsh-desktop-nav__workspace--active"
          onClick={onChooseDirectory}
          disabled={!canChangeDirectory}
        >
          <span className="gsh-desktop-nav__workspace-icon">
            <Folder aria-hidden="true" />
          </span>
          <span className="gsh-desktop-nav__workspace-copy">
            <span className="gsh-desktop-nav__workspace-name">{folderLabel}</span>
            <span className="gsh-desktop-nav__workspace-meta">Default shell workspace</span>
          </span>
        </button>
      </section>

      <section
        className="gsh-desktop-nav__section gsh-desktop-nav__section--sessions"
        aria-labelledby="gsh-chats-heading"
      >
        <div className="gsh-desktop-nav__section-heading">
          <span id="gsh-chats-heading">Chats</span>
        </div>
        <div className="gsh-desktop-nav__sessions">
          {sessions.status === 'loading' ? (
            <span className="gsh-desktop-nav__empty">Loading chats…</span>
          ) : null}
          {sessions.status === 'loaded' && sessions.items.length === 0 ? (
            <span className="gsh-desktop-nav__empty">No recent chats</span>
          ) : null}
          {sessions.items.map((session) => (
            <button
              type="button"
              className="gsh-desktop-nav__session"
              key={session.sessionId}
              onClick={() => onResume(session.sessionId)}
              disabled={!canResume}
              title={session.title ?? 'Untitled chat'}
            >
              {session.title ?? 'Untitled chat'}
            </button>
          ))}
        </div>
      </section>

      <footer className="gsh-desktop-nav__footer">
        <span className="gsh-desktop-nav__product">{productName}</span>
        <StatusPill
          lifecycleState={snapshot.lifecycleState}
          compatibility={snapshot.compatibility}
        />
      </footer>
    </aside>
  );
};
