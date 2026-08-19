import { forwardRef } from 'react';
import type { ShellSessionRecord } from '../../shell/sessionController';
import { COPY } from '../copy';
import { ShellButton } from './primitives';

/** Matches `MAX_PROMPT_BYTES` in `src/shell/sessionController.ts`. */
export const MAX_PROMPT_BYTES = 64 * 1024;
const COUNTER_THRESHOLD = Math.floor(MAX_PROMPT_BYTES * 0.8);

export function promptByteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

export interface ComposerProps {
  draft: string;
  session: ShellSessionRecord | null;
  blockedByInteraction: boolean;
  canSubmit: boolean;
  canCancel: boolean;
  onDraftChange: (draft: string) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export const Composer = forwardRef<HTMLTextAreaElement, ComposerProps>(function Composer(
  { draft, session, blockedByInteraction, canSubmit, canCancel, onDraftChange, onSubmit, onCancel },
  ref
) {
  const phase = session?.promptAttempt?.phase ?? 'idle';
  const bytes = promptByteLength(draft);
  const overLimit = bytes > MAX_PROMPT_BYTES;
  const streaming = phase === 'streaming';
  const cancelling = phase === 'cancelling';
  const sendDisabled =
    !canSubmit ||
    blockedByInteraction ||
    draft.length === 0 ||
    overLimit ||
    streaming ||
    cancelling;

  return (
    <div className="gsh-composer">
      <label className="gsh-composer__label" htmlFor="gsh-composer-input">
        Your request
      </label>
      <textarea
        id="gsh-composer-input"
        ref={ref}
        className="gsh-composer__input"
        value={draft}
        rows={3}
        placeholder={
          blockedByInteraction ? COPY.composerBlockedByInteraction : COPY.composerPlaceholder
        }
        disabled={blockedByInteraction}
        onChange={(event) => onDraftChange(event.target.value)}
        aria-describedby={overLimit ? 'gsh-composer-limit' : undefined}
      />
      <div className="gsh-composer__row">
        {streaming || cancelling ? (
          <ShellButton
            label={cancelling ? COPY.stopping : COPY.stopTask}
            onClick={onCancel}
            emphasis="primary"
            disabled={cancelling || !canCancel}
          />
        ) : (
          <ShellButton
            label={COPY.send}
            onClick={onSubmit}
            emphasis="primary"
            disabled={sendDisabled}
          />
        )}
        {bytes >= COUNTER_THRESHOLD ? (
          <span
            id="gsh-composer-limit"
            className={
              overLimit ? 'gsh-composer__count gsh-composer__count--over' : 'gsh-composer__count'
            }
          >
            {bytes.toLocaleString()} / {MAX_PROMPT_BYTES.toLocaleString()} bytes
          </span>
        ) : null}
      </div>
    </div>
  );
});
