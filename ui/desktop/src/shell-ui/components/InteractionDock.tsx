import { useEffect, useMemo, useRef, useState } from 'react';
import type { ShellElicitationField, ShellInteraction } from '../../shell/interactionController';
import { COPY } from '../copy';
import { ShellBadge, ShellButton, ShellButtonRow, ShellPill } from './primitives';

type PermissionInteraction = Extract<ShellInteraction, { kind: 'permission' }>;
type ElicitationInteraction = Extract<ShellInteraction, { kind: 'elicitation' }>;
type ConfirmInteraction = Extract<ShellInteraction, { kind: 'confirm' }>;

const PermissionRequest = ({
  interaction,
  onRespond,
}: {
  interaction: PermissionInteraction;
  onRespond: (allowOnce: boolean) => void;
}) => {
  const { summary } = interaction;
  return (
    <>
      <h2 className="gsh-dock__heading">
        <ShellBadge label={summary.effect} variant={summary.effect} />
        {COPY.permissionHeading}
      </h2>
      <dl className="gsh-dock__detail">
        {summary.toolTitle ? (
          <>
            <dt>Tool</dt>
            <dd>{summary.toolTitle}</dd>
          </>
        ) : null}
        {summary.targets.length > 0 ? (
          <>
            <dt>{summary.targets.length === 1 ? 'Target' : 'Targets'}</dt>
            <dd>{summary.targets.join(', ')}</dd>
          </>
        ) : null}
        {summary.inputFields.length > 0 ? (
          <>
            <dt>Input fields</dt>
            <dd>{summary.inputFields.join(', ')}</dd>
          </>
        ) : null}
      </dl>
      <ShellButtonRow>
        {summary.allowOnce ? (
          <ShellButton label={COPY.allowOnce} onClick={() => onRespond(true)} emphasis="primary" />
        ) : null}
        {summary.deny ? <ShellButton label={COPY.deny} onClick={() => onRespond(false)} /> : null}
      </ShellButtonRow>
    </>
  );
};

type FieldValue = string | number | boolean | string[];

function defaultFieldValue(field: ShellElicitationField): FieldValue {
  if (field.defaultValue !== undefined) return field.defaultValue;
  if (field.type === 'boolean') return false;
  if (field.type === 'multi_select') return [];
  return '';
}

function isFieldSatisfied(field: ShellElicitationField, value: FieldValue): boolean {
  if (!field.required) return true;
  if (field.type === 'boolean') return true;
  if (field.type === 'multi_select') return Array.isArray(value) && value.length > 0;
  return String(value).length > 0;
}

