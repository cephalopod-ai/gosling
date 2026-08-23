import { useCallback, useEffect, useState } from 'react';
import { Check, GitBranch, Search } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { cn } from '../../utils';
import { defineMessages, useIntl } from '../../i18n';

const i18n = defineMessages({
  currentBranch: {
    id: 'gitBranchIndicator.currentBranch',
    defaultMessage: 'Current branch',
  },
  searchBranches: {
    id: 'gitBranchIndicator.searchBranches',
    defaultMessage: 'Search branches…',
  },
  noBranchesFound: {
    id: 'gitBranchIndicator.noBranchesFound',
    defaultMessage: 'No branches found',
  },
});

interface GitBranchIndicatorProps {
  dir: string;
  className?: string;
}

export function GitBranchIndicator({ dir, className }: GitBranchIndicatorProps) {
  const intl = useIntl();
  const [branch, setBranch] = useState<string | null>(null);
  const [branches, setBranches] = useState<string[]>([]);
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);

  const loadBranch = useCallback(async () => {
    const info = await window.electron.getGitBranchInfo(dir).catch(() => null);
    setBranch(info?.branch ?? null);
  }, [dir]);

  useEffect(() => {
    void loadBranch();
  }, [loadBranch]);

  useEffect(() => {
    if (!open) return;

    void window.electron
      .listGitBranches(dir)
      .then(setBranches)
      .catch(() => setBranches([]));
  }, [dir, open]);

  const handleBranchSelect = useCallback(
    async (nextBranch: string) => {
      const result = await window.electron.switchGitBranch(dir, nextBranch).catch(() => ({
        success: false,
      }));

      if (result.success) {
        setBranch(nextBranch);
        setOpen(false);
      }
    },
    [dir]
  );

  if (!branch) return null;

  const filteredBranches = branches.filter((candidate) =>
    candidate.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className={cn(
            'flex min-w-0 max-w-44 items-center gap-1 rounded px-1.5 py-1 text-xs text-text-secondary hover:bg-background-secondary hover:text-text-primary',
            className
          )}
          title={branch}
        >
          <GitBranch className="size-3.5 shrink-0" />
          <span className="truncate">{branch}</span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>{intl.formatMessage(i18n.currentBranch)}</DropdownMenuLabel>
        <div className="relative px-1 pb-1">
          <Search className="absolute left-3 top-2.5 size-3.5 text-text-secondary" />
          <input
            autoFocus
            className="h-8 w-full rounded border border-border-primary bg-background-primary py-1 pr-2 pl-7 text-sm outline-none focus:border-text-secondary"
            onChange={(event) => setQuery(event.target.value)}
            placeholder={intl.formatMessage(i18n.searchBranches)}
            value={query}
          />
        </div>
        <DropdownMenuSeparator />
        {filteredBranches.length === 0 ? (
          <DropdownMenuItem disabled>{intl.formatMessage(i18n.noBranchesFound)}</DropdownMenuItem>
        ) : (
          filteredBranches.map((candidate) => (
            <DropdownMenuItem key={candidate} onSelect={() => void handleBranchSelect(candidate)}>
              <span className="min-w-0 flex-1 truncate">{candidate}</span>
              {candidate === branch && <Check className="size-3.5" />}
            </DropdownMenuItem>
          ))
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
