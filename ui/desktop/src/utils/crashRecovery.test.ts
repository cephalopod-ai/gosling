import { describe, expect, it } from 'vitest';
import type { Message, MessageContent } from '../types/message';
import { canSafelyAutoResume } from './crashRecovery';

function message(role: Message['role'], ...content: MessageContent[]): Message {
  return {
    role,
    content,
    created: 1,
    metadata: { userVisible: true, agentVisible: true },
  };
}

describe('canSafelyAutoResume', () => {
  it('allows an explicitly interrupted turn with no tool activity', () => {
    expect(
      canSafelyAutoResume('uncertain', [
        message('user', { type: 'text', text: 'Finish the report' }),
        message('assistant', { type: 'text', text: 'Working on it' }),
      ])
    ).toBe(true);
  });

  it('allows tool calls with durable successful responses', () => {
    expect(
      canSafelyAutoResume('uncertain', [
        message('user', { type: 'text', text: 'Read the file' }),
        message('assistant', {
          type: 'toolRequest',
          id: 'tool-1',
          toolCall: { status: 'success' },
        }),
        message('user', {
          type: 'toolResponse',
          id: 'tool-1',
          toolResult: { status: 'success' },
        }),
      ])
    ).toBe(true);
  });

  it('blocks failed, unresolved, and approval-pending tool activity', () => {
    const prompt = message('user', { type: 'text', text: 'Deploy it' });
    const request = message('assistant', {
      type: 'toolRequest',
      id: 'tool-1',
      toolCall: { status: 'success' },
    });
    const failed = message('user', {
      type: 'toolResponse',
      id: 'tool-1',
      toolResult: { status: 'error', error: 'execution status is in doubt' },
    });
    const approval = message('assistant', {
      type: 'toolConfirmationRequest',
      id: 'tool-1',
      toolName: 'deploy',
      arguments: {},
    });

    expect(canSafelyAutoResume('uncertain', [prompt, request])).toBe(false);
    expect(canSafelyAutoResume('uncertain', [prompt, request, failed])).toBe(false);
    expect(canSafelyAutoResume('uncertain', [prompt, approval])).toBe(false);
  });

  it('blocks unknown backend integrity and incomplete compacted history', () => {
    const prompt = message('user', { type: 'text', text: 'Continue' });
    expect(canSafelyAutoResume('unknown', [prompt])).toBe(false);
    expect(canSafelyAutoResume('clean', [prompt])).toBe(false);
    expect(
      canSafelyAutoResume('uncertain', [
        message('assistant', { type: 'text', text: 'Tail without the originating prompt' }),
      ])
    ).toBe(false);
  });
});
