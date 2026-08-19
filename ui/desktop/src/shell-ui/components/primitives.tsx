import type { ReactNode } from 'react';

export type ShellTone = 'ok' | 'busy' | 'warn' | 'error' | 'neutral';

export interface ShellButtonProps {
  label: string;
  onClick: () => void;
  emphasis?: 'primary' | 'default' | 'ghost';
  disabled?: boolean;
  describedBy?: string;
}

export const ShellButton = ({
  label,
  onClick,
  emphasis = 'default',
  disabled = false,
  describedBy,
}: ShellButtonProps) => (
  <button
    type="button"
    className={`gsh-btn gsh-btn--${emphasis}`}
    onClick={onClick}
    disabled={disabled}
    {...(describedBy ? { 'aria-describedby': describedBy } : {})}
  >
    {label}
  </button>
);

export const ShellButtonRow = ({ children }: { children: ReactNode }) => (
  <div className="gsh-btnrow">{children}</div>
);

/** A-9: the tone is decoration; the label carries the meaning for screen readers and monochrome. */
export const ShellPill = ({ tone, label }: { tone: ShellTone; label: string }) => (
  <span className={`gsh-pill gsh-pill--${tone}`}>{label}</span>
);

export const ShellChip = ({
  label,
  title,
  tone = 'neutral',
  muted = false,
}: {
  label: string;
  title?: string;
  tone?: ShellTone;
  muted?: boolean;
}) => (
  <span
    className={`gsh-chip gsh-chip--${tone}${muted ? ' gsh-chip--muted' : ''}`}
    {...(title ? { title } : {})}
  >
    {label}
  </span>
);

export const ShellBadge = ({ label, variant }: { label: string; variant: string }) => (
  <span className={`gsh-badge gsh-badge--${variant}`}>{label}</span>
);

export const ShellNotice = ({
  tone,
  message,
  children,
  live = false,
}: {
  tone: ShellTone;
  message: string;
  children?: ReactNode;
  live?: boolean;
}) => (
  <div
    className={`gsh-notice gsh-notice--${tone}`}
    {...(live ? { role: 'status', 'aria-live': 'polite' } : {})}
  >
    <span className="gsh-notice__text">{message}</span>
    {children ? <span className="gsh-notice__actions">{children}</span> : null}
  </div>
);

export const ShellCentered = ({
  heading,
  detail,
  children,
}: {
  heading: string;
  detail: string;
  children?: ReactNode;
}) => (
  <div className="gsh-center">
    <h2 className="gsh-center__heading">{heading}</h2>
    <p className="gsh-center__detail">{detail}</p>
    {children}
  </div>
);
