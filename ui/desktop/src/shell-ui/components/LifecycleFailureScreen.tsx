import type { ShellLifecycleAction } from '../../shell/lifecycle';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import { COPY, lifecycleCopy } from '../copy';
import { ShellButton, ShellButtonRow, ShellCentered } from './primitives';

export const ProvisioningIssueList = ({
  issues,
}: {
  issues: ShellRuntimeSnapshot['provisioningIssues'];
}) => {
  if (issues.length === 0) return null;
  return (
    <div className="gsh-issues">
      <h3 className="gsh-issues__heading">{COPY.provisioningHeading}</h3>
      <ul className="gsh-issues__list">
        {issues.map((issue, index) => (
          <li className="gsh-issues__row" key={`${issue.code}-${issue.path ?? index}`}>
            <span className="gsh-issues__code">{issue.code}</span>
            {issue.path ? <span className="gsh-issues__path">{issue.path}</span> : null}
          </li>
        ))}
      </ul>
    </div>
  );
};

export interface LifecycleFailureScreenProps {
  snapshot: ShellRuntimeSnapshot;
  productName: string;
  savedDiagnosticsFile: string | null;
  canHandOff: boolean;
  onRetry: () => void;
  onStop: () => void;
  onSaveDiagnostics: () => void;
  onHandoff: () => void;
  onRestart: () => void;
}

/**
 * Buttons are derived from `allowedActions`. Hard-coding Retry would produce a dead control in
 * `relink_required`, `incompatible`, and `fatal`, where the host rejects it. `stopped` allows no
 * host action at all, so Restart is offered instead: it is the only legal transition back to
 * `booting`, and it starts a new generation.
 */
export const LifecycleFailureScreen = ({
  snapshot,
  productName,
  savedDiagnosticsFile,
  canHandOff,
  onRetry,
  onStop,
  onSaveDiagnostics,
  onHandoff,
  onRestart,
}: LifecycleFailureScreenProps) => {
  const copy = lifecycleCopy(snapshot.lifecycleState, productName);
  const allows = (action: ShellLifecycleAction) => snapshot.allowedActions.includes(action);
  const stopped = snapshot.lifecycleState === 'stopped';

  return (
    <div className="gsh-lifecycle" role="alert" aria-live="assertive">
      <ShellCentered heading={copy.heading} detail={copy.detail}>
        {snapshot.reasonCode ? (
          <p className="gsh-lifecycle__reason">Reason: {snapshot.reasonCode}</p>
        ) : null}
        <ProvisioningIssueList issues={snapshot.provisioningIssues} />
        {allows('handoff') && !canHandOff ? (
          <p className="gsh-hint">{COPY.handoffUnavailable(productName)}</p>
        ) : null}
        {savedDiagnosticsFile ? (
          <p className="gsh-hint" role="status">
            {COPY.diagnosticsSaved(savedDiagnosticsFile)}
          </p>
        ) : null}
        <ShellButtonRow>
          {stopped ? (
            <ShellButton label={COPY.restart} onClick={onRestart} emphasis="primary" />
          ) : null}
          {allows('retry') ? (
            <ShellButton label={COPY.retry} onClick={onRetry} emphasis="primary" />
          ) : null}
          {canHandOff ? <ShellButton label={COPY.openInGosling} onClick={onHandoff} /> : null}
          {allows('diagnostics') ? (
            <ShellButton label={COPY.saveDiagnostics} onClick={onSaveDiagnostics} />
          ) : null}
          {allows('stop') ? <ShellButton label={COPY.quit} onClick={onStop} /> : null}
        </ShellButtonRow>
      </ShellCentered>
    </div>
  );
};
