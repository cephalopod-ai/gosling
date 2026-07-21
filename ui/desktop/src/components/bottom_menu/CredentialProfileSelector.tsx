import { Check, KeyRound, Settings2, TriangleAlert } from 'lucide-react';
import { useState } from 'react';
import { useWorkspace } from '../../contexts/WorkspaceContext';
import { defineMessages, useIntl } from '../../i18n';
import { CredentialProfileManagerDialog } from '../workspaces/CredentialProfileManagerDialog';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/Tooltip';
import { cn } from '../../utils';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';

const i18n = defineMessages({
  availableProfiles: {
    id: 'credentialProfileSelector.availableProfiles',
    defaultMessage: 'Available credential profiles',
  },
  credentialForChat: {
    id: 'credentialProfileSelector.credentialForChat',
    defaultMessage: 'Credential for this chat',
  },
  manageProfiles: {
    id: 'credentialProfileSelector.manageProfiles',
    defaultMessage: 'Manage credential profiles',
  },
  missingProfile: {
    id: 'credentialProfileSelector.missingProfile',
    defaultMessage: 'Profile unavailable',
  },
  newChats: {
    id: 'credentialProfileSelector.newChats',
    defaultMessage: 'New chats',
  },
  noProfile: {
    id: 'credentialProfileSelector.noProfile',
    defaultMessage: 'No credential',
  },
  noProfilePinned: {
    id: 'credentialProfileSelector.noProfilePinned',
    defaultMessage: 'No credential profile is pinned',
  },
  pinned: {
    id: 'credentialProfileSelector.pinned',
    defaultMessage: 'Pinned',
  },
  pinnedExplanation: {
    id: 'credentialProfileSelector.pinnedExplanation',
    defaultMessage: 'Credentials are pinned when a chat starts. Start a new chat to use another profile.',
  },
  tooltip: {
    id: 'credentialProfileSelector.tooltip',
    defaultMessage: 'Credential for this chat: {profile}',
  },
});

interface CredentialProfileSelectorProps {
  credentialProfileId?: string | null;
  credentialProfileName?: string | null;
  compact?: boolean;
  surface?: 'composer' | 'header';
}

export function CredentialProfileSelector({
  credentialProfileId,
  credentialProfileName,
  compact = false,
  surface = 'composer',
}: CredentialProfileSelectorProps) {
  const intl = useIntl();
  const { credentialProfiles } = useWorkspace();
  const [managerOpen, setManagerOpen] = useState(false);
  const currentProfile = credentialProfiles.find((profile) => profile.id === credentialProfileId);
  const savedProfileName = credentialProfileName?.trim() || null;
  const displayName =
    currentProfile?.name ?? savedProfileName ?? intl.formatMessage(i18n.noProfile);
  const missingProfile = Boolean(credentialProfileId && !currentProfile);
  const tooltip = intl.formatMessage(i18n.tooltip, { profile: displayName });

  return (
    <>
      <TooltipProvider>
        <Tooltip>
          <DropdownMenu>
            <TooltipTrigger asChild>
              <DropdownMenuTrigger asChild>
                <button
                  type="button"
                  aria-label={tooltip}
                  className={cn(
                    'z-[100] flex max-w-[220px] items-center text-xs text-text-primary/70 transition-colors hover:cursor-pointer hover:text-text-primary [&>svg]:size-4',
                    surface === 'header'
                      ? 'no-drag rounded-full border border-border-primary bg-background-secondary px-2 py-1 hover:bg-background-tertiary'
                      : 'pl-1'
                  )}
                >
                  <KeyRound className={compact ? '' : 'mr-1'} />
                  {!compact && <span className="truncate">{displayName}</span>}
                  {missingProfile && (
                    <TriangleAlert className="ml-1 shrink-0 text-amber-500" aria-hidden="true" />
                  )}
                </button>
              </DropdownMenuTrigger>
            </TooltipTrigger>
            <DropdownMenuContent
              className="z-[300] w-80"
              side={surface === 'header' ? 'bottom' : 'top'}
              align={surface === 'header' ? 'end' : 'start'}
            >
              <DropdownMenuLabel>{intl.formatMessage(i18n.credentialForChat)}</DropdownMenuLabel>
              <div className="flex items-center gap-2 rounded-md px-2 py-2 text-sm">
                {missingProfile ? (
                  <TriangleAlert className="size-4 shrink-0 text-amber-500" />
                ) : (
                  <KeyRound className="size-4 shrink-0 text-text-secondary" />
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{displayName}</div>
                  <div className="truncate text-xs text-text-secondary">
                    {currentProfile
                      ? `${currentProfile.providerOrServiceId} · ${currentProfile.status.replace(/_/g, ' ')}`
                      : missingProfile
                        ? intl.formatMessage(i18n.missingProfile)
                        : intl.formatMessage(i18n.noProfilePinned)}
                  </div>
                </div>
                {!missingProfile && credentialProfileId && (
                  <span className="flex items-center gap-1 text-xs text-text-secondary">
                    <Check className="size-3" />
                    {intl.formatMessage(i18n.pinned)}
                  </span>
                )}
              </div>
              <p className="px-2 pb-1 text-xs text-text-secondary">
                {intl.formatMessage(i18n.pinnedExplanation)}
              </p>

              {credentialProfiles.length > 0 && (
                <>
                  <DropdownMenuSeparator />
                  <DropdownMenuLabel>
                    {intl.formatMessage(i18n.availableProfiles)}
                  </DropdownMenuLabel>
                  {credentialProfiles.map((profile) => {
                    const isCurrent = profile.id === credentialProfileId;
                    return (
                      <div
                        key={profile.id}
                        className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm"
                      >
                        <KeyRound className="size-4 shrink-0 text-text-secondary" />
                        <div className="min-w-0 flex-1">
                          <div className="truncate">{profile.name}</div>
                          <div className="truncate text-xs text-text-secondary">
                            {profile.providerOrServiceId} · {profile.status.replace(/_/g, ' ')}
                          </div>
                        </div>
                        {isCurrent ? (
                          <Check
                            className="size-4 shrink-0"
                            aria-label={intl.formatMessage(i18n.pinned)}
                          />
                        ) : (
                          <span className="shrink-0 text-xs text-text-secondary">
                            {intl.formatMessage(i18n.newChats)}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </>
              )}

              <DropdownMenuSeparator />
              <DropdownMenuItem onSelect={() => setManagerOpen(true)}>
                <Settings2 className="size-4" />
                {intl.formatMessage(i18n.manageProfiles)}
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <TooltipContent side="top">{tooltip}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
      <CredentialProfileManagerDialog open={managerOpen} onOpenChange={setManagerOpen} />
    </>
  );
}
