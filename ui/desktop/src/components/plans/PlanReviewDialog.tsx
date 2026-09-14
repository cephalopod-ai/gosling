import type { AcpChatPlanState, AcpPlanFeedbackDraft } from '../../acp/chatSessionStore';
import MarkdownContent from '../MarkdownContent';
import { Button } from '../ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '../ui/dialog';

export interface PlanReviewDialogProps {
  open: boolean;
  plan: AcpChatPlanState;
  authorizationMode: string;
  sessionBusy: boolean;
  onOpenChange(open: boolean): void;
  onDraftChange(draft: Partial<AcpPlanFeedbackDraft>): void;
  onRequestChanges(): void;
  onApprove(): void;
  onApproveAndImplement(): void;
  onAbandon(): void;
  onExport(): void;
  onRefresh(): void;
  onStartAnother(): void;
}

export function PlanReviewDialog(props: PlanReviewDialogProps) {
  const snapshot = props.plan.snapshot;
  const revision = snapshot?.activeRevision;
  const status = snapshot?.plan.status;
  const pending = props.plan.actionPending !== null;
  const stale = props.plan.invalidated;
  const lineCount = Math.max(1, revision?.contentMarkdown.split('\n').length ?? 1);
  const draftTargetsCurrent =
    !props.plan.feedbackDraft.revisionId || props.plan.feedbackDraft.revisionId === revision?.id;
  const actionsDisabled =
    pending || props.sessionBusy || stale || status !== 'awaiting_review' || !revision;
  const exportDisabled = pending || props.sessionBusy || stale || !revision;
  const terminal = status === 'approved' || status === 'abandoned' || status === 'stale';

  const updateLine = (key: 'startLine' | 'endLine', raw: string) => {
    const value = raw === '' ? null : Math.min(lineCount, Math.max(1, Number(raw)));
    props.onDraftChange({
      [key]: Number.isFinite(value) ? value : null,
      revisionId: revision?.id ?? null,
    });
  };

  return (
    <Dialog open={props.open} onOpenChange={props.onOpenChange}>
      <DialogContent className="grid max-h-[calc(100vh-1rem)] grid-rows-[auto_minmax(0,1fr)_auto] gap-3 p-4 sm:max-w-4xl sm:p-6">
        <DialogHeader className="pr-7">
          <DialogTitle>Plan review</DialogTitle>
          <DialogDescription>
            {revision
              ? `Revision ${revision.revision} · ${revision.contentSha256.slice(0, 10)} · source through ${revision.sourceThroughRowId ?? 'current session'} · ${revision.plannerModel ?? snapshot?.plan.plannerModel ?? 'current model'}`
              : status === 'drafting'
                ? 'Gosling is drafting a host-enforced plan.'
                : 'No reviewable revision is available.'}
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-0 space-y-4 overflow-y-auto pr-1">
          {stale ? (
            <div
              className="rounded-md border border-border-warning bg-background-warning/20 p-3 text-sm"
              role="status"
              aria-live="polite"
            >
              A newer revision exists. Refresh before taking action.{' '}
              <Button type="button" size="xs" variant="outline" onClick={props.onRefresh}>
                Refresh
              </Button>
            </div>
          ) : null}
          {props.plan.loadError ? (
            <div className="rounded-md border border-border-danger p-3 text-sm" role="alert">
              {props.plan.loadError}
            </div>
          ) : null}
          {revision ? (
            <div className="rounded-md border border-border-primary bg-background-secondary p-4">
              <MarkdownContent content={revision.contentMarkdown} />
            </div>
          ) : null}

          {status === 'awaiting_review' && revision ? (
            <fieldset
              className="space-y-3 rounded-md border border-border-primary p-3"
              disabled={pending || stale}
            >
              <legend className="px-1 text-sm font-medium">Request changes</legend>
              {!draftTargetsCurrent ? (
                <p className="text-xs text-warning" role="status">
                  This draft was written for an older revision. Review it before sending against the
                  current plan.
                </p>
              ) : null}
              <div className="grid grid-cols-2 gap-3">
                <label className="text-xs text-text-secondary">
                  Start line (optional)
                  <input
                    className="mt-1 w-full rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    type="number"
                    min={1}
                    max={lineCount}
                    value={props.plan.feedbackDraft.startLine ?? ''}
                    onChange={(event) => updateLine('startLine', event.target.value)}
                  />
                </label>
                <label className="text-xs text-text-secondary">
                  End line (optional)
                  <input
                    className="mt-1 w-full rounded-md border border-border-primary bg-background-primary px-2 py-1.5 text-text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    type="number"
                    min={props.plan.feedbackDraft.startLine ?? 1}
                    max={lineCount}
                    value={props.plan.feedbackDraft.endLine ?? ''}
                    onChange={(event) => updateLine('endLine', event.target.value)}
                  />
                </label>
              </div>
              <label className="block text-xs text-text-secondary">
                Feedback
                <textarea
                  className="mt-1 min-h-24 w-full resize-y rounded-md border border-border-primary bg-background-primary px-3 py-2 text-sm text-text-primary outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  value={props.plan.feedbackDraft.body}
                  onChange={(event) =>
                    props.onDraftChange({ body: event.target.value, revisionId: revision.id })
                  }
                />
              </label>
              <Button
                type="button"
                variant="secondary"
                disabled={actionsDisabled || props.plan.feedbackDraft.body.trim().length === 0}
                onClick={props.onRequestChanges}
              >
                Request changes
              </Button>
            </fieldset>
          ) : null}

          <p className="text-xs text-text-secondary">
            “Approve and implement” starts implementation using the unchanged current authorization
            mode: {props.authorizationMode}.
          </p>
          <div role="status" aria-live="polite" className="min-h-5 text-sm text-text-secondary">
            {props.plan.workflowMessage}
          </div>
        </div>

        <DialogFooter className="flex-wrap border-t border-border-primary pt-3 sm:justify-between">
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              variant="destructive"
              disabled={
                pending ||
                props.sessionBusy ||
                stale ||
                !snapshot ||
                !['drafting', 'awaiting_review'].includes(status ?? '')
              }
              onClick={props.onAbandon}
            >
              Abandon
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={exportDisabled}
              onClick={props.onExport}
            >
              Export Markdown
            </Button>
          </div>
          <div className="flex flex-wrap gap-2">
            {terminal ? (
              <Button
                type="button"
                disabled={pending || props.sessionBusy || stale}
                onClick={props.onStartAnother}
              >
                Start new plan
              </Button>
            ) : (
              <>
                <Button
                  type="button"
                  variant="outline"
                  disabled={actionsDisabled}
                  onClick={props.onApprove}
                >
                  Approve
                </Button>
                <Button
                  type="button"
                  disabled={actionsDisabled}
                  onClick={props.onApproveAndImplement}
                >
                  Approve and implement
                </Button>
              </>
            )}
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
