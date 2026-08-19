import type { ShellSessionUpdate, ShellTranscriptSnapshot } from '../../shell/sessionController';

export type TranscriptIntegrity = ShellTranscriptSnapshot['integrity'];

export interface TranscriptState {
  sessionId: string | null;
  updates: ShellSessionUpdate[];
  integrity: TranscriptIntegrity;
  truncated: boolean;
  firstSeq: number | null;
  lastSeq: number | null;
  hasGap: boolean;
}

export type TranscriptBlock =
  | {
      kind: 'message';
      key: string;
      role: 'user' | 'assistant';
      delivery: ShellSessionUpdate['delivery'];
      text: string;
    }
  | {
      kind: 'tool';
      key: string;
      delivery: ShellSessionUpdate['delivery'];
      toolCallId: string;
      title: string | null;
      toolKind: string | null;
      status: string | null;
    }
  | { kind: 'seam'; key: string }
  | {
      kind: 'outcome';
      key: string;
      outcome: 'completed' | 'cancelled' | 'failed';
      message: string | null;
    };

export function emptyTranscript(): TranscriptState {
  return {
    sessionId: null,
    updates: [],
    integrity: 'complete',
    truncated: false,
    firstSeq: null,
    lastSeq: null,
    hasGap: false,
  };
}

function contiguityGap(updates: ShellSessionUpdate[]): boolean {
  for (let index = 1; index < updates.length; index += 1) {
    if (updates[index].updateSeq - updates[index - 1].updateSeq > 1) return true;
  }
  return false;
}

function bounds(updates: ShellSessionUpdate[]): {
  firstSeq: number | null;
  lastSeq: number | null;
} {
  return {
    firstSeq: updates[0]?.updateSeq ?? null,
    lastSeq: updates[updates.length - 1]?.updateSeq ?? null,
  };
}

/** R-2: order by updateSeq, discard duplicates, never sort by arrival. */
function insertBySeq(
  updates: ShellSessionUpdate[],
  update: ShellSessionUpdate
): ShellSessionUpdate[] {
  let low = 0;
  let high = updates.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (updates[middle].updateSeq < update.updateSeq) low = middle + 1;
    else high = middle;
  }
  if (updates[low]?.updateSeq === update.updateSeq) return updates;
  return [...updates.slice(0, low), update, ...updates.slice(low)];
}

export function transcriptFromSnapshot(snapshot: ShellTranscriptSnapshot): TranscriptState {
  const updates = [...snapshot.updates].sort((left, right) => left.updateSeq - right.updateSeq);
  const deduped: ShellSessionUpdate[] = [];
  for (const update of updates) {
    if (deduped[deduped.length - 1]?.updateSeq !== update.updateSeq) deduped.push(update);
  }
  return {
    sessionId: snapshot.sessionId,
    updates: deduped,
    integrity: snapshot.integrity,
    truncated: snapshot.truncated,
    ...bounds(deduped),
    hasGap: contiguityGap(deduped),
  };
}

export function appendTranscriptUpdate(
  state: TranscriptState,
  update: ShellSessionUpdate
): TranscriptState {
  if (state.sessionId !== null && state.sessionId !== update.sessionId) {
    return {
      ...emptyTranscript(),
      sessionId: update.sessionId,
      updates: [update],
      ...bounds([update]),
    };
  }
  const updates = insertBySeq(state.updates, update);
  if (updates === state.updates) return state;
  return {
    ...state,
    sessionId: update.sessionId,
    updates,
    ...bounds(updates),
    hasGap: contiguityGap(updates),
  };
}

function outcomeMessage(update: ShellSessionUpdate): string | null {
  return update.failure ? update.failure.message : null;
}

/**
 * R-3: history-delivery updates precede live ones after a resume, and exactly one seam marks the
 * boundary. A history update arriving after live content has begun is a renderer ordering bug, so
 * the seam is placed on the first live update rather than recomputed per block.
 */
export function transcriptBlocks(state: TranscriptState): TranscriptBlock[] {
  const blocks: TranscriptBlock[] = [];
  let sawHistory = false;
  let seamPlaced = false;
  const latestToolIndex = new Map<string, number>();

  for (const update of state.updates) {
    if (update.delivery === 'history') sawHistory = true;
    if (update.delivery === 'live' && sawHistory && !seamPlaced) {
      blocks.push({ kind: 'seam', key: `seam-${update.updateSeq}` });
      seamPlaced = true;
    }

    if (update.kind === 'completed' || update.kind === 'cancelled' || update.kind === 'failed') {
      blocks.push({
        kind: 'outcome',
        key: `outcome-${update.updateSeq}`,
        outcome: update.kind,
        message: outcomeMessage(update),
      });
      continue;
    }

    const stream = update.stream;
    if (!stream) continue;

    if (stream.type === 'content') {
      const previous = blocks[blocks.length - 1];
      if (
        previous?.kind === 'message' &&
        previous.role === stream.role &&
        previous.delivery === update.delivery
      ) {
        blocks[blocks.length - 1] = { ...previous, text: previous.text + stream.text };
        continue;
      }
      blocks.push({
        kind: 'message',
        key: `message-${update.updateSeq}`,
        role: stream.role,
        delivery: update.delivery,
        text: stream.text,
      });
      continue;
    }

    if (stream.type === 'tool') {
      const existing = latestToolIndex.get(stream.toolCallId);
      const block: TranscriptBlock = {
        kind: 'tool',
        key: `tool-${stream.toolCallId}`,
        delivery: update.delivery,
        toolCallId: stream.toolCallId,
        title: stream.title,
        toolKind: stream.toolKind,
        status: stream.status,
      };
      if (existing === undefined) {
        latestToolIndex.set(stream.toolCallId, blocks.length);
        blocks.push(block);
      } else {
        blocks[existing] = block;
      }
    }
  }

  return blocks;
}

export function latestUsage(state: TranscriptState): { used: number; size: number } | null {
  for (let index = state.updates.length - 1; index >= 0; index -= 1) {
    const stream = state.updates[index].stream;
    if (stream?.type === 'usage') return { used: stream.used, size: stream.size };
  }
  return null;
}

export function latestTitle(state: TranscriptState): string | null {
  for (let index = state.updates.length - 1; index >= 0; index -= 1) {
    const stream = state.updates[index].stream;
    if (stream?.type === 'session_info') return stream.title;
  }
  return null;
}
