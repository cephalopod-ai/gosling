import { useState } from 'react';
import { ChevronRight, ListTree } from 'lucide-react';
import { defineMessages, useIntl } from '../i18n';
import { cn } from '../utils';
import { ToolCallStatusIndicator, type ToolCallStatus } from './ToolCallStatusIndicator';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from './ui/collapsible';

const i18n = defineMessages({
  activityCount: {
    id: 'toolCallWithResponse.activityCount',
    defaultMessage: 'Activity ({count})',
  },
});

interface ToolActivityGroupProps {
  children: React.ReactNode;
  count: number;
  hasPendingApproval: boolean;
  status: ToolCallStatus;
  className?: string;
}

export default function ToolActivityGroup({
  children,
  count,
  hasPendingApproval,
  status,
  className,
}: ToolActivityGroupProps) {
  const intl = useIntl();
  const [isExpanded, setIsExpanded] = useState(false);
  const isOpen = hasPendingApproval || isExpanded;
  const label = intl.formatMessage(i18n.activityCount, { count });

  return (
    <Collapsible
      open={isOpen}
      onOpenChange={(open) => {
        if (!hasPendingApproval) setIsExpanded(open);
      }}
      className={cn('w-full', className)}
    >
      <CollapsibleTrigger
        aria-label={label}
        className="group flex w-[90%] items-center justify-between rounded-lg border border-border-primary px-4 py-3 text-sm text-text-primary transition-colors hover:bg-background-secondary"
      >
        <span className="flex min-w-0 items-center gap-2">
          <span className="relative shrink-0">
            <ListTree className="h-4 w-4" />
            <ToolCallStatusIndicator status={status} />
          </span>
          <span className="truncate">{label}</span>
        </span>
        <ChevronRight
          className={cn(
            'h-4 w-4 shrink-0 opacity-70 transition-transform group-hover:opacity-100',
            isOpen && 'rotate-90'
          )}
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-3 flex flex-col">{children}</CollapsibleContent>
    </Collapsible>
  );
}
