import type { ReactNode } from 'react';
import { cn } from '../utils';

interface ChatHeaderBarProps {
  title: ReactNode;
  actions: ReactNode;
  reserveNavToggle: boolean;
}

// The title and the actions share one flex row so they can never overlap: the
// actions keep at most half the row, and the title column takes the rest and
// truncates. When the sidebar is collapsed the app-level navigation toggle
// floats over the left edge of this panel, so the row starts after it.
export function ChatHeaderBar({ title, actions, reserveNavToggle }: ChatHeaderBarProps) {
  return (
    <div
      data-testid="chat-header-bar"
      className={cn(
        'pointer-events-none absolute inset-x-0 top-[14px] z-[60] flex items-start gap-2 pr-4',
        reserveNavToggle ? 'pl-36' : 'pl-4'
      )}
    >
      <div data-testid="chat-header-title" className="flex min-w-0 flex-1 justify-center">
        {title}
      </div>
      <div
        data-testid="chat-header-actions"
        className="flex min-w-0 max-w-1/2 flex-col items-end gap-2"
      >
        {actions}
      </div>
    </div>
  );
}
