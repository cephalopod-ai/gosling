import type { ShellLifecycleStateName } from '../shell/lifecycle';
import type { ShellSettingsRecovery } from '../shell/localSettings';
import type { ShellCredentialSnapshot } from '../shell/credentialController';
import type { ShellDirectorySnapshot } from '../shell/directoryController';

/**
 * Every user-facing string lives here so the wording accepted in Gate 1 is reviewable in one place
 * and cannot drift per component. Product name is injected, never hard-coded.
 */

export interface LifecycleCopy {
  heading: string;
  detail: string;
}

export function lifecycleCopy(state: ShellLifecycleStateName, name: string): LifecycleCopy {
  switch (state) {
    case 'booting':
      return {
        heading: `Starting ${name}…`,
        detail: 'This takes a moment while the backend comes up.',
      };
    case 'validating':
      return {
        heading: 'Checking your setup…',
        detail: 'Confirming this build matches the Gosling core it shipped with.',
      };
    case 'degraded':
      return {
        heading: `${name} started with a problem it couldn't work around`,
        detail:
          'Retrying restarts the shell. Your unsent message is kept; the current task is not.',
      };
    case 'relink_required':
      return {
        heading: "An account this shell needs isn't connected any more",
        detail: `Reconnect it in Gosling, then start ${name} again.`,
      };
    case 'incompatible':
      return {
        heading: `This version of ${name} doesn't match the Gosling core it shipped with`,
        detail:
          'Reinstalling the matching build is the only fix. Diagnostics will record the exact versions.',
      };
    case 'offline':
      return {
        heading: `${name} can't reach its backend`,
        detail: 'Nothing was lost. Retrying restarts the shell.',
      };
    case 'stopping':
      return { heading: 'Shutting down…', detail: 'Closing the session and stopping the backend.' };
    case 'stopped':
      return {
        heading: `${name} has stopped`,
        detail: 'Restarting begins a new session. Nothing is running in the background.',
      };
    case 'fatal':
      return {
        heading: `${name} hit a problem it can't recover from`,
        detail: 'Saving diagnostics will help work out why.',
      };
    default:
      return { heading: '', detail: '' };
  }
}

export function directoryCopy(directory: ShellDirectorySnapshot, name: string): LifecycleCopy {
  if (directory.state === 'missing') {
    return {
      heading: "The folder you used last time isn't there any more",
      detail:
        'It may have been moved, renamed, or on a drive that is not connected. Choose a folder to carry on.',
    };
  }
  if (directory.state === 'invalid') {
    return {
      heading: "That folder can't be used",
      detail: 'Choose a different folder to carry on.',
    };
  }
  return {
    heading: 'Choose a folder to work in',
    detail: `${name} works inside one folder you choose. It can read and change files there, and nowhere else.`,
  };
}

export function credentialCopy(credentials: ShellCredentialSnapshot, name: string): LifecycleCopy {
  if (credentials.selectionStatus === 'relink_required') {
    return {
      heading: 'That account needs reconnecting',
      detail: `${name} can use accounts but can't create or repair them. Reconnect it in Gosling, then come back.`,
    };
  }
  if (credentials.selectionStatus === 'missing') {
    return {
      heading: "That account doesn't exist any more",
      detail: 'Choose a different account, or reconnect the original one in Gosling.',
    };
  }
  if (credentials.catalogStatus === 'unavailable') {
    return {
      heading: 'Accounts are unavailable',
      detail: `${name} could not read the account list. Retrying restarts the shell.`,
    };
  }
  return {
    heading: 'Choose an account',
    detail: 'These are the accounts this shell is allowed to use. Secrets stay in Gosling.',
  };
}

