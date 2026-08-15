export type ShellRecoveryAction =
  | 'none'
  | 'retry'
  | 'refresh'
  | 'choose_directory'
  | 'select_credential'
  | 'review_session'
  | 'reset_settings'
  | 'open_gosling'
  | 'save_diagnostics'
  | 'restart';

export interface ShellOperationFailure {
  code:
    | 'CAPABILITY_UNAVAILABLE'
    | 'CREDENTIAL_REQUIRED'
    | 'DIRECTORY_REQUIRED'
    | 'INTERACTION_PENDING'
    | 'INVALID_INPUT'
    | 'INVALID_REQUEST'
    | 'OPERATION_FAILED'
    | 'RUNTIME_UNAVAILABLE'
    | 'SESSION_BUSY'
    | 'SESSION_UNAVAILABLE'
    | 'SETTINGS_RECOVERY_REQUIRED'
    | 'STALE_REQUEST';
  message: string;
  retrySafe: boolean;
  recovery: ShellRecoveryAction;
  preservesDraft: boolean;
}

const FAILURE_PREFIX = 'GOSLING_SHELL_FAILURE:';
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

function failure(
  code: ShellOperationFailure['code'],
  message: string,
  retrySafe: boolean,
  recovery: ShellRecoveryAction,
  preservesDraft = false
): ShellOperationFailure {
  return { code, message, retrySafe, recovery, preservesDraft };
}

export function classifyShellOperationFailure(
  operation: string,
  error: unknown
): ShellOperationFailure {
  const detail = error instanceof Error ? error.message : '';

  if (detail.includes('did not declare') || detail.includes('adapter is unavailable')) {
    return failure(
      'CAPABILITY_UNAVAILABLE',
      'This capability is not available in the current shell.',
      false,
      'open_gosling'
    );
  }
  if (detail.includes('is stale')) {
    return failure(
      'STALE_REQUEST',
      'The shell changed while this action was pending.',
      false,
      'refresh',
      operation === 'prompt.submit'
    );
  }
  if (detail.includes('no working directory is selected')) {
    return failure(
      'DIRECTORY_REQUIRED',
      'Choose a working folder before starting a task.',
      false,
      'choose_directory',
      operation === 'prompt.submit'
    );
  }
  if (detail.includes('credential') && /missing|required|unavailable|relink/i.test(detail)) {
    return failure(
      'CREDENTIAL_REQUIRED',
      'Select or reconnect an account before starting a task.',
      false,
      'select_credential',
      operation === 'prompt.submit'
    );
  }
  if (detail.includes('settings are in a recovery state')) {
    return failure(
      'SETTINGS_RECOVERY_REQUIRED',
      'Local shell settings need review before they can be changed.',
      false,
      'reset_settings'
    );
  }
  if (
    detail.includes('interaction is pending') ||
    detail.includes('interaction is in progress') ||
    detail.includes('confirmation is pending')
  ) {
    return failure(
      'INTERACTION_PENDING',
      'Respond to the pending request before continuing.',
      false,
      'review_session',
      operation === 'prompt.submit'
    );
  }
  if (
    detail.includes('active session already exists') ||
    detail.includes('prompt attempt is already active') ||
    detail.includes('cannot detach while')
  ) {
    return failure(
      'SESSION_BUSY',
      'Finish or stop the current task before continuing.',
      false,
      'review_session',
      operation === 'prompt.submit'
    );
  }
  if (
    detail.includes('session runtime is unavailable') ||
    detail.includes('session is not active') ||
    detail.includes('session working directory does not match') ||
    /session.*not found/i.test(detail)
  ) {
    return failure(
      'SESSION_UNAVAILABLE',
      'The requested session is not available.',
      false,
      'review_session',
      operation === 'prompt.submit'
    );
  }
  if (
    detail.includes('shell runtime is unavailable') ||
    detail.includes('timed out') ||
    detail.includes('transport')
  ) {
    return failure(
      'RUNTIME_UNAVAILABLE',
      'The shell backend is not currently available.',
      true,
      'retry',
      operation === 'prompt.submit'
    );
  }
  if (detail.includes('prompt text must')) {
    return failure(
      'INVALID_INPUT',
      'Enter a non-empty request within the shell size limit.',
      false,
      'none',
      true
    );
  }
  if (
    detail.includes('unsupported fields') ||
    detail.includes('must be') ||
    detail.includes('size limit') ||
    detail.includes('does not accept') ||
    detail.includes('explicit user gesture') ||
    detail.includes('not allowlisted') ||
    detail.includes('credentials are not allowed')
  ) {
    return failure(
      'INVALID_REQUEST',
      'The shell rejected an invalid or unsupported request.',
      false,
      'none',
      operation === 'prompt.submit'
    );
  }

  return failure(
    'OPERATION_FAILED',
    'The shell could not complete this action.',
    false,
    'save_diagnostics',
    operation === 'prompt.submit'
  );
}

export function encodeShellOperationFailure(failure: ShellOperationFailure): string {
  return `${FAILURE_PREFIX}${JSON.stringify(failure)}`;
}

export function decodeShellOperationFailure(error: unknown): ShellOperationFailure | null {
  const message = error instanceof Error ? error.message : typeof error === 'string' ? error : '';
  const start = message.indexOf(FAILURE_PREFIX);
  if (start < 0) return null;
  try {
    const parsed = JSON.parse(message.slice(start + FAILURE_PREFIX.length)) as unknown;
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    const value = parsed as Record<string, unknown>;
    const keys = Object.keys(value).sort();
    if (
      keys.join(',') !==
      ['code', 'message', 'preservesDraft', 'recovery', 'retrySafe'].sort().join(',')
    ) {
      return null;
    }
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
      return null;
    }
    return parsed as ShellOperationFailure;
  } catch {
    return null;
  }
}
