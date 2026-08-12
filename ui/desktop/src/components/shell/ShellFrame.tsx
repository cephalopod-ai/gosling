import type { ReactNode } from 'react';
import { cn } from '../../utils';

export interface ShellFrameProps {
  title: string;
  subtitle?: string;
  status?: ReactNode;
  navigation?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}

export const ShellFrame = ({
  title,
  subtitle,
  status,
  navigation,
  actions,
  children,
  className,
}: ShellFrameProps) => (
  <div className={cn('flex h-screen min-h-0 flex-col bg-background-default', className)}>
    <header className="flex min-h-14 items-center gap-4 border-b px-5 py-3">
      <div className="min-w-0 flex-1">
        <h1 className="truncate text-base font-semibold text-text-primary">{title}</h1>
        {subtitle ? <p className="truncate text-xs text-text-secondary">{subtitle}</p> : null}
      </div>
      {status ? <div aria-label="Shell status">{status}</div> : null}
      {actions ? <div className="flex items-center gap-2">{actions}</div> : null}
    </header>
    <div className="flex min-h-0 flex-1">
      {navigation ? (
        <nav className="w-60 shrink-0 overflow-y-auto border-r p-3" aria-label="Shell navigation">
          {navigation}
        </nav>
      ) : null}
      <main className="min-w-0 flex-1 overflow-y-auto p-5">{children}</main>
    </div>
  </div>
);