export function settingsRecoveryCopy(recovery: ShellSettingsRecovery, name: string): string | null {
  switch (recovery.status) {
    case 'unsupported_schema':
      return `These settings were written by a newer version of ${name}, so changes won't be saved. This folder won't be remembered until settings are reset.`;
    case 'malformed':
      return `${name}'s local settings can't be read, so changes won't be saved. This folder won't be remembered until settings are reset.`;
    case 'unreadable':
      return `${name} doesn't have permission to read its own settings, so changes won't be saved. This folder won't be remembered until settings are reset.`;
    default:
      return null;
  }
}

export const COPY = {
  verified: 'verified',
  unverified: 'unverified',
  chooseFolder: 'Choose folder',
  changeFolder: 'Change folder',
  change: 'Change',
  chooseAccount: 'Choose account',
  useAccount: 'Use this account',
  noFolder: 'No folder chosen',
  noAccount: 'No account chosen',
  accountFixedByProduct: 'set by product',
  accountsUnavailable: 'Accounts unavailable',
  sessionsHeading: 'Recent tasks in this folder (up to 20)',
  sessionsEmpty: 'No tasks here yet.',
  startNewTask: 'Start new task',
  resumeTask: 'Resume',
  refreshSessions: 'Refresh',
  sessions: 'Sessions',
  settings: 'Settings',
  close: 'Close',
  resetSettings: 'Reset settings',
  continueWithoutSaving: 'Continue without saving',
  send: 'Send',
  stopTask: 'Stop',
  stopping: 'Stopping…',
  composerPlaceholder: 'Describe the task. This shell works only inside the folder you chose.',
  composerBlockedByInteraction: 'Respond to the pending request before continuing.',
  transcriptGap: "Part of this conversation isn't shown.",
  repair: 'Repair',
  resumeUncertain: 'The last request may not have finished. Check the result before repeating it.',
  historySeam: 'resumed here',
  allowOnce: 'Allow once',
  deny: 'Deny',
  submit: 'Submit',
  decline: 'Decline',
  cancel: 'Cancel',
  approve: 'Approve',
  reject: 'Reject',
  permissionHeading: 'Allow this action?',
  formHeading: 'The task needs some details',
  confirmHeading: (action: string) => `Confirm “${action}”?`,
  confirmDetail: (name: string) => `${name} can't undo this once it runs.`,
  moreWaiting: (count: number) => `${count} more waiting`,
  retry: 'Retry',
  restart: 'Restart',
  quit: 'Quit',
  saveDiagnostics: 'Save diagnostics',
  diagnosticsSaved: (fileName: string) => `Diagnostics saved as ${fileName}.`,
  openInGosling: 'Open in Gosling',
  handoffUnavailable: (name: string) =>
    `${name} can't open Gosling for you from here, because it has no live backend connection to prepare the handover. Open Gosling yourself, fix this there, then start ${name} again.`,
  handoffQuestion: 'Continue this task in Gosling.',
  openGosling: 'Open Gosling',
  handoffHeading: 'Continue this in Gosling',
  handoffDetail: (name: string) =>
    `${name} can't finish this here. Gosling can. This is what will be sent:`,
  provisioningHeading: "Some of this product's setup couldn't be applied.",
  moduleUnavailable: 'A module this product declared is not available.',
  adapterUnavailable: 'A backend this product declared has stopped responding.',
  refresh: 'Refresh',
  reviewRequest: 'Go to the pending request',
  theme: 'Theme',
  textSize: 'Text size',
  rememberedFolder: 'Remembered folder',
  preferredAccount: 'Preferred account',
  settingsReadOnly: 'read-only until reset',
  outputsHeading: 'Outputs',
  outputsEmpty: 'No durable outputs have been recorded for this session.',
  outputsTruncated: 'Showing the first recorded outputs.',
  libraryHeading: 'Library',
  libraryHint:
    'Select items to include with your next request. Paste an image anywhere in this panel.',
  libraryEmpty: 'No project or task references yet.',
  libraryAddFile: 'Add file',
  libraryAddText: 'Add text',
  librarySaveText: 'Save text',
} as const;
