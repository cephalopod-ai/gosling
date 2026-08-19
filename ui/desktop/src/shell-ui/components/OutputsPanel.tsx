import type { ShellArtifactSummary } from '@repo-makeover/gosling-sdk';
import { COPY } from '../copy';

export interface OutputsPanelProps {
  outputs: {
    status: 'idle' | 'loading' | 'loaded';
    items: ShellArtifactSummary[];
    totalCount: number;
    truncated: boolean;
  };
}

export const OutputsPanel = ({ outputs }: OutputsPanelProps) => (
  <section className="gsh-outputs" aria-labelledby="gsh-outputs-heading">
    <div className="gsh-outputs__head">
      <h2 id="gsh-outputs-heading" className="gsh-outputs__heading">
        {COPY.outputsHeading}
      </h2>
      {outputs.status === 'loaded' ? (
        <span className="gsh-outputs__count">{outputs.totalCount}</span>
      ) : null}
    </div>
    {outputs.status !== 'loaded' ? (
      <p className="gsh-outputs__empty" role="status">
        Loading outputs…
      </p>
    ) : outputs.items.length === 0 ? (
      <p className="gsh-outputs__empty">{COPY.outputsEmpty}</p>
    ) : (
      <ul className="gsh-outputs__list">
        {outputs.items.map((output, index) => (
          <li className="gsh-output" key={`${output.name}:${output.relation}:${index}`}>
            <span className="gsh-output__name">{output.name}</span>
            <span className="gsh-output__meta">
              {output.kind} · {output.relation}
            </span>
          </li>
        ))}
      </ul>
    )}
    {outputs.truncated ? <p className="gsh-outputs__hint">{COPY.outputsTruncated}</p> : null}
  </section>
);
