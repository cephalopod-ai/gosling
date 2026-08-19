import type { ShellCredentialSnapshot } from '../../shell/credentialController';
import type { ShellDirectorySnapshot } from '../../shell/directoryController';
import { COPY, credentialCopy, directoryCopy } from '../copy';
import { ShellButton, ShellButtonRow, ShellCentered, ShellNotice } from './primitives';

export const DirectoryPrompt = ({
  directory,
  productName,
  canSelect,
  cancelled,
  onChoose,
  onSaveDiagnostics,
}: {
  directory: ShellDirectorySnapshot;
  productName: string;
  canSelect: boolean;
  cancelled: boolean;
  onChoose: () => void;
  onSaveDiagnostics: () => void;
}) => {
  const copy = directoryCopy(directory, productName);
  return (
    <ShellCentered heading={copy.heading} detail={copy.detail}>
      {directory.reasonCode ? <p className="gsh-hint">Reason: {directory.reasonCode}</p> : null}
      {cancelled ? <p className="gsh-hint">No folder was chosen.</p> : null}
      <ShellButtonRow>
        {canSelect ? (
          <ShellButton label={COPY.chooseFolder} onClick={onChoose} emphasis="primary" />
        ) : null}
        {directory.state === 'missing' || directory.state === 'invalid' ? (
          <ShellButton label={COPY.saveDiagnostics} onClick={onSaveDiagnostics} />
        ) : null}
      </ShellButtonRow>
    </ShellCentered>
  );
};

/**
 * Only the four safe catalog fields are rendered. No secret, no configured field list, and no
 * connection test: those belong to Gosling.
 */
export const CredentialPicker = ({
  credentials,
  productName,
  canSelect,
  onSelect,
  onRetry,
  onSaveDiagnostics,
}: {
  credentials: ShellCredentialSnapshot;
  productName: string;
  canSelect: boolean;
  onSelect: (profileId: string) => void;
  onRetry: () => void;
  onSaveDiagnostics: () => void;
}) => {
  const copy = credentialCopy(credentials, productName);

  if (credentials.catalogStatus === 'unavailable') {
    return (
      <ShellCentered heading={copy.heading} detail={copy.detail}>
        <ShellButtonRow>
          <ShellButton label={COPY.retry} onClick={onRetry} emphasis="primary" />
          <ShellButton label={COPY.saveDiagnostics} onClick={onSaveDiagnostics} />
        </ShellButtonRow>
      </ShellCentered>
    );
  }

  return (
    <div className="gsh-credentials">
      <h2 className="gsh-credentials__heading">{copy.heading}</h2>
      <p className="gsh-credentials__detail">{copy.detail}</p>
      <ul className="gsh-credentials__list">
        {credentials.profiles.map((profile) => (
          <li className="gsh-credentials__row" key={profile.id}>
            <span className="gsh-credentials__name">{profile.name}</span>
            <span className="gsh-credentials__provider">{profile.providerOrServiceId}</span>
            <span
              className={
                profile.status === 'configured'
                  ? 'gsh-credentials__status'
                  : 'gsh-credentials__status gsh-credentials__status--warn'
              }
            >
              {profile.status === 'configured' ? 'configured' : 'needs reconnecting'}
            </span>
            {canSelect ? (
              <ShellButton label={COPY.useAccount} onClick={() => onSelect(profile.id)} />
            ) : null}
          </li>
        ))}
      </ul>
      {credentials.profiles.length === 0 ? (
        <ShellNotice tone="warn" message="No accounts are available to this shell." />
      ) : null}
    </div>
  );
};

export const CredentialProblem = ({
  credentials,
  productName,
  canSelect,
  canHandOff,
  onOpenGosling,
  onChooseAnother,
}: {
  credentials: ShellCredentialSnapshot;
  productName: string;
  canSelect: boolean;
  canHandOff: boolean;
  onOpenGosling: () => void;
  onChooseAnother: () => void;
}) => {
  const copy = credentialCopy(credentials, productName);
  return (
    <ShellCentered heading={copy.heading} detail={copy.detail}>
      {canHandOff ? null : <p className="gsh-hint">{COPY.handoffUnavailable(productName)}</p>}
      <ShellButtonRow>
        {canHandOff ? (
          <ShellButton label={COPY.openInGosling} onClick={onOpenGosling} emphasis="primary" />
        ) : null}
        {canSelect && credentials.catalogStatus === 'available' ? (
          <ShellButton label="Choose a different account" onClick={onChooseAnother} />
        ) : null}
      </ShellButtonRow>
    </ShellCentered>
  );
};
