import { describe, expect, it } from 'vitest';
import {
  appendTranscriptUpdate,
  emptyTranscript,
  latestTitle,
  latestUsage,
  transcriptBlocks,
  transcriptFromSnapshot,
} from './transcript';
import { update } from '../testSupport';
import type { ShellSessionUpdate } from '../../shell/sessionController';

function ingest(updates: ShellSessionUpdate[]) {
  return updates.reduce(appendTranscriptUpdate, emptyTranscript());
}

function textUpdate(seq: number, text: string, delivery: 'history' | 'live' = 'live') {
  return update({
    updateSeq: seq,
    delivery,
    stream: { type: 'content', role: 'assistant', messageId: `m${seq}`, text },
  });
}

describe('transcript ordering', () => {
  const ordered = [1, 2, 3, 4, 5].map((seq) => textUpdate(seq, `t${seq}`));

  it('converges on the same order for every arrival permutation', () => {
    const permutations = [
      [0, 1, 2, 3, 4],
      [4, 3, 2, 1, 0],
      [2, 0, 4, 1, 3],
      [1, 0, 3, 2, 4],
      [3, 4, 0, 2, 1],
    ];
    const expected = ordered.map((entry) => entry.updateSeq);
    for (const permutation of permutations) {
      const state = ingest(permutation.map((index) => ordered[index]));
      expect(state.updates.map((entry) => entry.updateSeq)).toEqual(expected);
    }
  });

  it('discards duplicate sequence numbers', () => {
    const state = ingest([...ordered, ...ordered]);
    expect(state.updates).toHaveLength(ordered.length);
    expect(state.hasGap).toBe(false);
  });

  it('flags a missing sequence number as a gap', () => {
    const state = ingest([textUpdate(1, 'a'), textUpdate(2, 'b'), textUpdate(5, 'c')]);
    expect(state.hasGap).toBe(true);
    expect(state.firstSeq).toBe(1);
    expect(state.lastSeq).toBe(5);
  });

  it('resets when a different session starts reporting', () => {
    const first = ingest([textUpdate(1, 'a'), textUpdate(2, 'b')]);
    const second = appendTranscriptUpdate(
      first,
      update({
        sessionId: 'sess-2',
        updateSeq: 9,
        stream: { type: 'content', role: 'user', messageId: null, text: 'new' },
      })
    );
    expect(second.sessionId).toBe('sess-2');
    expect(second.updates).toHaveLength(1);
  });

  it('deduplicates and sorts a transcript snapshot', () => {
    const state = transcriptFromSnapshot({
      generation: 1,
      sessionId: 'sess-1',
      integrity: 'incomplete',
      firstSeq: 3,
      lastSeq: 1,
      truncated: true,
      updates: [textUpdate(3, 'c'), textUpdate(1, 'a'), textUpdate(1, 'a'), textUpdate(2, 'b')],
    });
    expect(state.updates.map((entry) => entry.updateSeq)).toEqual([1, 2, 3]);
    expect(state.truncated).toBe(true);
    expect(state.integrity).toBe('incomplete');
  });
});

describe('transcript blocks', () => {
  it('places exactly one seam between history and live delivery', () => {
    const state = ingest([
      textUpdate(1, 'old', 'history'),
      textUpdate(2, 'older', 'history'),
      textUpdate(3, 'new', 'live'),
      textUpdate(4, 'newer', 'live'),
    ]);
    const seams = transcriptBlocks(state).filter((block) => block.kind === 'seam');
    expect(seams).toHaveLength(1);
  });

  it('omits the seam when nothing was replayed', () => {
    const state = ingest([textUpdate(1, 'a'), textUpdate(2, 'b')]);
    expect(transcriptBlocks(state).some((block) => block.kind === 'seam')).toBe(false);
  });

  it('coalesces consecutive same-role content into one block', () => {
    const state = ingest([textUpdate(1, 'Hello '), textUpdate(2, 'world')]);
    const blocks = transcriptBlocks(state);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ kind: 'message', text: 'Hello world' });
  });

  it('collapses repeated tool updates onto the latest status', () => {
    const state = ingest([
      update({
        updateSeq: 1,
        stream: {
          type: 'tool',
          toolCallId: 'tool-1',
          title: 'write',
          toolKind: 'edit',
          status: 'pending',
        },
      }),
      update({
        updateSeq: 2,
        stream: {
          type: 'tool',
          toolCallId: 'tool-1',
          title: 'write',
          toolKind: 'edit',
          status: 'completed',
        },
      }),
    ]);
    const tools = transcriptBlocks(state).filter((block) => block.kind === 'tool');
    expect(tools).toHaveLength(1);
    expect(tools[0]).toMatchObject({ status: 'completed' });
  });

  it('renders terminal outcomes and carries a failure message', () => {
    const state = ingest([
      textUpdate(1, 'a'),
      update({
        updateSeq: 2,
        kind: 'failed',
        failure: {
          code: 'RUNTIME_UNAVAILABLE',
          message: 'The shell backend is not currently available.',
          retrySafe: true,
          recovery: 'retry',
          preservesDraft: true,
        },
      }),
    ]);
    const outcomes = transcriptBlocks(state).filter((block) => block.kind === 'outcome');
    expect(outcomes).toHaveLength(1);
    expect(outcomes[0]).toMatchObject({
      outcome: 'failed',
      message: 'The shell backend is not currently available.',
    });
  });

  it('reads the latest usage and title projections', () => {
    const state = ingest([
      update({ updateSeq: 1, stream: { type: 'usage', used: 10, size: 100 } }),
      update({ updateSeq: 2, stream: { type: 'session_info', title: 'Renamed' } }),
      update({ updateSeq: 3, stream: { type: 'usage', used: 40, size: 100 } }),
    ]);
    expect(latestUsage(state)).toEqual({ used: 40, size: 100 });
    expect(latestTitle(state)).toBe('Renamed');
  });
});
