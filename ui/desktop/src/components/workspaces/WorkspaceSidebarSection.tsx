import { useCallback, useState } from 'react';
import {
  Check,
  ChevronDown,
  ChevronRight,
  FolderOpen,
  MoreHorizontal,
  Plus,
  TriangleAlert,
} from 'lucide-react';
import type { Workspace, WorkspaceWithValidation } from '@repo-makeover/gosling-sdk';
import { toast } from 'react-toastify';
import { acpExportWorkspace } from '../../acp/workspaces';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { cn } from '../../utils';
import { workspaceErrorMessage } from '../../utils/workspaceError';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { WorkspaceEditorDialog } from './WorkspaceEditorDialog';

const COLLAPSED_KEY = 'workspaces_sidebar_collapsed';

export function WorkspaceSidebarSection() {
  const {
    workspaces,
    activeWorkspaceId,
    defaultWorkspaceId,
    sessionWorkspaceFilterId,
    setSessionWorkspaceFilterId,
    loading,
    error,
    duplicateWorkspace,
    deleteWorkspace,
    setActiveWorkspace,
  } = useWorkspace();
  const [expanded, setExpanded] = useState(
    () => window.localStorage.getItem(COLLAPSED_KEY) !== 'true'
  );
  const [editor, setEditor] = useState<{ open: boolean; workspace?: Workspace | null }>({
    open: false,
  });

  const toggleExpanded = useCallback(() => {
    setExpanded((current) => {
      window.localStorage.setItem(COLLAPSED_KEY, String(current));
      return !current;
    });
  }, []);

  const switchWorkspace = useCallback(
    async (workspace: Workspace) => {
      try {
        await setActiveWorkspace(workspace.id);
        toast.info(`New chats will use “${workspace.name}”. Open chats stay pinned.`);
      } catch (cause) {
        toast.error(workspaceErrorMessage(cause, 'Unable to switch workspace'));
      }
    },
    [setActiveWorkspace]
  );

  const reveal = useCallback(async (workspace: Workspace) => {
    try {
      await window.electron.addRecentDir(workspace.workingFolder);
      const opened = await window.electron.openDirectoryInExplorer(workspace.workingFolder);
      if (!opened) {
        throw new Error('The workspace folder is unavailable. Relink it in Edit workspace.');
      }
    } catch (cause) {
      toast.error(workspaceErrorMessage(cause, 'Unable to reveal the workspace folder'));
    }
  }, []);

  const exportMetadata = useCallback(async (workspace: Workspace) => {
    try {
      const document = await acpExportWorkspace(workspace.id);
      const result = await window.electron.showSaveDialog({
        title: 'Export workspace metadata',
        defaultPath: `${safeFileName(workspace.name)}.gosling-workspace.json`,
        filters: [{ name: 'Gosling workspace', extensions: ['json'] }],
      });
      if (!result.canceled && result.filePath) {
        const written = await window.electron.writeFile(result.filePath, document);
        if (!written) throw new Error('The export file could not be written');
        toast.success('Workspace metadata exported');
      }
    } catch (cause) {
      toast.error(workspaceErrorMessage(cause, 'Unable to export workspace'));
    }
  }, []);

  const remove = useCallback(
    async (workspace: Workspace) => {
      const response = await window.electron.showMessageBox({
        type: 'warning',
        buttons: ['Cancel', 'Delete workspace'],
        defaultId: 0,
        title: 'Delete workspace',
        message: `Delete “${workspace.name}”? Sessions and files will not be deleted.`,
      });
      if (response.response !== 1) return;
      try {
        await deleteWorkspace(workspace.id);
        toast.success('Workspace deleted; its sessions and files were preserved.');
      } catch (cause) {
        toast.error(workspaceErrorMessage(cause, 'Unable to delete workspace'));
      }
    },
    [deleteWorkspace]
  );

  return (
    <section aria-labelledby="workspaces-heading" className="border-b border-border-secondary pb-2">
      <div className="flex items-center px-3">
        <button
          type="button"
          onClick={toggleExpanded}
          className="flex min-w-0 flex-1 items-center gap-1 py-1 text-xs font-semibold uppercase tracking-wider text-text-secondary transition-colors hover:text-text-primary focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active"
          aria-expanded={expanded}
          aria-controls="workspace-list"
        >
          {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
          <span id="workspaces-heading">Workspaces</span>
        </button>
        <button
          type="button"
          onClick={() => setEditor({ open: true, workspace: null })}
          className="rounded-full p-1 text-text-secondary hover:bg-background-tertiary hover:text-text-primary focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active"
          aria-label="Add workspace"
        >
          <Plus className="size-4" />
        </button>
      </div>

      {expanded && (
        <div id="workspace-list" role="list" className="mt-1 space-y-0.5 px-2">
          <button
            type="button"
            onClick={() => setSessionWorkspaceFilterId(null)}
            className={workspaceRowClass(sessionWorkspaceFilterId === null)}
            aria-pressed={sessionWorkspaceFilterId === null}
          >
            <span className="flex size-5 items-center justify-center rounded bg-background-tertiary text-[10px]">
              ∞
            </span>
            <span className="flex-1 truncate text-left">All workspaces</span>
            {sessionWorkspaceFilterId === null && <Check className="size-3.5" />}
          </button>

          {loading ? (
            <div className="px-3 py-2 text-xs text-text-secondary">Loading workspaces…</div>
          ) : error ? (
            <div role="alert" className="px-3 py-2 text-xs text-red-600">
              {error}
            </div>
          ) : workspaces.length === 0 ? (
            <div className="px-3 py-2 text-xs text-text-secondary">No workspaces available</div>
          ) : (
            workspaces.map((item) => (
              <WorkspaceRow
                key={item.workspace.id}
                item={item}
                active={item.workspace.id === activeWorkspaceId}
                filtered={item.workspace.id === sessionWorkspaceFilterId}
                isDefault={item.workspace.id === defaultWorkspaceId}
                onOpen={() => void switchWorkspace(item.workspace)}
                onFilter={() => setSessionWorkspaceFilterId(item.workspace.id)}
                onEdit={() => setEditor({ open: true, workspace: item.workspace })}
                onDuplicate={() => {
                  void duplicateWorkspace(item.workspace.id)
                    .then((duplicate) => toast.success(`Created “${duplicate.name}”.`))
                    .catch((cause) =>
                      toast.error(workspaceErrorMessage(cause, 'Unable to duplicate workspace'))
                    );
                }}
                onReveal={() => void reveal(item.workspace)}
                onExport={() => void exportMetadata(item.workspace)}
                onDelete={() => void remove(item.workspace)}
              />
            ))
          )}
        </div>
      )}

      <WorkspaceEditorDialog
        open={editor.open}
        workspace={editor.workspace}
        onOpenChange={(open) => setEditor((current) => ({ ...current, open }))}
      />
    </section>
  );
}

function WorkspaceRow({
  item,
  active,
  filtered,
  isDefault,
  onOpen,
  onFilter,
  onEdit,
  onDuplicate,
  onReveal,
  onExport,
  onDelete,
}: {
  item: WorkspaceWithValidation;
  active: boolean;
  filtered: boolean;
  isDefault: boolean;
  onOpen(): void;
  onFilter(): void;
  onEdit(): void;
  onDuplicate(): void;
  onReveal(): void;
  onExport(): void;
  onDelete(): void;
}) {
  const { workspace, validation } = item;
  const hasWarnings = (validation.issues ?? []).length > 0;
  const warningSummary = (validation.issues ?? []).map((issue) => issue.message).join('; ');
  return (
    <div role="listitem" className="group flex items-center">
      <button
        type="button"
        onClick={onOpen}
        onDoubleClick={onFilter}
        className={workspaceRowClass(filtered)}
        aria-current={active ? 'true' : undefined}
        aria-label={`${workspace.name}${active ? ', active workspace' : ''}`}
      >
        <span className="flex size-5 items-center justify-center rounded bg-background-tertiary text-[10px] font-semibold uppercase">
          {(workspace.icon || workspace.name).slice(0, 2)}
        </span>
        <span className="min-w-0 flex-1 text-left">
          <span className="block truncate">{workspace.name}</span>
          <span className="block truncate text-[10px] text-text-secondary">
            {workspace.folders?.length ?? 0} refs · {workspace.productOutputFolders.length} outputs
          </span>
        </span>
        {hasWarnings && (
          <span
            role="img"
            aria-label={`Workspace needs attention: ${warningSummary}`}
            title={warningSummary}
          >
            <TriangleAlert className="size-3.5 text-amber-600" aria-hidden="true" />
          </span>
        )}
        {active && <span className="size-1.5 rounded-full bg-green-500" aria-hidden="true" />}
      </button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="-ml-7 rounded-full p-1 text-text-secondary opacity-0 hover:bg-background-secondary focus-visible:opacity-100 focus-visible:outline-none group-hover:opacity-100"
            aria-label={`Actions for ${workspace.name}`}
          >
            <MoreHorizontal className="size-4" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-48">
          <DropdownMenuItem onSelect={onOpen}>Open / switch</DropdownMenuItem>
          <DropdownMenuItem onSelect={onFilter}>Show its chats</DropdownMenuItem>
          <DropdownMenuItem onSelect={onEdit}>Edit</DropdownMenuItem>
          <DropdownMenuItem onSelect={onDuplicate}>Duplicate</DropdownMenuItem>
          <DropdownMenuItem onSelect={onReveal}>
            <FolderOpen className="size-4" /> Reveal primary folder
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={onExport}>Export metadata</DropdownMenuItem>
          <DropdownMenuItem
            onSelect={onDelete}
            disabled={isDefault}
            className="text-red-600 focus:text-red-600"
          >
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

function workspaceRowClass(selected: boolean): string {
  return cn(
    'flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-1.5 text-xs text-text-primary',
    'transition-colors hover:bg-background-tertiary/60 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-border-active',
    selected && 'bg-background-tertiary'
  );
}

function safeFileName(name: string): string {
  return (
    name
      .trim()
      .replace(/[^a-zA-Z0-9._-]+/g, '-')
      .replace(/^-+|-+$/g, '') || 'workspace'
  );
}
