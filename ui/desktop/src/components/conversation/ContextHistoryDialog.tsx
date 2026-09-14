import { useEffect, useMemo, useState } from 'react';
import type {
  CompactionHistoryPurgeMode,
  CompactionRevisionDto,
  CompactionRevisionListItemDto,
} from '@repo-makeover/gosling-sdk';
import { Copy, History, LoaderCircle, Pin, PinOff, Trash2 } from 'lucide-react';
import {
  deleteContextHistoryRevision,
  getContextHistory,
  getContextHistoryRevision,
  purgeContextHistory,
  setContextHistoryPinned,
} from '../../acp/contextHistory';
import { defineMessages, useIntl } from '../../i18n';
import { writeTextToClipboard } from '../../utils/clipboard';
import { errorMessage } from '../../utils/conversionUtils';
import { Button } from '../ui/button';
import { ConfirmationModal } from '../ui/ConfirmationModal';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../ui/dialog';

const i18n = defineMessages({
  title: { id: 'contextHistory.title', defaultMessage: 'Context History' },
  description: {
    id: 'contextHistory.description',
    defaultMessage:
      'Walk through the summaries Gosling used to keep long sessions within the model context window.',
  },
  boundary: {
    id: 'contextHistory.boundary',
    defaultMessage:
      'This is local brainstorming and coding history, not a tamper-proof compliance audit. Summaries may contain sensitive session details.',
  },
  loading: { id: 'contextHistory.loading', defaultMessage: 'Loading Context History…' },
  empty: {
    id: 'contextHistory.empty',
    defaultMessage: 'No saved context snapshots yet. New successful compactions will appear here.',
  },
  includeExpired: {
    id: 'contextHistory.includeExpired',
    defaultMessage: 'Show expired snapshots still in the grace period',
  },
  loadOlder: { id: 'contextHistory.loadOlder', defaultMessage: 'Load older snapshots' },
  snapshot: { id: 'contextHistory.snapshot', defaultMessage: 'Snapshot #{generation}' },
  pinned: { id: 'contextHistory.pinned', defaultMessage: 'Pinned' },
  expired: { id: 'contextHistory.expired', defaultMessage: 'Expired' },
  retained: { id: 'contextHistory.retained', defaultMessage: 'Retained' },
  durable: { id: 'contextHistory.durable', defaultMessage: 'Saved context' },
  temporary: { id: 'contextHistory.temporary', defaultMessage: 'Temporary resume context' },
  manual: { id: 'contextHistory.manual', defaultMessage: 'Manual compaction' },
  automatic: { id: 'contextHistory.automatic', defaultMessage: 'Automatic compaction' },
  overflow: { id: 'contextHistory.overflow', defaultMessage: 'Overflow recovery' },
  tokens: {
    id: 'contextHistory.tokens',
    defaultMessage:
      '{before, number} → {after, number} estimated tokens ({removed, number} removed)',
  },
  tokenEstimate: {
    id: 'contextHistory.tokenEstimate',
    defaultMessage: '{before, number} → {after, number} tokens',
  },
  sourceMessages: {
    id: 'contextHistory.sourceMessages',
    defaultMessage: '{count} source messages',
  },
  model: {
    id: 'contextHistory.model',
    defaultMessage: 'Model: {model} · Provider: {provider}',
  },
  noExpiry: { id: 'contextHistory.noExpiry', defaultMessage: 'No time-based expiration' },
  expires: { id: 'contextHistory.expires', defaultMessage: 'Expires {date}' },
  purgeAfter: {
    id: 'contextHistory.purgeAfter',
    defaultMessage: 'Eligible for automatic cleanup {date}',
  },
  pin: { id: 'contextHistory.pin', defaultMessage: 'Pin' },
  unpin: { id: 'contextHistory.unpin', defaultMessage: 'Unpin' },
  delete: { id: 'contextHistory.delete', defaultMessage: 'Delete' },
  copy: { id: 'contextHistory.copy', defaultMessage: 'Copy summary' },
  copied: { id: 'contextHistory.copied', defaultMessage: 'Summary copied' },
  compare: { id: 'contextHistory.compare', defaultMessage: 'Compare with previous available' },
  currentSummary: { id: 'contextHistory.currentSummary', defaultMessage: 'Selected summary' },
  previousSummary: { id: 'contextHistory.previousSummary', defaultMessage: 'Previous summary' },
  provenance: { id: 'contextHistory.provenance', defaultMessage: 'Technical provenance' },
  summaryHash: { id: 'contextHistory.summaryHash', defaultMessage: 'Summary hash' },
  sourceHash: { id: 'contextHistory.sourceHash', defaultMessage: 'Source hash' },
  promptHash: { id: 'contextHistory.promptHash', defaultMessage: 'Prompt hash' },
  revisionId: { id: 'contextHistory.revisionId', defaultMessage: 'Revision ID' },
  sourceIds: { id: 'contextHistory.sourceIds', defaultMessage: 'Source IDs' },
  none: { id: 'contextHistory.none', defaultMessage: 'None' },
  unknown: { id: 'contextHistory.unknown', defaultMessage: 'Unknown' },
  payloadSize: {
    id: 'contextHistory.payloadSize',
    defaultMessage: 'Saved payload: {size}',
  },
  missingEarlier: {
    id: 'contextHistory.missingEarlier',
    defaultMessage: '{count, number} snapshot(s) were removed; generation gaps are intentional.',
  },
  cleanExpired: { id: 'contextHistory.cleanExpired', defaultMessage: 'Clean expired' },
  clearUnpinned: { id: 'contextHistory.clearUnpinned', defaultMessage: 'Clear unpinned' },
  confirmDeleteTitle: {
    id: 'contextHistory.confirmDeleteTitle',
    defaultMessage: 'Delete this snapshot?',
  },
  confirmDelete: {
    id: 'contextHistory.confirmDelete',
    defaultMessage:
      'This removes the selected summary from Context History. It does not delete the original chat transcript.',
  },
  confirmPurgeTitle: {
    id: 'contextHistory.confirmPurgeTitle',
    defaultMessage: 'Clean Context History?',
  },
  confirmExpired: {
    id: 'contextHistory.confirmExpired',
    defaultMessage: 'Delete every expired, unpinned snapshot for this session now?',
  },
  confirmUnpinned: {
    id: 'contextHistory.confirmUnpinned',
    defaultMessage:
      'Delete every unpinned snapshot for this session, including snapshots that have not expired?',
  },
  deleted: { id: 'contextHistory.deleted', defaultMessage: 'Snapshot deleted.' },
  purged: {
    id: 'contextHistory.purged',
    defaultMessage: 'Deleted {count} snapshot(s).',
  },
});

