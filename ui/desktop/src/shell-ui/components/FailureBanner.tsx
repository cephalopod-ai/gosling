import type { ShellOperationFailure, ShellRecoveryAction } from '../../shell/operationFailure';
import { COPY } from '../copy';
import { ShellButton } from './primitives';

export interface RecoveryHandlers {
  retry: () => void;
  refresh: () => void;
  chooseDirectory: () => void;
  selectCredential: () => void;
  reviewSession: () => void;
  resetSettings: () => void;
  openGosling: () => void;
  saveDiagnostics: () => void;
  restart: () => void;
}

const RECOVERY_LABELS: Record<Exclude<ShellRecoveryAction, 'none'>, string> = {
  retry: COPY.retry,
  refresh: COPY.refresh,
  choose_directory: COPY.chooseFolder,
  select_credential: COPY.chooseAccount,
  review_session: COPY.reviewRequest,
  reset_settings: COPY.resetSettings,
  open_gosling: COPY.openInGosling,
  save_diagnostics: COPY.saveDiagnostics,
  restart: COPY.restart,
};

function handlerFor(
  recovery: Exclude<ShellRecoveryAction, 'none'>,
  handlers: RecoveryHandlers
): () => void {
  switch (recovery) {
    case 'retry':
      return handlers.retry;
    case 'refresh':
      return handlers.refresh;
    case 'choose_directory':
      return handlers.chooseDirectory;
    case 'select_credential':
      return handlers.selectCredential;
    case 'review_session':
      return handlers.reviewSession;
    case 'reset_settings':
      return handlers.resetSettings;
    case 'open_gosling':
      return handlers.openGosling;
    case 'save_diagnostics':
      return handlers.saveDiagnostics;
    case 'restart':
      return handlers.restart;
    default:
      return () => undefined;
  }
}

/**
 * `STALE_REQUEST` is a normal consequence of retry and shutdown, so it is rendered as a quiet
 * inline notice rather than an error banner.
 */
export const FailureBanner = ({
  failure,
  handlers,
}: {
  failure: ShellOperationFailure;
  handlers: RecoveryHandlers;
}) => {
  const quiet = failure.code === 'STALE_REQUEST';
  return (
    <div
      className={quiet ? 'gsh-failure gsh-failure--quiet' : 'gsh-failure'}
      role={quiet ? 'status' : 'alert'}
      aria-live={quiet ? 'polite' : 'assertive'}
    >
      <span className="gsh-failure__message">{failure.message}</span>
      <span className="gsh-failure__code">{failure.code}</span>
      {failure.recovery === 'none' ? null : (
        <ShellButton
          label={RECOVERY_LABELS[failure.recovery]}
          onClick={handlerFor(failure.recovery, handlers)}
          emphasis={quiet ? 'ghost' : 'default'}
        />
      )}
    </div>
  );
};
