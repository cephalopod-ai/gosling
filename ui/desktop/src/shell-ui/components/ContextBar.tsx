import type { ShellCredentialSnapshot } from '../../shell/credentialController';
import type { ShellDirectorySnapshot } from '../../shell/directoryController';
import type { ShellLifecycleStateName } from '../../shell/lifecycle';
import type { ShellRuntimeSnapshot } from '../../shell/runtimeSnapshot';
import type { ShellSessionRecord } from '../../shell/sessionController';
import { COPY } from '../copy';
import { ShellButton, ShellChip, ShellPill, type ShellTone } from './primitives';

const LIFECYCLE_TONES: Record<ShellLifecycleStateName, ShellTone> = {
  booting: 'neutral',
  validating: 'busy',
  ready: 'ok',
  busy: 'busy',
  degraded: 'error',
  relink_required: 'error',
  incompatible: 'error',
  offline: 'error',
  stopping: 'neutral',
  stopped: 'neutral',
  fatal: 'error',
};

const LIFECYCLE_LABELS: Record<ShellLifecycleStateName, string> = {
  booting: 'starting',
  validating: 'checking',
  ready: 'ready',
  busy: 'working',
  degraded: 'needs attention',
  relink_required: 'account not connected',
  incompatible: 'version mismatch',
  offline: 'offline',
  stopping: 'shutting down',
  stopped: 'stopped',
  fatal: 'stopped unexpectedly',
};

export const StatusPill = ({
  lifecycleState,
  compatibility,
}: {
  lifecycleState: ShellLifecycleStateName;
  compatibility: ShellRuntimeSnapshot['compatibility'];
}) => (
  <ShellPill
    tone={LIFECYCLE_TONES[lifecycleState]}
    label={
      compatibility.status === 'incompatible'
        ? LIFECYCLE_LABELS.incompatible
        : LIFECYCLE_LABELS[lifecycleState]
    }
  />
);

/**
 * `identity` stays null until ACP preflight verifies it, so the packaged display name is shown from
 * the profile while the verification state is reported separately rather than implied.
 */
export const IdentityBadge = ({
  identity,
  fallbackName,
}: {
  identity: ShellRuntimeSnapshot['identity'];
  fallbackName: string;
}) => (
  <span className="gsh-identity">
    <span className="gsh-identity__name">{identity?.displayName ?? fallbackName}</span>
    {identity ? <span className="gsh-identity__version">{identity.version}</span> : null}
    <span className="gsh-identity__verified">{identity ? COPY.verified : COPY.unverified}</span>
  </span>
);

function directoryChip(directory: ShellDirectorySnapshot) {
  if (directory.state === 'unselected') {
    return <ShellChip label={COPY.noFolder} muted />;
  }
  const label = directory.label ?? '';
  if (directory.state === 'missing') {
    return <ShellChip label={`${label} — folder is gone`} tone="error" />;
  }
  if (directory.state === 'invalid') {
    return <ShellChip label={`${label} — not usable`} tone="error" />;
  }
  return <ShellChip label={label} {...(directory.path ? { title: directory.path } : {})} />;
}

function credentialChip(credentials: ShellCredentialSnapshot) {
  const selected = credentials.profiles.find(
    (profile) => profile.id === credentials.selectedProfileId
  );
  const name = selected?.name ?? credentials.selectedProfileId ?? '';
  if (credentials.catalogStatus === 'unavailable') {
    return <ShellChip label={COPY.accountsUnavailable} tone="error" />;
  }
  if (credentials.selectionStatus === 'none') {
    return <ShellChip label={COPY.noAccount} muted />;
  }
  if (credentials.selectionStatus === 'relink_required') {
    return <ShellChip label={`${name} — needs reconnecting`} tone="error" />;
  }
  if (credentials.selectionStatus === 'missing') {
    return <ShellChip label={`${name} — no longer exists`} tone="error" />;
  }
  if (credentials.catalogStatus === 'denied') {
    return <ShellChip label={`${name} (${COPY.accountFixedByProduct})`} />;
  }
  return <ShellChip label={name} />;
}

export interface ContextBarProps {
  directory: ShellDirectorySnapshot;
  credentials: ShellCredentialSnapshot;
  session: ShellSessionRecord | null;
  canChangeDirectory: boolean;
  canSelectDirectory: boolean;
  canSelectCredential: boolean;
  onChooseDirectory: () => void;
  onChooseAccount: () => void;
}

export const ContextBar = ({
  directory,
  credentials,
  session,
  canChangeDirectory,
  canSelectDirectory,
  canSelectCredential,
  onChooseDirectory,
  onChooseAccount,
}: ContextBarProps) => {
  const showCredentials = canSelectCredential || credentials.selectionStatus !== 'none';

  return (
    <div className="gsh-ctxbar">
      <span className="gsh-ctxbar__group">
        <span className="gsh-ctxbar__label">Folder</span>
        {directoryChip(directory)}
        {canSelectDirectory ? (
          <ShellButton
            label={directory.state === 'selected' ? COPY.change : COPY.chooseFolder}
            onClick={onChooseDirectory}
            emphasis="ghost"
            disabled={!canChangeDirectory}
            describedBy={canChangeDirectory ? undefined : 'gsh-directory-locked'}
          />
        ) : null}
        {canChangeDirectory ? null : (
          <span id="gsh-directory-locked" className="gsh-hint">
            Stop or leave the current task before changing folder.
          </span>
        )}
      </span>
      {showCredentials ? (
        <span className="gsh-ctxbar__group">
          <span className="gsh-ctxbar__label">Account</span>
          {credentialChip(credentials)}
          {canSelectCredential && credentials.catalogStatus === 'available' ? (
            <ShellButton label={COPY.change} onClick={onChooseAccount} emphasis="ghost" />
          ) : null}
        </span>
      ) : null}
      {session && session.providerId ? (
        <span className="gsh-ctxbar__group">
          <ShellChip
            label={`${session.providerId}${session.modelId ? ` · ${session.modelId}` : ''}`}
            muted
          />
        </span>
      ) : null}
    </div>
  );
};
