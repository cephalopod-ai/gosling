import { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, Folder } from 'lucide-react';
import WorkingDirectoriesMenu from './WorkingDirectoriesMenu';
import type { Session } from '../types/session';

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
  const [isExpanded, setIsExpanded] = useState(true);

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

  if (directories.length === 0) {
    return null;
  }

  return (
    <div className="flex min-w-[220px] max-w-[420px] flex-col items-end gap-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={isExpanded}
          aria-label={isExpanded ? 'Collapse working directories' : 'Expand working directories'}
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
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
