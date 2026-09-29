/**
 * Shared constants and utilities for extension error handling
 */

import type { ExtensionLoadResult } from '../types/extensions';
import { toastService, ExtensionLoadingStatus } from '../toasts';

export const MAX_ERROR_MESSAGE_LENGTH = 200;

/**
 * Creates recovery hints for the "Ask gosling" feature when extension loading fails
 */
export function createExtensionRecoverHints(errorMsg: string): string {
  return (
    `Explain the following error: ${errorMsg}. ` +
    'This happened while trying to install an extension. Look out for issues where the ' +
    "extension attempted to execute something incorrectly, didn't exist, or there was trouble with " +
    'the network configuration - VPNs like WARP often cause issues.'
  );
}

/**
 * The cause shown in a toast: the backend's message, shortened when long (the full text stays
 * available through Copy error). Replacing long messages with the fallback hid every real cause,
 * since the backend's start-up errors name the command or URL and the reason.
 */
export function formatExtensionErrorMessage(
  errorMsg: string,
  fallback: string = 'Failed to add extension'
): string {
  const message = errorMsg.trim();
  if (!message) {
    return fallback;
  }
  return message.length <= MAX_ERROR_MESSAGE_LENGTH
    ? message
    : `${message.slice(0, MAX_ERROR_MESSAGE_LENGTH - 1).trimEnd()}…`;
}

/**
 * Shows toast notifications for extension load results.
 * Uses grouped toast for multiple extensions, individual error toast for single failed extension.
 * @param results - Array of extension load results from the backend
 */
export function showExtensionLoadResults(results: ExtensionLoadResult[] | null | undefined): void {
  if (!results || results.length === 0) {
    return;
  }

  const failedExtensions = results.filter((r) => !r.success);

  if (results.length === 1 && failedExtensions.length === 1) {
    const failed = failedExtensions[0];
    const errorMsg = failed.error || 'Unknown error';
    const recoverHints = createExtensionRecoverHints(errorMsg);
    const displayMsg = formatExtensionErrorMessage(errorMsg, 'Failed to load extension');

    toastService.error({
      title: failed.name,
      msg: displayMsg,
      traceback: errorMsg,
      recoverHints,
    });
    return;
  }

  const extensionStatuses: ExtensionLoadingStatus[] = results.map((r) => {
    const errorMsg = r.error || 'Unknown error';
    return {
      name: r.name,
      status: r.success ? 'success' : 'error',
      error: r.success ? undefined : errorMsg,
      recoverHints: r.success ? undefined : createExtensionRecoverHints(errorMsg),
    };
  });

  toastService.extensionLoading(extensionStatuses, results.length, true);
}
