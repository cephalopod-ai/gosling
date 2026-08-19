import { COPY } from '../copy';
import { latestUsage, transcriptBlocks, type TranscriptState } from '../state/transcript';
import { ShellButton, ShellNotice } from './primitives';

const OUTCOME_LABELS = {
  completed: 'Task finished',
  cancelled: 'Task stopped',
  failed: 'Task failed',
} as const;

export const UsageMeter = ({ used, size }: { used: number; size: number }) => {
  const percent = size > 0 ? Math.min(100, Math.round((used / size) * 100)) : 0;
  return (
    <div className="gsh-usage">
      <span className="gsh-usage__label">
        Context {percent}% ({used} of {size})
      </span>
      <span className="gsh-usage__track" aria-hidden="true">
        {/* Written through the CSSOM by React rather than a style attribute, which the shell's
            `style-src 'self'` CSP would reject. */}
        <span className="gsh-usage__fill" style={{ width: `${percent}%` }} />
      </span>
    </div>
  );
};

export const TranscriptGapNotice = ({
  transcript,
  onRepair,
  canRepair,
}: {
  transcript: TranscriptState;
  onRepair: () => void;
  canRepair: boolean;
}) => {
  if (transcript.integrity === 'resume_uncertain') {
    return (
      <ShellNotice tone="warn" message={COPY.resumeUncertain} live>
        {canRepair ? <ShellButton label={COPY.repair} onClick={onRepair} /> : null}
      </ShellNotice>
    );
  }
  if (transcript.integrity === 'incomplete' || transcript.truncated || transcript.hasGap) {
    return (
      <ShellNotice tone="warn" message={COPY.transcriptGap} live>
        {canRepair ? <ShellButton label={COPY.repair} onClick={onRepair} /> : null}
      </ShellNotice>
    );
  }
  return null;
};

export interface TranscriptViewProps {
  transcript: TranscriptState;
  onRepair: () => void;
  canRepair: boolean;
}

export const TranscriptView = ({ transcript, onRepair, canRepair }: TranscriptViewProps) => {
  const blocks = transcriptBlocks(transcript);
  const usage = latestUsage(transcript);
  return (
    <div className="gsh-transcript">
      <TranscriptGapNotice transcript={transcript} onRepair={onRepair} canRepair={canRepair} />
      <ol className="gsh-transcript__list">
        {blocks.map((block) => {
          if (block.kind === 'seam') {
            return (
              <li key={block.key} className="gsh-seam">
                <span>{COPY.historySeam}</span>
              </li>
            );
          }
          if (block.kind === 'message') {
            return (
              <li
                key={block.key}
                className={`gsh-msg gsh-msg--${block.role} gsh-msg--${block.delivery}`}
              >
                <span className="gsh-msg__who">
                  {block.role === 'user' ? 'You' : 'Assistant'}
                  <span className="gsh-msg__delivery"> · {block.delivery}</span>
                </span>
                <span className="gsh-msg__text">{block.text}</span>
              </li>
            );
          }
          if (block.kind === 'tool') {
            return (
              <li key={block.key} className="gsh-tool">
                <span className="gsh-tool__title">{block.title ?? block.toolCallId}</span>
                {block.toolKind ? <span className="gsh-tool__kind">{block.toolKind}</span> : null}
                {block.status ? <span className="gsh-tool__status">{block.status}</span> : null}
              </li>
            );
          }
          return (
            <li
              key={block.key}
              className={`gsh-outcome gsh-outcome--${block.outcome}`}
              role="status"
              aria-live="polite"
            >
              <span>{OUTCOME_LABELS[block.outcome]}</span>
              {block.message ? <span className="gsh-outcome__detail">{block.message}</span> : null}
            </li>
          );
        })}
      </ol>
      {usage ? <UsageMeter used={usage.used} size={usage.size} /> : null}
    </div>
  );
};
