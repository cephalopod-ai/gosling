import type { ActionRequired } from '../types/message';
import { defineMessages, useIntl } from '../i18n';
import { snakeToTitleCase } from '../utils';
import ToolApprovalButtons from './ToolApprovalButtons';

const i18n = defineMessages({
  allowToolCallWithName: {
    id: 'toolConfirmation.allowToolCallWithName',
    defaultMessage: 'Allow {toolName}?',
  },
  goslingWouldLikeToCallWithName: {
    id: 'toolConfirmation.goslingWouldLikeToCallWithName',
    defaultMessage: 'Gosling would like to call {toolName}. Allow?',
  },
});

function formatToolName(fullName: string): string {
  const delimiterIndex = fullName.lastIndexOf('__');
  const shortName = delimiterIndex === -1 ? fullName : fullName.substring(delimiterIndex + 2);
  return snakeToTitleCase(shortName);
}

// Mirrors the key list Gosling's backend uses to build a fallback tool-call
// title (see `summarize_tool_call` in crates/gosling/src/acp/server.rs), so
// the approval prompt shows the same "what is this actually doing" detail
// instead of a bare tool name.
const DETAIL_ARG_KEYS = ['path', 'file', 'command', 'query', 'url', 'uri', 'name', 'pattern', 'source'];
const MAX_DETAIL_LENGTH = 140;

function summarizeArguments(args: Record<string, unknown> | undefined): string | undefined {
  if (!args) return undefined;
  for (const key of DETAIL_ARG_KEYS) {
    const value = args[key];
    if (value === undefined || value === null) continue;
    const text = typeof value === 'string' ? value : JSON.stringify(value);
    const firstLine = text.split('\n')[0];
    if (!firstLine) continue;
    return firstLine.length > MAX_DETAIL_LENGTH
      ? `${firstLine.slice(0, MAX_DETAIL_LENGTH)}…`
      : firstLine;
  }
  return undefined;
}

type ToolConfirmationData = Extract<ActionRequired['data'], { actionType: 'toolConfirmation' }>;

interface ToolConfirmationProps {
  sessionId: string;
  isClicked: boolean;
  actionRequiredContent: ActionRequired & { type: 'actionRequired' };
}

export default function ToolConfirmation({
  sessionId,
  isClicked,
  actionRequiredContent,
}: ToolConfirmationProps) {
  const intl = useIntl();
  const data = actionRequiredContent.data as ToolConfirmationData;
  const { id, toolName, prompt, arguments: toolArguments } = data;
  const displayName = formatToolName(toolName);
  const detail = summarizeArguments(toolArguments);

  return (
    <div className="gosling-message-content bg-background-primary border border-border-primary rounded-2xl overflow-hidden">
      <div className="bg-background-secondary px-4 py-2 text-text-primary">
        <div>
          {prompt
            ? intl.formatMessage(i18n.allowToolCallWithName, { toolName: displayName })
            : intl.formatMessage(i18n.goslingWouldLikeToCallWithName, { toolName: displayName })}
        </div>
        {detail && (
          <div className="text-sm text-text-secondary mt-0.5 font-mono truncate" title={detail}>
            {detail}
          </div>
        )}
      </div>
      <ToolApprovalButtons
        data={{ id, toolName, prompt: prompt ?? undefined, sessionId, isClicked }}
      />
    </div>
  );
}