type PendingAction =
  | { type: 'delete'; generation: number }
  | { type: 'purge'; mode: CompactionHistoryPurgeMode }
  | null;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export function ContextHistoryDialog({
  sessionId,
  open,
  onOpenChange,
}: {
  sessionId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const intl = useIntl();
  const [items, setItems] = useState<CompactionRevisionListItemDto[]>([]);
  const [selectedGeneration, setSelectedGeneration] = useState<number | null>(null);
  const [selected, setSelected] = useState<CompactionRevisionDto | null>(null);
  const [previous, setPrevious] = useState<CompactionRevisionDto | null>(null);
  const [next, setNext] = useState<number | null>(null);
  const [purgedCount, setPurgedCount] = useState(0);
  const [includeExpired, setIncludeExpired] = useState(false);
  const [compare, setCompare] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);

  useEffect(() => {
    if (!open) return;
    let canceled = false;
    setLoading(true);
    setError(null);
    setNotice(null);
    void getContextHistory(sessionId, undefined, includeExpired)
      .then((page) => {
        if (canceled) return;
        setItems(page.revisions);
        setNext(page.nextBeforeGeneration ?? null);
        setPurgedCount(page.purgedCount);
        setSelectedGeneration((current) =>
          page.revisions.some((item) => item.generation === current)
            ? current
            : (page.revisions[0]?.generation ?? null)
        );
      })
      .catch((reason) => {
        if (!canceled) setError(errorMessage(reason));
      })
      .finally(() => {
        if (!canceled) setLoading(false);
      });
    return () => {
      canceled = true;
    };
  }, [includeExpired, open, refresh, sessionId]);

  const previousGeneration = useMemo(() => {
    const index = items.findIndex((item) => item.generation === selectedGeneration);
    return index >= 0 ? (items[index + 1]?.generation ?? null) : null;
  }, [items, selectedGeneration]);

  useEffect(() => {
    if (!open || selectedGeneration === null) {
      setSelected(null);
      setPrevious(null);
      return;
    }
    let canceled = false;
    setError(null);
    setSelected(null);
    setPrevious(null);
    void getContextHistoryRevision(sessionId, selectedGeneration)
      .then(({ revision }) => {
        if (!canceled) setSelected(revision);
      })
      .catch((reason) => {
        if (!canceled) setError(errorMessage(reason));
      });
    if (compare && previousGeneration !== null) {
      void getContextHistoryRevision(sessionId, previousGeneration)
        .then(({ revision }) => {
          if (!canceled) setPrevious(revision);
        })
        .catch((reason) => {
          if (!canceled) setError(errorMessage(reason));
        });
    } else {
      setPrevious(null);
    }
    return () => {
      canceled = true;
    };
  }, [compare, open, previousGeneration, selectedGeneration, sessionId]);

  const runPendingAction = async () => {
    if (!pendingAction || busy) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      if (pendingAction.type === 'delete') {
        await deleteContextHistoryRevision(sessionId, pendingAction.generation);
        setNotice(intl.formatMessage(i18n.deleted));
      } else {
        const result = await purgeContextHistory(sessionId, pendingAction.mode);
        setNotice(intl.formatMessage(i18n.purged, { count: result.deletedCount }));
      }
      setRefresh((value) => value + 1);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
      setPendingAction(null);
    }
  };

  const togglePinned = async () => {
    if (!selected || busy) return;
    setBusy(true);
    setError(null);
    try {
      const { revision } = await setContextHistoryPinned(
        sessionId,
        selected.generation,
        !selected.pinnedAt
      );
      setSelected(revision);
      setItems((current) =>
        current.map((item) =>
          item.generation === revision.generation
            ? {
                ...item,
                pinnedAt: revision.pinnedAt,
                expiresAt: revision.expiresAt,
                purgeAfter: revision.purgeAfter,
                expired: revision.expired,
              }
            : item
        )
      );
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  };

  const triggerLabel = (item: CompactionRevisionListItemDto) =>
    intl.formatMessage(
      item.trigger === 'manual'
        ? i18n.manual
        : item.trigger === 'overflow_recovery'
          ? i18n.overflow
          : i18n.automatic
    );

  return (
    <>
      <Dialog open={open} onOpenChange={(value) => !busy && onOpenChange(value)}>
        <DialogContent className="flex max-h-[90vh] flex-col sm:max-w-6xl">
          <DialogHeader className="pr-6">
            <DialogTitle className="flex items-center gap-2">
              <History className="size-5" />
              {intl.formatMessage(i18n.title)}
            </DialogTitle>
            <DialogDescription>{intl.formatMessage(i18n.description)}</DialogDescription>
          </DialogHeader>
          <p className="text-xs text-text-secondary">{intl.formatMessage(i18n.boundary)}</p>
          <div className="flex flex-wrap items-center gap-3 text-xs">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={includeExpired}
                disabled={busy}
                onChange={(event) => setIncludeExpired(event.target.checked)}
              />
              {intl.formatMessage(i18n.includeExpired)}
            </label>
            <span className="flex-1" />
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setPendingAction({ type: 'purge', mode: 'expired' })}
            >
              {intl.formatMessage(i18n.cleanExpired)}
            </Button>
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => setPendingAction({ type: 'purge', mode: 'all_unpinned' })}
            >
              {intl.formatMessage(i18n.clearUnpinned)}
            </Button>
          </div>
          {purgedCount > 0 && (
            <p className="text-xs text-text-secondary">
              {intl.formatMessage(i18n.missingEarlier, { count: purgedCount })}
            </p>
          )}
          {notice && (
            <p role="status" className="text-sm text-text-secondary">
              {notice}
            </p>
          )}
          {error && (
            <p role="alert" className="break-words text-sm text-text-danger">
              {error}
            </p>
          )}
          <div className="min-h-0 overflow-y-auto">
            {loading ? (
              <p role="status" className="flex items-center gap-2">
                <LoaderCircle className="size-4 animate-spin" />
                {intl.formatMessage(i18n.loading)}
              </p>
            ) : items.length === 0 ? (
              <p className="py-8 text-center text-sm text-text-secondary">
                {intl.formatMessage(i18n.empty)}
              </p>
            ) : (
              <div className="grid min-h-0 gap-4 md:grid-cols-[280px_minmax(0,1fr)]">
                <div className="max-h-72 overflow-y-auto md:max-h-[58vh]">
                  {items.map((item) => {
                    const state = item.pinnedAt
                      ? i18n.pinned
                      : item.expired
                        ? i18n.expired
                        : i18n.retained;
                    return (
                      <button
                        key={item.revisionId}
                        type="button"
                        aria-pressed={selectedGeneration === item.generation}
                        disabled={busy}
                        className={`mb-2 w-full rounded border p-3 text-left text-xs ${selectedGeneration === item.generation ? 'border-border-primary bg-background-secondary' : 'border-transparent hover:bg-background-secondary'}`}
                        onClick={() => setSelectedGeneration(item.generation)}
                      >
                        <strong className="block">
                          {intl.formatMessage(i18n.snapshot, { generation: item.generation })} ·{' '}
                          {intl.formatMessage(state)}
                        </strong>
                        <time className="block" dateTime={item.createdAt}>
                          {intl.formatDate(item.createdAt, {
                            dateStyle: 'medium',
                            timeStyle: 'medium',
                          })}
                        </time>
                        <span className="block">{triggerLabel(item)}</span>
                        <span className="block">
                          {intl.formatMessage(i18n.tokenEstimate, {
                            before: item.estimatedTokensBefore,
                            after: item.estimatedTokensAfter,
                          })}
                        </span>
                      </button>
                    );
                  })}
                  {next !== null && (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy}
                      onClick={async () => {
                        setBusy(true);
                        setError(null);
                        try {
                          const page = await getContextHistory(sessionId, next, includeExpired);
                          setItems((current) => [...current, ...page.revisions]);
                          setNext(page.nextBeforeGeneration ?? null);
                          setPurgedCount(page.purgedCount);
                        } catch (reason) {
                          setError(errorMessage(reason));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      {intl.formatMessage(i18n.loadOlder)}
                    </Button>
                  )}
                </div>
                <div className="min-w-0 space-y-3">
                  {selected ? (
                    <>
                      <div className="flex flex-wrap items-center gap-2">
                        <Button size="sm" variant="outline" disabled={busy} onClick={togglePinned}>
                          {selected.pinnedAt ? (
                            <PinOff className="size-4" />
                          ) : (
                            <Pin className="size-4" />
                          )}
                          {intl.formatMessage(selected.pinnedAt ? i18n.unpin : i18n.pin)}
                        </Button>
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={busy}
                          onClick={async () => {
                            await writeTextToClipboard(selected.summary);
                            setNotice(intl.formatMessage(i18n.copied));
                          }}
                        >
                          <Copy className="size-4" />
                          {intl.formatMessage(i18n.copy)}
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          disabled={busy}
                          onClick={() =>
                            setPendingAction({ type: 'delete', generation: selected.generation })
                          }
                        >
                          <Trash2 className="size-4" />
                          {intl.formatMessage(i18n.delete)}
                        </Button>
                      </div>
                      <div className="space-y-1 text-xs text-text-secondary">
                        <p>
                          {intl.formatMessage(
                            selected.effect === 'durable' ? i18n.durable : i18n.temporary
                          )}{' '}
                          · {triggerLabel(selected)}
                        </p>
                        <p>
                          {intl.formatMessage(i18n.tokens, {
                            before: selected.estimatedTokensBefore,
                            after: selected.estimatedTokensAfter,
                            removed: Math.max(
                              0,
                              selected.estimatedTokensBefore - selected.estimatedTokensAfter
                            ),
                          })}
                        </p>
                        <p>
                          {intl.formatMessage(i18n.sourceMessages, {
                            count: selected.sourceMessageCount,
                          })}
                        </p>
                        <p>
                          {intl.formatMessage(i18n.model, {
                            model: selected.resolvedModel,
                            provider: selected.provider ?? intl.formatMessage(i18n.unknown),
                          })}
                        </p>
                        <p>
                          {selected.pinnedAt
                            ? intl.formatMessage(i18n.pinned)
                            : selected.expiresAt
                              ? intl.formatMessage(i18n.expires, {
                                  date: intl.formatDate(selected.expiresAt, {
                                    dateStyle: 'medium',
                                    timeStyle: 'short',
                                  }),
                                })
                              : intl.formatMessage(i18n.noExpiry)}
                        </p>
                        {selected.purgeAfter && !selected.pinnedAt && (
                          <p>
                            {intl.formatMessage(i18n.purgeAfter, {
                              date: intl.formatDate(selected.purgeAfter, {
                                dateStyle: 'medium',
                                timeStyle: 'short',
                              }),
                            })}
                          </p>
                        )}
                        <p>
                          {intl.formatMessage(i18n.payloadSize, {
                            size: formatBytes(selected.payloadBytes),
                          })}
                        </p>
                      </div>
                      <label className="flex items-center gap-2 text-xs">
                        <input
                          type="checkbox"
                          checked={compare}
                          disabled={previousGeneration === null || busy}
                          onChange={(event) => setCompare(event.target.checked)}
                        />
                        {intl.formatMessage(i18n.compare)}
                      </label>
                      <div className={`grid gap-3 ${previous ? 'lg:grid-cols-2' : ''}`}>
                        {previous && (
                          <div>
                            <p className="mb-1 text-xs">
                              {intl.formatMessage(i18n.previousSummary)}
                            </p>
                            <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded bg-background-secondary p-3 text-xs">
                              {previous.summary}
                            </pre>
                          </div>
                        )}
                        <div>
                          <p className="mb-1 text-xs">{intl.formatMessage(i18n.currentSummary)}</p>
                          <pre className="max-h-80 overflow-auto whitespace-pre-wrap break-words rounded bg-background-secondary p-3 text-xs">
                            {selected.summary}
                          </pre>
                        </div>
                      </div>
                      <details className="text-xs text-text-secondary">
                        <summary className="cursor-pointer">
                          {intl.formatMessage(i18n.provenance)}
                        </summary>
                        <dl className="mt-2 grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 break-all">
                          <dt>{intl.formatMessage(i18n.summaryHash)}</dt>
                          <dd>{selected.summaryHash}</dd>
                          <dt>{intl.formatMessage(i18n.sourceHash)}</dt>
                          <dd>{selected.sourceHash}</dd>
                          <dt>{intl.formatMessage(i18n.promptHash)}</dt>
                          <dd>{selected.promptHash}</dd>
                          <dt>{intl.formatMessage(i18n.revisionId)}</dt>
                          <dd>{selected.revisionId}</dd>
                          <dt>{intl.formatMessage(i18n.sourceIds)}</dt>
                          <dd>
                            {selected.sourceMessageIds.join(', ') || intl.formatMessage(i18n.none)}
                          </dd>
                        </dl>
                      </details>
                    </>
                  ) : (
                    <p role="status" className="flex items-center gap-2 text-sm">
                      <LoaderCircle className="size-4 animate-spin" />
                      {intl.formatMessage(i18n.loading)}
                    </p>
                  )}
                </div>
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
      <ConfirmationModal
        isOpen={pendingAction !== null}
        title={intl.formatMessage(
          pendingAction?.type === 'delete' ? i18n.confirmDeleteTitle : i18n.confirmPurgeTitle
        )}
        message={intl.formatMessage(
          pendingAction?.type === 'delete'
            ? i18n.confirmDelete
            : pendingAction?.mode === 'all_unpinned'
              ? i18n.confirmUnpinned
              : i18n.confirmExpired
        )}
        confirmLabel={intl.formatMessage(i18n.delete)}
        confirmVariant="destructive"
        isSubmitting={busy}
        onCancel={() => setPendingAction(null)}
        onConfirm={() => void runPendingAction()}
      />
    </>
  );
}
