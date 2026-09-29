import { toastService } from '../../../toasts';
import type { ExtensionConfig } from '../../../types/extensions';
import { addSessionExtension, removeSessionExtension } from '../../../acp/session-extensions';
import { describeAcpCause } from '../../../acp/errors';
import {
  createExtensionRecoverHints,
  formatExtensionErrorMessage,
} from '../../../utils/extensionErrorUtils';
import {
  clearExtensionFailure,
  recordExtensionFailure,
} from '../../../utils/extensionLoadFailures';

export async function addToAgent(
  extensionConfig: ExtensionConfig,
  sessionId: string,
  showToast: boolean
) {
  const extensionName = extensionConfig.name;
  let toastId = showToast
    ? toastService.loading({
        title: extensionName,
        msg: `adding ${extensionName} extension...`,
      })
    : 0;

  try {
    await addSessionExtension(sessionId, extensionConfig);
    clearExtensionFailure(sessionId, extensionName);
    if (showToast) {
      toastService.dismiss(toastId);
      toastService.success({
        title: extensionName,
        msg: `Successfully added extension`,
      });
    }
  } catch (error) {
    const errMsg = describeAcpCause(error);
    recordExtensionFailure(sessionId, extensionName, errMsg);
    if (showToast) {
      toastService.dismiss(toastId);
      const recoverHints = createExtensionRecoverHints(errMsg);
      const msg = formatExtensionErrorMessage(errMsg, 'Failed to add extension');
      toastService.error({
        title: extensionName,
        msg: msg,
        traceback: errMsg,
        recoverHints,
      });
    }
    throw error;
  }
}

export async function removeFromAgent(
  extensionName: string,
  sessionId: string,
  showToast: boolean
) {
  let toastId = showToast
    ? toastService.loading({
        title: extensionName,
        msg: `Removing ${extensionName} extension...`,
      })
    : 0;

  try {
    await removeSessionExtension(sessionId, extensionName);
    clearExtensionFailure(sessionId, extensionName);

    if (showToast) {
      toastService.dismiss(toastId);
      toastService.success({
        title: extensionName,
        msg: `Successfully removed extension`,
      });
    }
  } catch (error) {
    if (showToast) {
      toastService.dismiss(toastId);
      const errMsg = describeAcpCause(error);
      const msg = formatExtensionErrorMessage(errMsg, 'Failed to remove extension');
      toastService.error({
        title: extensionName,
        msg: msg,
        traceback: errMsg,
      });
    }
    throw error;
  }
}

export function sanitizeName(name: string) {
  return name.toLowerCase().replace(/-/g, '').replace(/_/g, '').replace(/\s/g, '');
}
