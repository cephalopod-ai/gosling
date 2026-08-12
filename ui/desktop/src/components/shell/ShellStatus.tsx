import { cn } from '../../utils';

export type ShellStatusTone = 'ready' | 'busy' | 'degraded' | 'offline';

const toneClasses: Record<ShellStatusTone, string> = {
  ready: 'bg-green-500',
  busy: 'bg-blue-500',
  degraded: 'bg-amber-500',
  offline: 'bg-red-500',
};

export interface ShellStatusProps {
  label: string;
  tone?: ShellStatusTone;
  className?: string;
}

export const ShellStatus = ({ label, tone = 'ready', className }: ShellStatusProps) => (
  <span
    className={cn(
      'inline-flex items-center gap-2 rounded-full border px-2.5 py-1 text-xs text-text-secondary',
      className
    )}
  >
    <span className={cn('size-2 rounded-full', toneClasses[tone])} aria-hidden="true" />
    {label}
  </span>
);
