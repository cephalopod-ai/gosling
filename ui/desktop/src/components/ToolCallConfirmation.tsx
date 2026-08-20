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
// Upper bound on what the approval prompt will render. Generous on purpose:
// this is the text the operator is being asked to approve, so it should be cut
// only to stop a pathological payload from taking over the window.
const MAX_DETAIL_LENGTH = 4000;

// The approval prompt used to show the first matching argument's *first line*,
// clipped to 140 characters and then CSS-truncated again — so a shell command
// whose second line was the destructive part was approved unseen
// (WEB-GOS-002). The full value is now returned and rendered in a bounded,
// scrollable block; nothing that will run is hidden from the decision.
export function summarizeArguments(args: Record<string, unknown> | undefined): string | undefined {
  if (!args) return undefined;
  for (const key of DETAIL_ARG_KEYS) {
    const value = args[key];
    if (value === undefined || value === null) continue;
    const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
    if (!text.trim()) continue;
    return text.length > MAX_DETAIL_LENGTH
      ? `${text.slice(0, MAX_DETAIL_LENGTH)}\n… (truncated for display)`
      : text;
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
  const { id, toolName, prompt, domain, arguments: toolArguments } = data;
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
          <pre className="text-sm text-text-secondary mt-0.5 font-mono whitespace-pre-wrap break-words max-h-48 overflow-auto">
            {detail}
          </pre>
        )}
      </div>
      <ToolApprovalButtons
        data={{
          id,
          toolName,
          prompt: prompt ?? undefined,
          domain: domain ?? undefined,
          sessionId,
          isClicked,
        }}
      />
    </div>
  );
}
