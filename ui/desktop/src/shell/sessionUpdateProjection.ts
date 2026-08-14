import type { SessionUpdate } from '@agentclientprotocol/sdk';

const MAX_TEXT_BYTES = 60 * 1024;
const MAX_TITLE_BYTES = 4 * 1024;

export type ShellSessionStream =
  | {
      type: 'content';
      role: 'user' | 'assistant' | 'thought';
      messageId: string | null;
      text: string;
    }
  | {
      type: 'tool';
      toolCallId: string;
      title: string | null;
      toolKind: string | null;
      status: string | null;
    }
  | { type: 'session_info'; title: string }
  | {
      type: 'usage';
      used: number;
      size: number;
    };

function boundedString(value: unknown, maximum: number): string | null {
  if (typeof value !== 'string' || Buffer.byteLength(value, 'utf8') > maximum) return null;
  return value;
}

function boundedNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

export function projectShellSessionUpdate(update: SessionUpdate): ShellSessionStream | null {
  if (
    update.sessionUpdate === 'user_message_chunk' ||
    update.sessionUpdate === 'agent_message_chunk' ||
    update.sessionUpdate === 'agent_thought_chunk'
  ) {
    const role =
      update.sessionUpdate === 'user_message_chunk'
        ? 'user'
        : update.sessionUpdate === 'agent_message_chunk'
          ? 'assistant'
          : 'thought';
    if (update.content.type !== 'text') return null;
    const text = boundedString(update.content.text, MAX_TEXT_BYTES);
    if (text === null) return null;
    return {
      type: 'content',
      role,
      messageId: boundedString(update.messageId, 512),
      text,
    };
  }

  if (update.sessionUpdate === 'tool_call' || update.sessionUpdate === 'tool_call_update') {
    const toolCallId = boundedString(update.toolCallId, 512);
    if (!toolCallId) return null;
    return {
      type: 'tool',
      toolCallId,
      title: boundedString(update.title, MAX_TITLE_BYTES),
      toolKind: boundedString(update.kind, 64),
      status: boundedString(update.status, 64),
    };
  }

  if (update.sessionUpdate === 'session_info_update') {
    const title = boundedString(update.title, MAX_TITLE_BYTES);
    return title ? { type: 'session_info', title } : null;
  }

  if (update.sessionUpdate === 'usage_update') {
    const used = boundedNumber(update.used);
    const size = boundedNumber(update.size);
    if (used === null || size === null) return null;
    return { type: 'usage', used, size };
  }

  return null;
}
