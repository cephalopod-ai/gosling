import { useMemo, useState } from 'react';
import { defineMessages, useIntl } from '../../i18n';

export type ContextHistoryDiffMode = 'words' | 'lines';

export interface ContextHistoryDiffPart {
  type: 'equal' | 'added' | 'removed';
  value: string;
}

const i18n = defineMessages({
  title: {
    id: 'contextHistory.diff.title',
    defaultMessage: 'Changes from snapshot #{before} to #{after}',
  },
  detail: { id: 'contextHistory.diff.detail', defaultMessage: 'Diff detail' },
  words: { id: 'contextHistory.diff.words', defaultMessage: 'Words' },
  lines: { id: 'contextHistory.diff.lines', defaultMessage: 'Lines' },
  removed: { id: 'contextHistory.diff.removed', defaultMessage: 'Removed' },
  added: { id: 'contextHistory.diff.added', defaultMessage: 'Added' },
  unchanged: {
    id: 'contextHistory.diff.unchanged',
    defaultMessage: 'Unchanged content hidden',
  },
  noChanges: {
    id: 'contextHistory.diff.noChanges',
    defaultMessage: 'No summary changes.',
  },
});

const MAX_EXACT_DIFF_CELLS = 2_000_000;

function tokenize(value: string, mode: ContextHistoryDiffMode): string[] {
  if (mode === 'lines') {
    return value.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  }
  return value.match(/\s+|[\p{L}\p{N}_]+|[^\s\p{L}\p{N}_]+/gu) ?? [];
}

function coalesce(parts: ContextHistoryDiffPart[]): ContextHistoryDiffPart[] {
  const result: ContextHistoryDiffPart[] = [];
  for (const part of parts) {
    if (!part.value) continue;
    const previous = result[result.length - 1];
    if (previous?.type === part.type) {
      previous.value += part.value;
    } else {
      result.push({ ...part });
    }
  }
  return result;
}

function diffMiddle(before: string[], after: string[]): ContextHistoryDiffPart[] {
  if (before.length === 0) {
    return [{ type: 'added', value: after.join('') }];
  }
  if (after.length === 0) {
    return [{ type: 'removed', value: before.join('') }];
  }

  const width = after.length + 1;
  const cells = (before.length + 1) * width;
  if (cells > MAX_EXACT_DIFF_CELLS) {
    // A whole-block replacement stays truthful while preventing unrelated
    // summaries from forcing quadratic memory use in the renderer.
    return [
      { type: 'removed', value: before.join('') },
      { type: 'added', value: after.join('') },
    ];
  }

  const lengths = new Uint32Array(cells);
  for (let beforeIndex = 1; beforeIndex <= before.length; beforeIndex += 1) {
    for (let afterIndex = 1; afterIndex <= after.length; afterIndex += 1) {
      const position = beforeIndex * width + afterIndex;
      if (before[beforeIndex - 1] === after[afterIndex - 1]) {
        lengths[position] = lengths[(beforeIndex - 1) * width + afterIndex - 1] + 1;
      } else {
        lengths[position] = Math.max(
          lengths[(beforeIndex - 1) * width + afterIndex],
          lengths[beforeIndex * width + afterIndex - 1]
        );
      }
    }
  }

  const reversed: ContextHistoryDiffPart[] = [];
  let beforeIndex = before.length;
  let afterIndex = after.length;
  while (beforeIndex > 0 || afterIndex > 0) {
    if (beforeIndex > 0 && afterIndex > 0 && before[beforeIndex - 1] === after[afterIndex - 1]) {
      reversed.push({ type: 'equal', value: before[beforeIndex - 1] });
      beforeIndex -= 1;
      afterIndex -= 1;
    } else if (
      afterIndex > 0 &&
      (beforeIndex === 0 ||
        lengths[beforeIndex * width + afterIndex - 1] >=
          lengths[(beforeIndex - 1) * width + afterIndex])
    ) {
      reversed.push({ type: 'added', value: after[afterIndex - 1] });
      afterIndex -= 1;
    } else {
      reversed.push({ type: 'removed', value: before[beforeIndex - 1] });
      beforeIndex -= 1;
    }
  }

  return coalesce(reversed.reverse());
}

