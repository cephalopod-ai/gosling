import { Activity, AlertCircle, CheckCircle2, Loader2 } from 'lucide-react';
import { defineMessages, useIntl } from '../i18n';
import type { RunStatus } from '../hooks/useRunStatus';
import { isBackgroundTaskActive, type BackgroundTaskState } from '../acp/backgroundTasks';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from './ui/dropdown-menu';
import { cn } from '../utils';

const i18n = defineMessages({
  activity: { id: 'runStatus.activity', defaultMessage: 'Activity' },
  checking: { id: 'runStatus.checking', defaultMessage: 'Checking status…' },
  unavailable: { id: 'runStatus.unavailable', defaultMessage: 'Status unavailable' },
  quiet: { id: 'runStatus.quiet', defaultMessage: 'No recent progress' },
  waiting: { id: 'runStatus.waiting', defaultMessage: 'Waiting for you' },
  background: {
    id: 'runStatus.background',
    defaultMessage: '{count, plural, one {# background agent} other {# background agents}}',
  },
  finished: { id: 'runStatus.finished', defaultMessage: 'Background work finished' },
  failed: { id: 'runStatus.failed', defaultMessage: 'Background task failed' },
  cadence: { id: 'runStatus.cadence', defaultMessage: 'Status checks every 15 minutes' },
  backend: {
    id: 'runStatus.backend',
    defaultMessage: 'Backend answered the last check. This alone does not confirm progress.',
  },
  lastCheck: { id: 'runStatus.lastCheck', defaultMessage: 'Last check: {time}' },
  noCheck: { id: 'runStatus.noCheck', defaultMessage: 'No verified status check yet' },
  lastOutput: { id: 'runStatus.lastOutput', defaultMessage: 'Last output: {time}' },
  noOutput: { id: 'runStatus.noOutput', defaultMessage: 'No output received yet' },
  quietHint: {
    id: 'runStatus.quietHint',
    defaultMessage:
      'No output for at least 15 minutes. Work may be slow or stalled. Review activity before deciding whether to interrupt the task.',
  },
  unavailableHint: {
    id: 'runStatus.unavailableHint',
    defaultMessage: 'Liveness could not be verified. Check again or review the task in its chat.',
  },
  checkNow: { id: 'runStatus.checkNow', defaultMessage: 'Check now' },
  openTask: { id: 'runStatus.openTask', defaultMessage: 'Open agent chat' },
  running: { id: 'runStatus.running', defaultMessage: 'Running' },
  completed: { id: 'runStatus.completed', defaultMessage: 'Completed' },
  taskFailed: { id: 'runStatus.taskFailed', defaultMessage: 'Failed' },
  cancelled: { id: 'runStatus.cancelled', defaultMessage: 'Cancelled' },
  unknown: { id: 'runStatus.unknown', defaultMessage: 'Unverified' },
  turns: { id: 'runStatus.turns', defaultMessage: '{count, plural, one {# turn} other {# turns}}' },
  idle: { id: 'runStatus.idle', defaultMessage: 'Idle: {count} min' },
});

const TASK_MESSAGES: Record<BackgroundTaskState, keyof typeof i18n> = {
  running: 'running',
  quiet: 'quiet',
  completed: 'completed',
  failed: 'taskFailed',
  cancelled: 'cancelled',
  unknown: 'unknown',
};

