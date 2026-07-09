import { Folder } from 'lucide-react';
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
  if (!session?.working_dir) {
    return null;
  }

  const directories = [session.working_dir, ...(session.additional_working_dirs ?? [])];

  return (
    <div className="flex max-w-[540px] flex-wrap items-center justify-end gap-2">
      {directories.map((dir, index) => (
        <span
          key={dir}
          title={dir}
          className="inline-flex max-w-[220px] items-center gap-1 rounded-full border border-border-primary bg-background-secondary px-2 py-0.5 text-xs text-text-primary"
        >
          <Folder className="h-3 w-3 shrink-0 text-text-secondary" />
          <span className="truncate">{directoryLabel(dir)}</span>
          {index === 0 && <span className="text-[10px] uppercase text-text-secondary">Primary</span>}
        </span>
      ))}
      <WorkingDirectoriesMenu
        session={session}
        onSessionChange={onSessionChange}
        className="rounded-full border border-border-primary bg-background-secondary px-2 py-1 text-xs text-text-primary hover:bg-background-tertiary"
        compact={false}
      />
    </div>
  );
}
