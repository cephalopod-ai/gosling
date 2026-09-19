// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { buildContextHistoryDiff } from './ContextHistoryDiff';

describe('buildContextHistoryDiff', () => {
  it('keeps only the changed word between shared context', () => {
    expect(buildContextHistoryDiff('The brown fox', 'The red fox', 'words')).toEqual([
      { type: 'equal', value: 'The ' },
      { type: 'removed', value: 'brown' },
      { type: 'added', value: 'red' },
      { type: 'equal', value: ' fox' },
    ]);
  });

  it('compares complete lines while preserving their line endings', () => {
    expect(
      buildContextHistoryDiff('first\nold detail\nlast\n', 'first\nnew detail\nlast\n', 'lines')
    ).toEqual([
      { type: 'equal', value: 'first\n' },
      { type: 'removed', value: 'old detail\n' },
      { type: 'added', value: 'new detail\n' },
      { type: 'equal', value: 'last\n' },
    ]);
  });

  it('falls back to a bounded whole-block replacement for unrelated large summaries', () => {
    const before = `old${' a'.repeat(1_500)}`;
    const after = `new${' b'.repeat(1_500)}`;

    expect(buildContextHistoryDiff(before, after, 'words')).toEqual([
      { type: 'removed', value: before },
      { type: 'added', value: after },
    ]);
  });
});
