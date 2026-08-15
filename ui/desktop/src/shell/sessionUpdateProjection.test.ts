import { describe, expect, it } from 'vitest';
import { projectShellSessionUpdate } from './sessionUpdateProjection';

describe('shell session update projection', () => {
  it('projects text and tool progress without protocol metadata or raw tool values', () => {
    expect(
      projectShellSessionUpdate({
        sessionUpdate: 'agent_message_chunk',
        messageId: 'message-a',
        content: { type: 'text', text: 'hello' },
        _meta: { private: 'discarded' },
      })
    ).toEqual({ type: 'content', role: 'assistant', messageId: 'message-a', text: 'hello' });
    expect(
      projectShellSessionUpdate({
        sessionUpdate: 'tool_call_update',
        toolCallId: 'tool-a',
        title: 'Read source',
        kind: 'read',
        status: 'completed',
        rawInput: { private: 'discarded' },
        rawOutput: { private: 'discarded' },
      })
    ).toEqual({
      type: 'tool',
      toolCallId: 'tool-a',
      title: 'Read source',
      toolKind: 'read',
      status: 'completed',
    });
  });

  it('rejects unsupported and oversized updates', () => {
    expect(
      projectShellSessionUpdate({
        sessionUpdate: 'agent_thought_chunk',
        content: { type: 'text', text: 'private reasoning' },
      })
    ).toBeNull();
    expect(
      projectShellSessionUpdate({
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: 'x'.repeat(60 * 1024 + 1) },
      })
    ).toBeNull();
    expect(
      projectShellSessionUpdate({
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'image', data: 'image-data', mimeType: 'image/png' },
      })
    ).toBeNull();
  });
});
