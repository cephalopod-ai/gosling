import { ScrollText } from 'lucide-react';
import type { AcpChatPlanState } from '../../acp/chatSessionStore';
import { Button } from '../ui/button';

export function PlanStatusControl({
  plan,
  disabled,
  onStart,
  onOpen,
}: {
  plan: AcpChatPlanState;
  disabled: boolean;
  onStart(): void;
  onOpen(): void;
}) {
  const status = plan.snapshot?.plan.status;
  const hasPlan = plan.snapshot !== null;
  const active = status === 'drafting' || status === 'awaiting_review';
  const label = plan.loading
    ? 'Refreshing plan'
    : status === 'awaiting_review'
      ? 'Review plan'
      : status === 'drafting'
        ? 'Planning'
        : status === 'approved'
          ? 'Plan approved'
          : status === 'abandoned'
            ? 'Plan abandoned'
            : status === 'stale'
              ? 'Plan stale'
              : 'Start plan';
  const unsupported = !hasPlan && !plan.loading && !plan.providerSupportsHostEnforcedPlanning;
  const reason = unsupported
    ? 'Planning is unavailable because the current provider cannot enforce Gosling planning restrictions.'
    : status === 'drafting'
      ? 'Planning — can inspect this workspace but cannot edit or run commands.'
      : label;

  return (
    <Button
      type="button"
      size="xs"
      variant={active ? 'secondary' : 'ghost'}
      title={reason}
      aria-label={reason}
      aria-pressed={active}
      disabled={disabled || plan.loading || unsupported}
      onClick={hasPlan ? onOpen : onStart}
      data-testid="plan-status-control"
    >
      <ScrollText className="size-3.5" />
      <span className="hidden sm:inline">{label}</span>
    </Button>
  );
}