function coerce(field: ShellElicitationField, value: FieldValue): unknown {
  if (field.type === 'number' || field.type === 'integer') {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return value;
}

const inputTypeFor = (field: ShellElicitationField): string => {
  if (field.type === 'number' || field.type === 'integer') return 'number';
  if (field.format === 'email') return 'email';
  if (field.format === 'uri') return 'url';
  if (field.format === 'date') return 'date';
  if (field.format === 'date-time') return 'datetime-local';
  return 'text';
};

const ElicitationForm = ({
  interaction,
  onRespond,
}: {
  interaction: ElicitationInteraction;
  onRespond: (action: 'submit' | 'decline' | 'cancel', fields?: Record<string, unknown>) => void;
}) => {
  const { summary } = interaction;
  const [values, setValues] = useState<Record<string, FieldValue>>(() =>
    Object.fromEntries(summary.fields.map((field) => [field.name, defaultFieldValue(field)]))
  );

  const complete = useMemo(
    () => summary.fields.every((field) => isFieldSatisfied(field, values[field.name] ?? '')),
    [summary.fields, values]
  );

  const setValue = (name: string, value: FieldValue) =>
    setValues((previous) => ({ ...previous, [name]: value }));

  const submit = () => {
    const payload: Record<string, unknown> = {};
    for (const field of summary.fields) {
      const coerced = coerce(field, values[field.name] ?? defaultFieldValue(field));
      if (coerced !== undefined) payload[field.name] = coerced;
    }
    onRespond('submit', payload);
  };

  return (
    <>
      <h2 className="gsh-dock__heading">{summary.title ?? COPY.formHeading}</h2>
      <p className="gsh-dock__message">{summary.message}</p>
      {summary.description ? <p className="gsh-dock__message">{summary.description}</p> : null}
      <div className="gsh-form">
        {summary.fields.map((field) => {
          const id = `gsh-field-${interaction.actionId}-${field.name}`;
          const value = values[field.name] ?? defaultFieldValue(field);
          return (
            <div className="gsh-form__field" key={field.name}>
              <label className="gsh-form__label" htmlFor={id}>
                {field.label}
                {field.required ? <span className="gsh-form__required"> (required)</span> : null}
              </label>
              {field.description ? (
                <span className="gsh-form__description">{field.description}</span>
              ) : null}
              {field.type === 'boolean' ? (
                <input
                  id={id}
                  type="checkbox"
                  checked={value === true}
                  onChange={(event) => setValue(field.name, event.target.checked)}
                />
              ) : field.type === 'multi_select' ? (
                <select
                  id={id}
                  multiple
                  value={Array.isArray(value) ? value : []}
                  onChange={(event) =>
                    setValue(
                      field.name,
                      [...event.target.selectedOptions].map((option) => option.value)
                    )
                  }
                >
                  {(field.options ?? []).map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              ) : field.options && field.options.length > 0 ? (
                <select
                  id={id}
                  value={String(value)}
                  onChange={(event) => setValue(field.name, event.target.value)}
                >
                  <option value="">—</option>
                  {field.options.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  id={id}
                  type={inputTypeFor(field)}
                  value={String(value)}
                  required={field.required}
                  {...(field.minimum === undefined ? {} : { min: field.minimum })}
                  {...(field.maximum === undefined ? {} : { max: field.maximum })}
                  {...(field.minLength === undefined ? {} : { minLength: field.minLength })}
                  {...(field.maxLength === undefined ? {} : { maxLength: field.maxLength })}
                  {...(field.type === 'integer' ? { step: 1 } : {})}
                  onChange={(event) => setValue(field.name, event.target.value)}
                />
              )}
            </div>
          );
        })}
      </div>
      <ShellButtonRow>
        <ShellButton label={COPY.submit} onClick={submit} emphasis="primary" disabled={!complete} />
        <ShellButton label={COPY.decline} onClick={() => onRespond('decline')} />
        <ShellButton label={COPY.cancel} onClick={() => onRespond('cancel')} />
      </ShellButtonRow>
    </>
  );
};

const ConfirmationRequest = ({
  interaction,
  productName,
  onRespond,
}: {
  interaction: ConfirmInteraction;
  productName: string;
  onRespond: (approve: boolean) => void;
}) => (
  <>
    <h2 className="gsh-dock__heading">{COPY.confirmHeading(interaction.summary.action)}</h2>
    <dl className="gsh-dock__detail">
      <dt>Action</dt>
      <dd>{interaction.summary.action}</dd>
      {interaction.summary.inputFields.length > 0 ? (
        <>
          <dt>Input fields</dt>
          <dd>{interaction.summary.inputFields.join(', ')}</dd>
        </>
      ) : null}
    </dl>
    <p className="gsh-dock__message">{COPY.confirmDetail(productName)}</p>
    <ShellButtonRow>
      <ShellButton label={COPY.approve} onClick={() => onRespond(true)} emphasis="primary" />
      <ShellButton label={COPY.reject} onClick={() => onRespond(false)} />
    </ShellButtonRow>
  </>
);

export interface InteractionDockProps {
  interactions: ShellInteraction[];
  productName: string;
  onPermission: (actionId: string, allowOnce: boolean) => void;
  onElicitation: (
    actionId: string,
    action: 'submit' | 'decline' | 'cancel',
    fields?: Record<string, unknown>
  ) => void;
  onConfirmation: (actionId: string, approve: boolean) => void;
}

/**
 * At most one interaction is presented; the rest are counted. A-2/A-3: the dock claims focus when a
 * request arrives and hands it back to the element that had it once the queue empties.
 */
export const InteractionDock = ({
  interactions,
  productName,
  onPermission,
  onElicitation,
  onConfirmation,
}: InteractionDockProps) => {
  const dockRef = useRef<HTMLElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const current = interactions[0];
  const actionId = current?.actionId ?? null;
  // A ref, not state: two clicks inside one React batch would both observe a stale state value and
  // send the response twice, and every response is single-use.
  const respondedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!actionId) {
      const previous = previousFocusRef.current;
      previousFocusRef.current = null;
      if (previous && previous.isConnected) previous.focus();
      return;
    }
    const active = document.activeElement;
    if (
      !previousFocusRef.current &&
      active instanceof HTMLElement &&
      !dockRef.current?.contains(active)
    ) {
      previousFocusRef.current = active;
    }
    dockRef.current?.focus();
    respondedRef.current = null;
  }, [actionId]);

  if (!current) return null;

  const once =
    <T,>(handler: (value: T) => void) =>
    (value: T) => {
      if (respondedRef.current === current.actionId) return;
      respondedRef.current = current.actionId;
      handler(value);
    };

  return (
    <section
      className="gsh-dock"
      ref={dockRef}
      tabIndex={-1}
      aria-label="Pending request"
      role="region"
    >
      <div className="gsh-dock__queue" role="status" aria-live="polite">
        {interactions.length > 1 ? (
          <ShellPill tone="neutral" label={COPY.moreWaiting(interactions.length - 1)} />
        ) : null}
      </div>
      {current.kind === 'permission' ? (
        <PermissionRequest
          interaction={current}
          onRespond={once((allowOnce: boolean) => onPermission(current.actionId, allowOnce))}
        />
      ) : current.kind === 'elicitation' ? (
        <ElicitationForm
          interaction={current}
          onRespond={(action, fields) => {
            if (respondedRef.current === current.actionId) return;
            respondedRef.current = current.actionId;
            onElicitation(current.actionId, action, fields);
          }}
        />
      ) : (
        <ConfirmationRequest
          interaction={current}
          productName={productName}
          onRespond={once((approve: boolean) => onConfirmation(current.actionId, approve))}
        />
      )}
    </section>
  );
};
