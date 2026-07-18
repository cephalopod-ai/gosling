import { useEffect, useState } from 'react';
import {
  AlertTriangle,
  Copy,
  ExternalLink,
  FileOutput,
  FolderOpen,
  PanelRightClose,
  Save,
  X,
} from 'lucide-react';
import { toast } from 'react-toastify';
import { defineMessages, useIntl } from '../../i18n';
import { useArtifactWorkbench } from '../../contexts/ArtifactWorkbenchContext';
import { cn } from '../../utils';
import MarkdownContent from '../MarkdownContent';
import { Button } from '../ui/button';
import { addSandboxCsp, parseCsv } from './artifactUtils';
import type { ArtifactTab } from './types';
import { useArtifactRouter } from '../../contexts/ArtifactRouterContext';
import { errorMessage } from '../../utils/conversionUtils';

const i18n = defineMessages({
  outputs: { id: 'artifactPane.outputs', defaultMessage: 'Outputs' },
  closePane: { id: 'artifactPane.closePane', defaultMessage: 'Close outputs pane' },
  openFile: { id: 'artifactPane.openFile', defaultMessage: 'Open file' },
  emptyTitle: { id: 'artifactPane.emptyTitle', defaultMessage: 'View an output or deliverable' },
  emptyBody: {
    id: 'artifactPane.emptyBody',
    defaultMessage: 'Open a local file, or send a tool result here from the conversation.',
  },
  previewFailed: { id: 'artifactPane.previewFailed', defaultMessage: 'Preview unavailable' },
  previewTruncated: {
    id: 'artifactPane.previewTruncated',
    defaultMessage: 'This preview is truncated. Open the file for the complete output.',
  },
  unsupported: {
    id: 'artifactPane.unsupported',
    defaultMessage: 'This file type does not have an in-app preview yet.',
  },
  loading: { id: 'artifactPane.loading', defaultMessage: 'Loading…' },
  copyPath: { id: 'artifactPane.copyPath', defaultMessage: 'Copy path' },
  reveal: { id: 'artifactPane.reveal', defaultMessage: 'Reveal' },
  openExternal: { id: 'artifactPane.openExternal', defaultMessage: 'Open externally' },
  saveCopy: { id: 'artifactPane.saveCopy', defaultMessage: 'Save a copy' },
  savedCopy: { id: 'artifactPane.savedCopy', defaultMessage: 'Artifact copy saved' },
  saveCopyFailed: {
    id: 'artifactPane.saveCopyFailed',
    defaultMessage: 'Unable to save artifact: {error}',
  },
});

interface PreviewData {
  content: string;
  encoding: 'base64' | 'utf8';
  error: string | null;
  filePath?: string;
  sizeBytes?: number;
  truncated: boolean;
}

function mimeTypeForTab(tab: ArtifactTab): string {
  if (tab.source.type === 'content') return tab.source.mimeType;
  switch (tab.kind) {
    case 'svg':
      return 'image/svg+xml';
    case 'image': {
      const extension = tab.source.path.split('.').pop()?.toLowerCase();
      if (extension === 'jpg' || extension === 'jpeg') return 'image/jpeg';
      if (extension === 'gif') return 'image/gif';
      if (extension === 'webp') return 'image/webp';
      return 'image/png';
    }
    default:
      return 'text/plain';
  }
}

function JsonPreview({ content, jsonl }: { content: string; jsonl: boolean }) {
  let formatted = content;
  try {
    formatted = jsonl
      ? content
          .split('\n')
          .filter(Boolean)
          .map((line) => JSON.stringify(JSON.parse(line), null, 2))
          .join('\n')
      : JSON.stringify(JSON.parse(content), null, 2);
  } catch {
    // Keep the original text visible when an incomplete or malformed file is being inspected.
  }
  return <pre className="whitespace-pre-wrap break-words font-mono text-xs p-4">{formatted}</pre>;
}