export function RunStatusControl({
  status,
  onOpenTask,
}: {
  status: RunStatus;
  onOpenTask: (taskId: string) => void;
}) {
  const intl = useIntl();
  if (!status.visible) return null;
  const activeTasks = status.tasks.filter(isBackgroundTaskActive);
  const failed = status.tasks.some((task) => task.state === 'failed');
  const warning = status.unavailable || status.quiet || failed;
  const relativeTime = (time: number) =>
    intl.formatRelativeTime(-Math.floor(Math.max(0, status.now - time) / 60_000), 'minute', {
      numeric: 'auto',
    });
  const label = status.unavailable
    ? intl.formatMessage(i18n.unavailable)
    : failed
      ? intl.formatMessage(i18n.failed)
      : status.quiet
        ? intl.formatMessage(i18n.quiet)
        : status.checking && !status.result
          ? intl.formatMessage(i18n.checking)
          : status.waitingForUser && activeTasks.length === 0
            ? intl.formatMessage(i18n.waiting)
            : activeTasks.length > 0
              ? intl.formatMessage(i18n.background, { count: activeTasks.length })
              : status.foregroundActive
                ? intl.formatMessage(i18n.activity)
                : intl.formatMessage(i18n.finished);
  const Icon = warning
    ? AlertCircle
    : status.checking
      ? Loader2
      : status.foregroundActive || activeTasks.length
        ? Activity
        : CheckCircle2;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className={cn(
            'pointer-events-auto no-drag flex items-center gap-1.5 rounded-full border bg-background-primary px-2.5 py-1 text-xs',
            warning
              ? 'border-border-warning text-text-warning'
              : 'border-border-primary text-text-secondary'
          )}
          aria-label={label}
          data-run-status={
            status.unavailable
              ? 'unavailable'
              : failed
                ? 'failed'
                : status.quiet
                  ? 'quiet'
                  : status.foregroundActive || activeTasks.length
                    ? 'active'
                    : 'finished'
          }
        >
          <Icon className={cn('size-3.5', status.checking && !warning && 'animate-spin')} />
          <span role="status">{label}</span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-80 max-w-[calc(100vw-2rem)]">
        <div className="space-y-1 px-3 py-2 text-xs text-text-secondary">
          <p className="font-medium text-text-primary">{intl.formatMessage(i18n.cadence)}</p>
          <p>
            {status.result
              ? intl.formatMessage(i18n.lastCheck, { time: relativeTime(status.result.checkedAt) })
              : intl.formatMessage(i18n.noCheck)}
          </p>
          {status.foregroundActive && (
            <p>
              {status.lastActivityAt !== null
                ? intl.formatMessage(i18n.lastOutput, { time: relativeTime(status.lastActivityAt) })
                : intl.formatMessage(i18n.noOutput)}
            </p>
          )}
          {status.result?.backendResponded && <p>{intl.formatMessage(i18n.backend)}</p>}
          {status.quiet && (
            <p className="text-text-warning">{intl.formatMessage(i18n.quietHint)}</p>
          )}
          {status.unavailable && (
            <p className="text-text-warning">{intl.formatMessage(i18n.unavailableHint)}</p>
          )}
          {status.result?.error && (
            <p className="line-clamp-2 break-words">{status.result.error}</p>
          )}
        </div>
        {status.tasks.length > 0 && (
          <>
            <DropdownMenuSeparator />
            <div className="max-h-64 overflow-y-auto px-3 py-2 space-y-3">
              {status.tasks.map((task) => (
                <div key={task.id} className="text-xs space-y-1">
                  <p className="truncate font-medium text-text-primary" title={task.description}>
                    {task.description}
                  </p>
                  <p className="text-text-secondary">
                    {task.id} · {intl.formatMessage(i18n[TASK_MESSAGES[task.state]])}
                  </p>
                  <p className="text-text-secondary">
                    {task.turns !== null && intl.formatMessage(i18n.turns, { count: task.turns })}
                    {task.turns !== null && task.idleMs !== null && ' · '}
                    {task.idleMs !== null &&
                      intl.formatMessage(i18n.idle, { count: Math.floor(task.idleMs / 60_000) })}
                  </p>
                  {task.error && (
                    <p className="line-clamp-2 break-words text-text-warning">{task.error}</p>
                  )}
                  <button
                    type="button"
                    onClick={() => onOpenTask(task.id)}
                    className="text-text-secondary underline hover:text-text-primary"
                  >
                    {intl.formatMessage(i18n.openTask)}
                  </button>
                </div>
              ))}
            </div>
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem
          disabled={status.checking || (!status.foregroundActive && activeTasks.length === 0)}
          onSelect={(event) => {
            event.preventDefault();
            void status.checkNow();
          }}
        >
          {status.checking ? intl.formatMessage(i18n.checking) : intl.formatMessage(i18n.checkNow)}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
