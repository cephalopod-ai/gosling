import BottomMenuAlertPopover from './BottomMenuAlertPopover';
import { Alert } from '../alerts';

interface ContextWindowIndicatorProps {
  totalTokens: number;
  tokenLimit: number;
  alerts: Alert[];
  estimated?: boolean;
  lastRequestTokens?: number;
  // Persistent-session CLI/ACP providers (e.g. Claude Code) manage their own
  // context — Gosling can't compact it and this number isn't heading toward a
  // Gosling-triggered failure, so it shouldn't wear the same orange/red
  // "action needed" escalation as a context Gosling actually controls.
  managesOwnContext?: boolean;
}

const formatTokenCount = (count: number): string => {
  if (count >= 1_000_000) return `${Math.round(count / 1_000_000)}M`;
  if (count >= 1_000) return `${Math.round(count / 1_000)}k`;
  return count.toString();
};

const getProgressColor = (percentage: number, managesOwnContext: boolean): string => {
  if (managesOwnContext) return 'text-text-primary/70';
  if (percentage <= 75) return 'text-text-primary/70';
  if (percentage <= 90) return 'text-orange-500';
  return 'text-red-500';
};

export function ContextWindowIndicator({
  totalTokens,
  tokenLimit,
  alerts,
  estimated = false,
  lastRequestTokens,
  managesOwnContext = false,
}: ContextWindowIndicatorProps) {
  if (!tokenLimit) return null;

  const percentage = Math.round((totalTokens / tokenLimit) * 100);
  const colorClass = getProgressColor(percentage, managesOwnContext);
  const formattedUsage = `${formatTokenCount(totalTokens)} of ${formatTokenCount(tokenLimit)} effective context limit`;
  const usageLabel = managesOwnContext
    ? `Context managed by the connected CLI tool. Last request: ${formattedUsage}`
    : estimated
      ? `Active context estimate: ${formattedUsage}. Last model request: ${formatTokenCount(lastRequestTokens ?? totalTokens)}`
      : `Last model request: ${formattedUsage}`;
  const visibleUsage = `${formatTokenCount(totalTokens)}${estimated && !managesOwnContext ? ' est' : ' req'} / ${formatTokenCount(tokenLimit)}`;

  return (
    <div className="flex items-center h-full">
      <BottomMenuAlertPopover alerts={alerts}>
        <span
          aria-label={usageLabel}
          title={usageLabel}
          className={`text-xs font-mono ${colorClass}`}
        >
          {visibleUsage}
        </span>
      </BottomMenuAlertPopover>
    </div>
  );
}
