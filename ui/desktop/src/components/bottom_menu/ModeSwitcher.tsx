import React, { useState } from 'react';
import { Check, Hand, MessageSquareText, Sparkles, Zap } from 'lucide-react';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/Tooltip';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { toast } from 'react-toastify';
import { defineMessages, useIntl } from '../../i18n';
import { all_gosling_modes } from '../settings/mode/ModeSelectionItem';
import type { GoslingMode } from '../../types/session';

const i18n = defineMessages({
  failedToUpdateMode: {
    id: 'modeSwitcher.failedToUpdateMode',
    defaultMessage: 'Failed to update session mode',
  },
  sessionMode: {
    id: 'modeSwitcher.sessionMode',
    defaultMessage: 'Mode for this session',
  },
});

const MODE_ICONS: Record<GoslingMode, React.ComponentType<{ className?: string }>> = {
  auto: Zap,
  approve: Hand,
  smart_approve: Sparkles,
  chat: MessageSquareText,
};

interface ModeSwitcherProps {
  sessionId: string | undefined;
  mode: GoslingMode | undefined;
  disabled?: boolean;
  onModeChange?: (newMode: GoslingMode) => Promise<void> | void;
}

export const ModeSwitcher: React.FC<ModeSwitcherProps> = ({
  sessionId,
  mode,
  disabled = false,
  onModeChange,
}) => {
  const intl = useIntl();
  const [isMenuOpen, setIsMenuOpen] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);

  if (!sessionId || !mode) {
    return null;
  }

  const currentModeEntry = all_gosling_modes.find((m) => m.key === mode);
  const CurrentIcon = MODE_ICONS[mode] ?? Sparkles;

  const handleSelectMode = async (newMode: GoslingMode) => {
    setIsMenuOpen(false);
    if (newMode === mode || disabled || isUpdating) {
      return;
    }

    setIsUpdating(true);
    try {
      await onModeChange?.(newMode);
    } catch (error) {
      console.error('[ModeSwitcher] Failed to update session mode:', error);
      toast.error(intl.formatMessage(i18n.failedToUpdateMode));
    } finally {
      setIsUpdating(false);
    }
  };

  return (
    <TooltipProvider>
      <Tooltip>
        <DropdownMenu open={isMenuOpen} onOpenChange={setIsMenuOpen}>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <button
                className={`z-[100] ${disabled || isUpdating ? 'opacity-50' : 'hover:cursor-pointer hover:text-text-primary'} text-text-primary/70 text-xs flex items-center transition-colors pl-1 [&>svg]:size-4`}
                disabled={disabled || isUpdating}
              >
                <CurrentIcon className="mr-1" />
                <div className="max-w-[120px] truncate">
                  {currentModeEntry ? intl.formatMessage(currentModeEntry.labelDescriptor) : mode}
                </div>
              </button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <DropdownMenuContent className="w-80" side="top" align="start">
            <DropdownMenuLabel>{intl.formatMessage(i18n.sessionMode)}</DropdownMenuLabel>
            {all_gosling_modes.map((modeEntry) => {
              const ModeIcon = MODE_ICONS[modeEntry.key as GoslingMode] ?? Sparkles;
              return (
                <DropdownMenuItem
                  key={modeEntry.key}
                  onSelect={() => void handleSelectMode(modeEntry.key as GoslingMode)}
                >
                  <ModeIcon className="mr-2 h-4 w-4 shrink-0" />
                  <div className="flex flex-col min-w-0">
                    <span>{intl.formatMessage(modeEntry.labelDescriptor)}</span>
                    <span className="text-xs text-text-secondary truncate">
                      {intl.formatMessage(modeEntry.descriptionDescriptor)}
                    </span>
                  </div>
                  {modeEntry.key === mode && <Check className="ml-auto h-4 w-4 shrink-0" />}
                </DropdownMenuItem>
              );
            })}
          </DropdownMenuContent>
        </DropdownMenu>
        <TooltipContent side="top">{intl.formatMessage(i18n.sessionMode)}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
};
