import { useCallback, useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, Folder, X } from 'lucide-react';
import { toast } from 'react-toastify';
import WorkingDirectoriesMenu from './WorkingDirectoriesMenu';
import { defineMessages, useIntl } from '../i18n';
import { acpRemoveSessionWorkingDir } from '../acp/sessions';
import type { Session } from '../types/session';
import { CredentialProfileSelector } from './bottom_menu/CredentialProfileSelector';

const i18n = defineMessages({
  collapse: {
    id: 'workingDirectoriesSummary.collapse',
    defaultMessage: 'Collapse working directories',
  },
  expand: {
    id: 'workingDirectoriesSummary.expand',
    defaultMessage: 'Expand working directories',
  },
  removeDirectory: {
    id: 'workingDirectoriesSummary.removeDirectory',
    defaultMessage: 'Remove working directory',
  },
  failedToRemove: {
    id: 'workingDirectoriesSummary.failedToRemove',
    defaultMessage: 'Failed to remove working directory',
  },
});

interface WorkingDirectoriesSummaryProps {
  session?: Session;
  onSessionChange: (updater: (session: Session) => Session) => void;
}

function directoryLabel(dir: string): string {
  const trimmed = dir.replace(/\/+$/, '');
  const leaf = trimmed.split('/').pop();
  return leaf && leaf.length > 0 ? leaf : dir;
}

export default function WorkingDirectoriesSummary({
  session,
  onSessionChange,
}: WorkingDirectoriesSummaryProps) {
  const intl = useIntl();
  const [isExpanded, setIsExpanded] = useState(false);

  const directories = useMemo(() => {
    if (!session?.working_dir) {
      return [];
    }

    return [
      { path: session.working_dir, kind: 'primary' as const },
      ...(session.additional_working_dirs ?? []).map((path) => ({
        path,
        kind: 'additional' as const,
      })),
    ];
  }, [session?.working_dir, session?.additional_working_dirs]);

  const removeDirectory = useCallback(
    async (dir: string) => {
      if (!session) {
        return;
      }

      try {
        const result = await acpRemoveSessionWorkingDir(session.id, dir);
        onSessionChange((current) => ({
          ...current,
          additional_working_dirs: result.additionalWorkingDirs,
        }));
      } catch (error) {
        console.error('[WorkingDirectoriesSummary] Failed to remove working directory:', error);
        toast.error(intl.formatMessage(i18n.failedToRemove));
      }
    },
    [session, onSessionChange, intl]
  );

  if (directories.length === 0) {
    return null;
  }

  return (
    <div className="pointer-events-auto flex min-w-[220px] max-w-[420px] flex-col items-end gap-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={isExpanded}
          aria-label={
            isExpanded ? intl.formatMessage(i18n.collapse) : intl.formatMessage(i18n.expand)
          }
          className="no-drag inline-flex items-center gap-1 rounded-full border border-border-primary bg-background-secondary px-2 py-1 text-xs text-text-primary transition-colors hover:bg-background-tertiary"
          onClick={() => setIsExpanded((current) => !current)}
        >
          {isExpanded ? (
            <ChevronDown className="h-3.5 w-3.5 text-text-secondary" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-text-secondary" />
          )}
          <Folder className="h-3.5 w-3.5 text-text-secondary" />
          <span>Dirs</span>
          <span className="tabular-nums text-[11px] text-text-secondary">{directories.length}</span>
        </button>

        <WorkingDirectoriesMenu
          session={session}
          onSessionChange={onSessionChange}
          className="rounded-full border border-border-primary bg-background-secondary px-2 py-1 text-xs text-text-primary hover:bg-background-tertiary"
          compact
          showCount={false}
        />
      </div>

      <CredentialProfileSelector
        credentialProfileId={session?.credential_profile_id}
        credentialProfileName={session?.credential_profile_name}
        surface="header"
      />

      {isExpanded && (
        <div className="flex w-full flex-col gap-1.5 rounded-2xl border border-border-primary bg-background-secondary/70 p-2">
          {directories.map(({ path, kind }) => (
            <div
              key={path}
              title={path}
              className="flex w-full items-start gap-2 rounded-xl border border-border-primary/60 bg-background-primary/50 px-2.5 py-2 text-left"
            >
              <Folder className="mt-0.5 h-3.5 w-3.5 shrink-0 text-text-secondary" />
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-xs text-text-primary">{directoryLabel(path)}</span>
                  <span className="shrink-0 text-[10px] uppercase tracking-[0.08em] text-text-secondary">
                    {kind === 'primary' ? 'Primary working dir' : 'Working dir'}
                  </span>
                </div>
                <div className="truncate text-[11px] text-text-secondary">{path}</div>
              </div>
              {kind === 'additional' ? (
                <button
                  type="button"
                  aria-label={intl.formatMessage(i18n.removeDirectory)}
                  className="no-drag mt-0.5 rounded p-1 text-text-secondary transition-colors hover:bg-background-secondary hover:text-text-primary"
                  onClick={() => void removeDirectory(path)}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
