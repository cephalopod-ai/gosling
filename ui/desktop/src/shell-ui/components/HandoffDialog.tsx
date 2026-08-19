import type { ShellHandoffEnvelope } from '@repo-makeover/gosling-sdk';
import { COPY } from '../copy';
import { ShellButton, ShellButtonRow } from './primitives';

export interface HandoffDialogProps {
  handoff: ShellHandoffEnvelope;
  productName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * The dialog states exactly what leaves the shell. References are shown as a count of identifiers
 * because the envelope carries ids, not file contents.
 */
export const HandoffDialog = ({
  handoff,
  productName,
  onConfirm,
  onCancel,
}: HandoffDialogProps) => (
  <section className="gsh-handoff" aria-label={COPY.handoffHeading}>
    <h2 className="gsh-handoff__heading">{COPY.handoffHeading}</h2>
    <p className="gsh-handoff__detail">{COPY.handoffDetail(productName)}</p>
    <dl className="gsh-handoff__list">
      <dt>Question</dt>
      <dd>{handoff.question}</dd>
      <dt>From</dt>
      <dd>
        {handoff.origin.displayName} {handoff.origin.version}
      </dd>
      <dt>Session</dt>
      <dd>{handoff.sourceSessionId}</dd>
      <dt>Capability</dt>
      <dd>{handoff.requestedCapability}</dd>
      <dt>References</dt>
      <dd>
        {handoff.references && handoff.references.length > 0
          ? `${handoff.references.length} items (identifiers only)`
          : 'none'}
      </dd>
    </dl>
    <ShellButtonRow>
      <ShellButton label={COPY.openGosling} onClick={onConfirm} emphasis="primary" />
      <ShellButton label={COPY.cancel} onClick={onCancel} />
    </ShellButtonRow>
  </section>
);
