import type { Message } from '../../types/message';
import type { AcpElicitationRequest } from '../elicitationRequests';
import {
  type AcpChatStateChange,
  type AdapterState,
  DEFAULT_VISIBLE_MESSAGE_METADATA,
  messageUpserted,
} from './shared';

export type ElicitationStatus = 'submitted' | 'cancelled';

export function applyElicitationRequest(
  state: AdapterState,
  request: AcpElicitationRequest
): AcpChatStateChange[] {
  if (hasExistingElicitation(state, request.id)) {
    return [];
  }

  const message: Message = {
    id: request.id,
    role: 'assistant',
    created: Math.floor(Date.now() / 1000),
    content: [
      {
        type: 'actionRequired',
        data: {
          actionType: 'elicitation',
          id: request.id,
          message: request.request.message,
          requested_schema: request.request.requestedSchema,
        },
      },
    ],
    metadata: { ...DEFAULT_VISIBLE_MESSAGE_METADATA },
  };
  state.messages.push(message);

  return [messageUpserted(state, message)];
}

export function applyElicitationStatus(
  state: AdapterState,
  elicitationId: string,
  status: ElicitationStatus
): AcpChatStateChange[] {
  const statusData = {
    isSubmitted: status === 'submitted',
    isCancelled: status === 'cancelled',
  };
  const changes: AcpChatStateChange[] = [];

  state.messages = state.messages.map((message, index) => {
    let messageChanged = false;
    const content = message.content.map((content) => {
      if (
        content.type !== 'actionRequired' ||
        content.data.actionType !== 'elicitation' ||
        content.data.id !== elicitationId
      ) {
        return content;
      }

      messageChanged = true;
      return {
        ...content,
        data: {
          ...content.data,
          ...statusData,
        },
      };
    });

    if (!messageChanged) {
      return message;
    }

    const updatedMessage = { ...message, content };
    changes.push(messageUpserted(state, updatedMessage, index));
    return updatedMessage;
  });

  return changes;
}

function hasExistingElicitation(state: AdapterState, elicitationId: string): boolean {
  return state.messages.some((message: Message) =>
    message.content.some(
      (content) =>
        content.type === 'actionRequired' &&
        content.data.actionType === 'elicitation' &&
        content.data.id === elicitationId
    )
  );
}
