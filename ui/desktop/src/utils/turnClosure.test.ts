import { describe, expect, it } from 'vitest';
import type { Message } from '../types/message';
import { getTurnClosureNotice, isToolNotRunError } from './turnClosure';

function assistantText(...texts: string[]): Message {
  return {
    id: 'notice',
    role: 'assistant',
    created: 1,
    content: texts.map((text) => ({ type: 'text', text })),
    metadata: { agentVisible: true, userVisible: true },
  };
}

describe('getTurnClosureNotice', () => {
  it('recognizes the notices the backend appends when it closes a stopped turn', () => {
    expect(getTurnClosureNotice(assistantText('Run interrupted before completion.'))).toBe(
      'Run interrupted before completion.'
    );
    expect(getTurnClosureNotice(assistantText('Run cancelled by user before completion.'))).toBe(
      'Run cancelled by user before completion.'
    );
  });

  it('leaves ordinary replies alone', () => {
    expect(getTurnClosureNotice(assistantText('The run finished.'))).toBeUndefined();
    expect(
      getTurnClosureNotice(assistantText('Run interrupted before completion.', 'More text'))
    ).toBeUndefined();
    expect(
      getTurnClosureNotice({ ...assistantText('Run interrupted before completion.'), role: 'user' })
    ).toBeUndefined();
  });
});

describe('isToolNotRunError', () => {
  const notRun =
    'Tool execution was cancelled before it started because the prior turn ended. It will not be retried automatically.';

  it('matches the result given to a tool call that never ran, live or replayed', () => {
    expect(isToolNotRunError(notRun)).toBe(true);
    // session/load replays stored tool errors with their JSON-RPC code in front.
    expect(isToolNotRunError(`-32600: ${notRun}`)).toBe(true);
  });

  it('leaves every other error alone', () => {
    expect(isToolNotRunError('ENOENT: missing file')).toBe(false);
    expect(isToolNotRunError(`-32603: ${notRun} Retrying.`)).toBe(false);
    expect(isToolNotRunError(undefined)).toBe(false);
  });
});