function CsvPreview({ content }: { content: string }) {
  const rows = parseCsv(content);
  if (rows.length === 0) return null;
  const [header, ...body] = rows;
  return (
    <div className="overflow-auto h-full p-3">
      <table className="min-w-full border-collapse text-xs">
        <thead className="sticky top-0 bg-background-secondary">
          <tr>
            {header.map((cell, index) => (
              <th
                key={index}
                className="border border-border-primary px-2 py-1.5 text-left font-medium"
              >
                {cell}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {body.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {header.map((_, columnIndex) => (
                <td
                  key={columnIndex}
                  className="border border-border-primary px-2 py-1.5 align-top"
                >
                  {row[columnIndex] ?? ''}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function Preview({ tab, data }: { tab: ArtifactTab; data: PreviewData }) {
  const intl = useIntl();
  if (data.error) {
    return (
      <div className="m-4 rounded-lg border border-border-primary p-4 text-sm">
        <div className="flex items-center gap-2 font-medium">
          <AlertTriangle className="h-4 w-4" />
          {intl.formatMessage(i18n.previewFailed)}
        </div>
        <p className="mt-2 text-text-secondary">{data.error}</p>
      </div>
    );
  }

  if (
    data.truncated &&
    (tab.kind === 'html' || tab.kind === 'image' || tab.kind === 'pdf' || tab.kind === 'svg')
  ) {
    return (
      <div className="m-4 rounded-lg border border-border-primary p-4 text-sm text-text-secondary">
        {intl.formatMessage(i18n.previewTruncated)}
      </div>
    );
  }

  switch (tab.kind) {
    case 'markdown':
      return (
        <div className="p-5">
          <MarkdownContent content={data.content} />
        </div>
      );
    case 'csv':
      return <CsvPreview content={data.content} />;
    case 'json':
      return <JsonPreview content={data.content} jsonl={false} />;
    case 'jsonl':
      return <JsonPreview content={data.content} jsonl />;
    case 'html':
      return (
        <iframe
          className="h-full w-full border-0 bg-white"
          sandbox="allow-scripts"
          referrerPolicy="no-referrer"
          srcDoc={addSandboxCsp(data.content)}
          title={tab.title}
        />
      );
    case 'image':
    case 'svg':
      return (
        <div className="flex min-h-full items-center justify-center p-4 bg-background-secondary">
          <img
            className="max-h-full max-w-full object-contain"
            src={`data:${mimeTypeForTab(tab)};base64,${data.content}`}
            alt={tab.title}
          />
        </div>
      );
    case 'pdf':
      return (
        <iframe
          className="h-full w-full border-0 bg-white"
          src={`data:application/pdf;base64,${data.content}`}
          title={tab.title}
        />
      );
    case 'graphml':
    case 'text':
      return (
        <pre className="whitespace-pre-wrap break-words font-mono text-xs p-4">{data.content}</pre>
      );
    default:
      return (
        <div className="p-5 text-sm text-text-secondary">
          {intl.formatMessage(i18n.unsupported)}
        </div>
      );
  }
}

export function ArtifactPane() {
  const intl = useIntl();
  const { saveArtifact } = useArtifactRouter();
  const {
    activeTab,
    activeTabId,
    closeTab,
    openFile,
    resolveFilePath,
    setActiveTabId,
    setIsOpen,
    setWidth,
    tabs,
    width,
  } = useArtifactWorkbench();
  const [preview, setPreview] = useState<PreviewData | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!activeTab) {
      setPreview(null);
      return;
    }
    if (activeTab.source.type === 'content') {
      setPreview({
        content: activeTab.source.content,
        encoding: activeTab.source.encoding,
        error: null,
        truncated: false,
      });
      return;
    }
    if (activeTab.kind === 'unknown') {
      setPreview({ content: '', encoding: 'utf8', error: null, truncated: false });
      return;
    }

    let cancelled = false;
    const sourcePath = activeTab.source.path;
    const sourceBaseDirectory = activeTab.source.baseDirectory;
    setLoading(true);
    window.electron
      .readArtifactFile(sourcePath, sourceBaseDirectory)
      .then((response) => {
        if (cancelled) return;
        setPreview(response);
        if (response.found && response.filePath !== sourcePath) {
          resolveFilePath(activeTab.id, response.filePath);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab, resolveFilePath]);

  const chooseFile = async () => {
    const selected = await window.electron.selectArtifactFile(
      activeTab?.source.type === 'file' ? activeTab.source.path : undefined
    );
    if (selected) openFile(selected);
  };

  const resizeFrom = (event: React.PointerEvent) => {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = width;
    const move = (moveEvent: globalThis.PointerEvent) =>
      setWidth(startWidth + startX - moveEvent.clientX);
    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop);
  };

  const filePath = activeTab?.source.type === 'file' ? activeTab.source.path : null;
  const fileBaseDirectory =
    activeTab?.source.type === 'file' ? activeTab.source.baseDirectory : undefined;

  const saveCopy = async () => {
    if (!activeTab) return;
    try {
      const source =
        activeTab.source.type === 'file'
          ? {
              type: 'file' as const,
              path: activeTab.source.path,
              baseDirectory: activeTab.source.baseDirectory,
            }
          : {
              type: 'content' as const,
              content: activeTab.source.content,
              encoding: activeTab.source.encoding,
            };
      const result = await saveArtifact({
        workspaceId: activeTab.workspaceId,
        mimeType: mimeTypeForTab(activeTab),
        suggestedName: activeTab.title,
        title: intl.formatMessage(i18n.saveCopy),
        source,
      });
      if (!result.canceled) toast.success(intl.formatMessage(i18n.savedCopy));
    } catch (cause) {
      toast.error(
        intl.formatMessage(i18n.saveCopyFailed, {
          error: errorMessage(cause, 'Unknown error'),
        })
      );
    }
  };

  return (
    <div className="relative flex h-full flex-col overflow-hidden rounded-xl border border-border-primary bg-background-primary">
      <div
        className="absolute inset-y-0 -left-1 z-10 w-2 cursor-col-resize"
        onPointerDown={resizeFrom}
      />
      <div className="flex h-11 shrink-0 items-center gap-2 border-b border-border-primary px-2">
        <FileOutput className="h-4 w-4 text-text-secondary" />
        <span className="text-sm font-medium">{intl.formatMessage(i18n.outputs)}</span>
        <div className="ml-auto flex items-center gap-1">
          <Button
            variant="ghost"
            size="xs"
            onClick={() => void chooseFile()}
            title={intl.formatMessage(i18n.openFile)}
          >
            <FolderOpen className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="xs"
            onClick={() => setIsOpen(false)}
            title={intl.formatMessage(i18n.closePane)}
          >
            <PanelRightClose className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {tabs.length > 0 && (
        <div className="flex shrink-0 overflow-x-auto border-b border-border-primary">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTabId(tab.id)}
              className={cn(
                'group flex max-w-52 shrink-0 items-center gap-2 border-r border-border-primary px-3 py-2 text-xs',
                tab.id === activeTabId
                  ? 'bg-background-secondary text-text-primary'
                  : 'text-text-secondary hover:bg-background-secondary/60'
              )}
            >
              <span className="truncate">{tab.title}</span>
              <X
                className="h-3 w-3 shrink-0 opacity-60 hover:opacity-100"
                onClick={(event) => {
                  event.stopPropagation();
                  closeTab(tab.id);
                }}
              />
            </button>
          ))}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto">
        {!activeTab ? (
          <div className="flex h-full flex-col items-center justify-center p-8 text-center">
            <FileOutput className="h-8 w-8 text-text-secondary" />
            <h2 className="mt-3 text-sm font-medium">{intl.formatMessage(i18n.emptyTitle)}</h2>
            <p className="mt-1 max-w-xs text-xs text-text-secondary">
              {intl.formatMessage(i18n.emptyBody)}
            </p>
            <Button className="mt-4" variant="outline" size="sm" onClick={() => void chooseFile()}>
              <FolderOpen className="mr-2 h-4 w-4" />
              {intl.formatMessage(i18n.openFile)}
            </Button>
          </div>
        ) : loading || !preview ? (
          <div className="flex h-full items-center justify-center text-sm text-text-secondary">
            {intl.formatMessage(i18n.loading)}
          </div>
        ) : (
          <>
            {preview.truncated &&
              activeTab.kind !== 'html' &&
              activeTab.kind !== 'image' &&
              activeTab.kind !== 'pdf' &&
              activeTab.kind !== 'svg' && (
                <div className="m-3 flex items-center gap-2 rounded-md border border-border-primary px-3 py-2 text-xs text-text-secondary">
                  <AlertTriangle className="h-4 w-4" />
                  {intl.formatMessage(i18n.previewTruncated)}
                </div>
              )}
            <Preview tab={activeTab} data={preview} />
          </>
        )}
      </div>

      {activeTab && (
        <div className="flex shrink-0 items-center gap-1 border-t border-border-primary px-2 py-1.5">
          <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-text-secondary">
            {filePath ?? activeTab.title}
          </span>
          <Button
            variant="ghost"
            size="xs"
            title={intl.formatMessage(i18n.saveCopy)}
            aria-label={intl.formatMessage(i18n.saveCopy)}
            disabled={loading || Boolean(preview?.error)}
            onClick={() => void saveCopy()}
          >
            <Save className="h-3.5 w-3.5" />
          </Button>
          {filePath && (
            <>
              <Button
                variant="ghost"
                size="xs"
                title={intl.formatMessage(i18n.copyPath)}
                onClick={() => void window.electron.writeClipboardText(filePath)}
              >
                <Copy className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="xs"
                title={intl.formatMessage(i18n.reveal)}
                onClick={() => void window.electron.revealArtifactFile(filePath, fileBaseDirectory)}
              >
                <FolderOpen className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="xs"
                title={intl.formatMessage(i18n.openExternal)}
                onClick={() => void window.electron.openArtifactFile(filePath, fileBaseDirectory)}
              >
                <ExternalLink className="h-3.5 w-3.5" />
              </Button>
            </>
          )}
        </div>
      )}
    </div>
  );
}
