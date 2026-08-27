import React from 'react';
import { Check, Clock3, LoaderCircle, X } from 'lucide-react';
import { defineMessages, useIntl } from '../i18n';
import { cn } from '../utils';

const i18n = defineMessages({
  toolStatus: {
    id: 'toolCallStatusIndicator.toolStatus',
    defaultMessage: 'Tool status: {status}',
  },
});

export type ToolCallStatus = 'pending' | 'loading' | 'success' | 'error';

interface ToolCallStatusIndicatorProps {
  status: ToolCallStatus;
  className?: string;
}

export const ToolCallStatusIndicator: React.FC<ToolCallStatusIndicatorProps> = ({
  status,
  className,
}) => {
  const intl = useIntl();
  const getStatusStyles = () => {
    switch (status) {
      case 'success':
        return 'bg-green-500';
      case 'error':
        return 'bg-red-500';
      case 'loading':
        return 'bg-yellow-500 animate-pulse';
      case 'pending':
      default:
        return 'bg-gray-400';
    }
  };
  const StatusIcon = {
    pending: Clock3,
    loading: LoaderCircle,
    success: Check,
    error: X,
  }[status];

  return (
    <div
      className={cn(
        'absolute -top-1 -right-1 flex h-3 w-3 items-center justify-center rounded-full border border-border-primary text-white',
        getStatusStyles(),
        className
      )}
      aria-label={intl.formatMessage(i18n.toolStatus, { status })}
      data-status={status}
    >
      <StatusIcon
        className={cn('h-2 w-2 stroke-[3]', status === 'loading' && 'animate-spin')}
        aria-hidden
      />
    </div>
  );
};

/**
 * Wrapper component that adds a status indicator to a tool icon
 */
interface ToolIconWithStatusProps {
  ToolIcon: React.ComponentType<{ className?: string }>;
  status: ToolCallStatus;
  className?: string;
}

export const ToolIconWithStatus: React.FC<ToolIconWithStatusProps> = ({
  ToolIcon,
  status,
  className,
}) => {
  return (
    <div className={cn('relative inline-block', className)}>
      <ToolIcon className="w-3 h-3 flex-shrink-0" />
      <ToolCallStatusIndicator status={status} />
    </div>
  );
};
