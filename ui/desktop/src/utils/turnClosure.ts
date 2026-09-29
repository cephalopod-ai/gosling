import type { Message } from '../types/message';

// A turn that stopped before it finished is closed by the backend with ordinary text, so these
// must stay identical to its wording: the notices in
// crates/gosling/src/session/session_manager/turn_closure.rs and the result
// tool_operations.rs `cancelled_before_dispatch` gives a tool call that never ran.
const TURN_CLOSURE_NOTICES = new Set([
  'Run interrupted before completion.',
  'Run cancelled by user before completion.',
]);
const TOOL_NOT_RUN_ERROR =
  'Tool execution was cancelled before it started because the prior turn ended. It will not be retried automatically.';
// A tool error read back from session history is replayed as "<JSON-RPC code>: <message>".
const ERROR_CODE_PREFIX = /^-?\d+:\s*/;

export function getTurnClosureNotice(message: Message): string | undefined {
  if (message.role !== 'assistant' || message.content.length !== 1) {
    return undefined;
  }
  const [content] = message.content;
  if (content.type !== 'text') {
    return undefined;
  }
  const text = content.text.trim();
  return TURN_CLOSURE_NOTICES.has(text) ? text : undefined;
}

export function isToolNotRunError(error: string | undefined): boolean {
  return error?.trim().replace(ERROR_CODE_PREFIX, '') === TOOL_NOT_RUN_ERROR;
}
