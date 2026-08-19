import type { GoslingShellAPI } from '../shell/preloadApi';
import type { ShellOperationFailure, ShellRecoveryAction } from '../shell/operationFailure';

const FAILURE_CODES = new Set<ShellOperationFailure['code']>([
  'CAPABILITY_UNAVAILABLE',
  'CREDENTIAL_REQUIRED',
  'DIRECTORY_REQUIRED',
  'INTERACTION_PENDING',
  'INVALID_INPUT',
  'INVALID_REQUEST',
  'OPERATION_FAILED',
  'RUNTIME_UNAVAILABLE',
  'SESSION_BUSY',
  'SESSION_UNAVAILABLE',
  'SETTINGS_RECOVERY_REQUIRED',
  'STALE_REQUEST',
]);

const RECOVERY_ACTIONS = new Set<ShellRecoveryAction>([
  'none',
  'retry',
  'refresh',
  'choose_directory',
  'select_credential',
  'review_session',
  'reset_settings',
  'open_gosling',
  'save_diagnostics',
  'restart',
]);

const UNKNOWN_FAILURE: ShellOperationFailure = Object.freeze({
  code: 'OPERATION_FAILED',
  message: 'The shell could not complete this action.',
  retrySafe: false,
  recovery: 'save_diagnostics',
  preservesDraft: false,
});

/**
 * The preload already decodes main's encoded failures, so a rejection is normally a
 * `ShellOperationFailure`. Anything else is reduced to `OPERATION_FAILED` rather than surfaced,
 * because a raw backend error string must never reach the DOM.
 */
export function asOperationFailure(error: unknown): ShellOperationFailure {
  if (!error || typeof error !== 'object' || Array.isArray(error)) return UNKNOWN_FAILURE;
  const value = error as Record<string, unknown>;
  if (
    typeof value.code !== 'string' ||
    !FAILURE_CODES.has(value.code as ShellOperationFailure['code']) ||
    typeof value.message !== 'string' ||
    value.message.length === 0 ||
    value.message.length > 512 ||
    typeof value.retrySafe !== 'boolean' ||
    typeof value.recovery !== 'string' ||
    !RECOVERY_ACTIONS.has(value.recovery as ShellRecoveryAction) ||
    typeof value.preservesDraft !== 'boolean'
  ) {
    return UNKNOWN_FAILURE;
  }
  return {
    code: value.code as ShellOperationFailure['code'],
    message: value.message,
    retrySafe: value.retrySafe,
    recovery: value.recovery as ShellRecoveryAction,
    preservesDraft: value.preservesDraft,
  };
}

export function resolveShellApi(): GoslingShellAPI {
  const api = globalThis.window?.goslingShell;
  if (!api) throw new Error('the Gosling shell preload API is unavailable');
  return api;
}