export function buildContextHistoryDiff(
  beforeValue: string,
  afterValue: string,
  mode: ContextHistoryDiffMode
): ContextHistoryDiffPart[] {
  const before = tokenize(beforeValue, mode);
  const after = tokenize(afterValue, mode);
  let prefixLength = 0;
  while (
    prefixLength < before.length &&
    prefixLength < after.length &&
    before[prefixLength] === after[prefixLength]
  ) {
    prefixLength += 1;
  }

  let suffixLength = 0;
  while (
    suffixLength < before.length - prefixLength &&
    suffixLength < after.length - prefixLength &&
    before[before.length - suffixLength - 1] === after[after.length - suffixLength - 1]
  ) {
    suffixLength += 1;
  }

  const parts: ContextHistoryDiffPart[] = [];
  if (prefixLength > 0) {
    parts.push({ type: 'equal', value: before.slice(0, prefixLength).join('') });
  }
  parts.push(
    ...diffMiddle(
      before.slice(prefixLength, before.length - suffixLength),
      after.slice(prefixLength, after.length - suffixLength)
    )
  );
  if (suffixLength > 0) {
    parts.push({
      type: 'equal',
      value: before.slice(before.length - suffixLength).join(''),
    });
  }
  return coalesce(parts);
}

export function ContextHistoryDiff({
  before,
  after,
  beforeGeneration,
  afterGeneration,
}: {
  before: string;
  after: string;
  beforeGeneration: number;
  afterGeneration: number;
}) {
  const intl = useIntl();
  const [mode, setMode] = useState<ContextHistoryDiffMode>('words');
  const parts = useMemo(() => buildContextHistoryDiff(before, after, mode), [after, before, mode]);
  const hasChanges = parts.some((part) => part.type !== 'equal');

  return (
    <section
      aria-label={intl.formatMessage(i18n.title, {
        before: beforeGeneration,
        after: afterGeneration,
      })}
      className="space-y-2"
    >
      <div className="flex flex-wrap items-end justify-between gap-3">
        <p className="text-xs font-medium">
          {intl.formatMessage(i18n.title, {
            before: beforeGeneration,
            after: afterGeneration,
          })}
        </p>
        <fieldset className="flex items-center gap-3 text-xs">
          <legend className="sr-only">{intl.formatMessage(i18n.detail)}</legend>
          {(['words', 'lines'] as const).map((option) => (
            <label key={option} className="flex items-center gap-1.5">
              <input
                type="radio"
                name={`context-history-diff-${beforeGeneration}-${afterGeneration}`}
                value={option}
                checked={mode === option}
                onChange={() => setMode(option)}
              />
              {intl.formatMessage(option === 'words' ? i18n.words : i18n.lines)}
            </label>
          ))}
        </fieldset>
      </div>
      <p className="flex gap-3 text-xs text-text-secondary">
        <span>− {intl.formatMessage(i18n.removed)}</span>
        <span>+ {intl.formatMessage(i18n.added)}</span>
      </p>
      {!hasChanges ? (
        <p className="rounded bg-background-secondary p-3 text-xs">
          {intl.formatMessage(i18n.noChanges)}
        </p>
      ) : (
        <div className="max-h-80 space-y-1 overflow-auto rounded bg-background-secondary p-2 text-xs">
          {parts.map((part, index) => {
            if (part.type === 'equal') {
              return (
                <p
                  key={`${part.type}-${index}`}
                  className="border-y border-border-primary/50 py-1 text-center text-text-secondary"
                >
                  … {intl.formatMessage(i18n.unchanged)} …
                </p>
              );
            }

            const label = intl.formatMessage(part.type === 'removed' ? i18n.removed : i18n.added);
            const className = part.type === 'removed' ? 'bg-red-500/10' : 'bg-green-500/10';
            const marker = part.type === 'removed' ? '−' : '+';
            const Content = part.type === 'removed' ? 'del' : 'ins';
            return (
              <p key={`${part.type}-${index}`} className={`${className} flex rounded px-2 py-1`}>
                <span aria-hidden="true" className="mr-2 shrink-0 font-mono font-semibold">
                  {marker}
                </span>
                <Content
                  aria-label={`${label}: ${part.value}`}
                  className="whitespace-pre-wrap break-words no-underline"
                >
                  {part.value}
                </Content>
              </p>
            );
          })}
        </div>
      )}
    </section>
  );
}
