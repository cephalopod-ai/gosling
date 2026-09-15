import { useCallback, useEffect, useState } from 'react';
import type {
  CompactionHistoryPolicyDto,
  PreviewCompactionHistoryPolicyResponse_unstable,
  ReadCompactionHistoryPolicyResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import { History, LoaderCircle, Save } from 'lucide-react';
import {
  applyContextHistoryPolicy,
  previewContextHistoryPolicy,
  readContextHistoryPolicy,
} from '../../../acp/contextHistory';
import { defineMessages, useIntl } from '../../../i18n';
import { errorMessage } from '../../../utils/conversionUtils';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { ConfirmationModal } from '../../ui/ConfirmationModal';
import { Input } from '../../ui/input';
import { Switch } from '../../ui/switch';

const i18n = defineMessages({
  title: { id: 'contextHistorySettings.title', defaultMessage: 'Context History' },
  description: {
    id: 'contextHistorySettings.description',
    defaultMessage:
      'Keep compacted summaries for coding and brainstorming walkthroughs while bounding local storage.',
  },
  capture: { id: 'contextHistorySettings.capture', defaultMessage: 'Save new snapshots' },
  captureDescription: {
    id: 'contextHistorySettings.captureDescription',
    defaultMessage: 'Record a snapshot after each successful manual or automatic compaction.',
  },
  expire: { id: 'contextHistorySettings.expire', defaultMessage: 'Expire snapshots' },
  expireDescription: {
    id: 'contextHistorySettings.expireDescription',
    defaultMessage: 'Pinned snapshots do not expire.',
  },
  retentionDays: {
    id: 'contextHistorySettings.retentionDays',
    defaultMessage: 'Keep for (days)',
  },
  graceDays: {
    id: 'contextHistorySettings.graceDays',
    defaultMessage: 'Cleanup grace period (days)',
  },
  graceDescription: {
    id: 'contextHistorySettings.graceDescription',
    defaultMessage: 'Expired snapshots stay recoverable for this long before automatic cleanup.',
  },
  maxPerSession: {
    id: 'contextHistorySettings.maxPerSession',
    defaultMessage: 'Maximum per session',
  },
  maxStorage: {
    id: 'contextHistorySettings.maxStorage',
    defaultMessage: 'Maximum local storage (MiB)',
  },
  currentUsage: {
    id: 'contextHistorySettings.currentUsage',
    defaultMessage:
      '{count, number} snapshots ({pinned, number} pinned) use {size}. {purged, number} snapshots have been removed.',
  },
  environmentManaged: {
    id: 'contextHistorySettings.environmentManaged',
    defaultMessage:
      'This policy is managed by the GOSLING_COMPACTION_HISTORY_POLICY environment variable. Change it in the launching environment.',
  },
  review: { id: 'contextHistorySettings.review', defaultMessage: 'Review changes' },
  reviewTitle: {
    id: 'contextHistorySettings.reviewTitle',
    defaultMessage: 'Apply Context History policy?',
  },
  reviewMessage: {
    id: 'contextHistorySettings.reviewMessage',
    defaultMessage:
      'The new policy applies to existing unpinned snapshots as well as future snapshots.',
  },
  previewImpact: {
    id: 'contextHistorySettings.previewImpact',
    defaultMessage:
      '{expired, number} will be marked expired, {purged, number} will be deleted now, and {limits, number} more will be removed to satisfy count or storage limits. {remaining, number} snapshots will remain.',
  },
  save: { id: 'contextHistorySettings.save', defaultMessage: 'Apply policy' },
  saved: {
    id: 'contextHistorySettings.saved',
    defaultMessage: 'Policy saved. {count, number} snapshot(s) were cleaned up.',
  },
  loading: { id: 'contextHistorySettings.loading', defaultMessage: 'Loading policy…' },
});

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

export default function ContextHistorySettings() {
  const intl = useIntl();
  const [current, setCurrent] = useState<ReadCompactionHistoryPolicyResponse_unstable | null>(null);
  const [draft, setDraft] = useState<CompactionHistoryPolicyDto | null>(null);
  const [preview, setPreview] = useState<PreviewCompactionHistoryPolicyResponse_unstable | null>(
    null
  );
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await readContextHistoryPolicy();
      setCurrent(response);
      setDraft(response.policy);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const update = <K extends keyof CompactionHistoryPolicyDto>(
    key: K,
    value: CompactionHistoryPolicyDto[K]
  ) => setDraft((policy) => (policy ? { ...policy, [key]: value } : policy));

  const review = async () => {
    if (!draft || busy) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      setPreview(await previewContextHistoryPolicy(draft));
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  };

  const apply = async () => {
    if (!preview || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await applyContextHistoryPolicy(preview.policy, preview.previewHash);
      setPreview(null);
      setNotice(
        intl.formatMessage(i18n.saved, {
          count: result.cleanup.deletedCount,
        })
      );
      await load();
    } catch (reason) {
      setPreview(null);
      setNotice(null);
      const applyError = errorMessage(reason);
      try {
        const response = await readContextHistoryPolicy();
        setCurrent(response);
        setDraft(response.policy);
        setError(applyError);
      } catch (reloadReason) {
        setCurrent(null);
        setDraft(null);
        setError(`${applyError} ${errorMessage(reloadReason)}`);
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Card className="rounded-lg">
        <CardHeader className="pb-0">
          <CardTitle className="flex items-center gap-2">
            <History className="size-5 text-iconStandard" />
            {intl.formatMessage(i18n.title)}
          </CardTitle>
          <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-5 px-4 pt-4">
          {error && (
            <p role="alert" className="break-words text-sm text-text-danger">
              {error}
            </p>
          )}
          {loading ? (
            <p role="status" className="flex items-center gap-2 text-sm text-text-secondary">
              <LoaderCircle className="size-4 animate-spin" />
              {intl.formatMessage(i18n.loading)}
            </p>
          ) : draft && current ? (
            <>
              <p className="text-xs text-text-secondary">
                {intl.formatMessage(i18n.currentUsage, {
                  count: current.stats.revisionCount,
                  pinned: current.stats.pinnedCount,
                  size: formatBytes(current.stats.payloadBytes),
                  purged: current.stats.purgedCount,
                })}
              </p>
              {current.managedByEnvironment && (
                <p className="rounded border border-border-primary bg-background-secondary p-3 text-xs text-text-secondary">
                  {intl.formatMessage(i18n.environmentManaged)}
                </p>
              )}
              <div className="flex items-center justify-between gap-4">
                <div>
                  <p className="text-sm text-text-primary">{intl.formatMessage(i18n.capture)}</p>
                  <p className="text-xs text-text-secondary">
                    {intl.formatMessage(i18n.captureDescription)}
                  </p>
                </div>
                <Switch
                  aria-label={intl.formatMessage(i18n.capture)}
                  checked={draft.captureEnabled}
                  disabled={current.managedByEnvironment || busy}
                  onCheckedChange={(checked) => update('captureEnabled', checked)}
                  variant="mono"
                />
              </div>
              <div className="flex items-center justify-between gap-4">
                <div>
                  <p className="text-sm text-text-primary">{intl.formatMessage(i18n.expire)}</p>
                  <p className="text-xs text-text-secondary">
                    {intl.formatMessage(i18n.expireDescription)}
                  </p>
                </div>
                <Switch
                  aria-label={intl.formatMessage(i18n.expire)}
                  checked={draft.retentionDays != null}
                  disabled={current.managedByEnvironment || busy}
                  onCheckedChange={(checked) => update('retentionDays', checked ? 90 : null)}
                  variant="mono"
                />
              </div>
              <div className="grid gap-4 sm:grid-cols-2">
                <label className="space-y-1 text-xs text-text-secondary">
                  {intl.formatMessage(i18n.retentionDays)}
                  <Input
                    type="number"
                    min={1}
                    max={3650}
                    value={draft.retentionDays ?? ''}
                    disabled={draft.retentionDays == null || current.managedByEnvironment || busy}
                    onChange={(event) => update('retentionDays', Number(event.target.value))}
                  />
                </label>
                <label className="space-y-1 text-xs text-text-secondary">
                  {intl.formatMessage(i18n.graceDays)}
                  <Input
                    type="number"
                    min={0}
                    max={365}
                    value={draft.purgeGraceDays}
                    disabled={current.managedByEnvironment || busy}
                    onChange={(event) => update('purgeGraceDays', Number(event.target.value))}
                  />
                  <span className="block">{intl.formatMessage(i18n.graceDescription)}</span>
                </label>
                <label className="space-y-1 text-xs text-text-secondary">
                  {intl.formatMessage(i18n.maxPerSession)}
                  <Input
                    type="number"
                    min={1}
                    max={10000}
                    value={draft.maxRevisionsPerSession}
                    disabled={current.managedByEnvironment || busy}
                    onChange={(event) =>
                      update('maxRevisionsPerSession', Number(event.target.value))
                    }
                  />
                </label>
                <label className="space-y-1 text-xs text-text-secondary">
                  {intl.formatMessage(i18n.maxStorage)}
                  <Input
                    type="number"
                    min={1}
                    max={16384}
                    value={Math.round(draft.maxTotalBytes / (1024 * 1024))}
                    disabled={current.managedByEnvironment || busy}
                    onChange={(event) =>
                      update('maxTotalBytes', Number(event.target.value) * 1024 * 1024)
                    }
                  />
                </label>
              </div>
              {notice && (
                <p role="status" className="text-sm text-text-secondary">
                  {notice}
                </p>
              )}
              <Button
                size="sm"
                disabled={current.managedByEnvironment || busy}
                onClick={() => void review()}
              >
                {busy ? (
                  <LoaderCircle className="size-4 animate-spin" />
                ) : (
                  <Save className="size-4" />
                )}
                {intl.formatMessage(i18n.review)}
              </Button>
            </>
          ) : null}
        </CardContent>
      </Card>
      <ConfirmationModal
        isOpen={preview !== null}
        title={intl.formatMessage(i18n.reviewTitle)}
        message={intl.formatMessage(i18n.reviewMessage)}
        detail={
          preview && (
            <div className="space-y-2">
              <p>
                {intl.formatMessage(i18n.previewImpact, {
                  expired: preview.impact.wouldExpireCount,
                  purged: preview.impact.wouldPurgeNowCount,
                  limits: preview.impact.wouldRemoveForLimitsCount,
                  remaining: preview.impact.projectedRevisionCount,
                })}
              </p>
              {preview.impact.warnings.map((warning) => (
                <p key={warning}>{warning}</p>
              ))}
            </div>
          )
        }
        confirmLabel={intl.formatMessage(i18n.save)}
        isSubmitting={busy}
        onCancel={() => setPreview(null)}
        onConfirm={() => void apply()}
      />
    </>
  );
}
