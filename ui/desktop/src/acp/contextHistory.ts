import type {
  CompactionHistoryPolicyDto,
  CompactionHistoryPurgeMode,
} from '@repo-makeover/gosling-sdk';
import type { Message } from '../types/message';
import { getAcpClient } from './acpConnection';
import { acpListSessionMessages } from './sessions';

export interface ContextHistorySourceMessages {
  messages: Message[];
  unavailableCount: number;
}

export async function getContextHistory(
  sessionId: string,
  beforeGeneration?: number,
  includeExpired: boolean = true
) {
  const client = await getAcpClient();
  return client.gosling.sessionCompactionsHistory_unstable({
    sessionId,
    beforeGeneration,
    includeExpired,
    limit: 50,
  });
}

export async function getContextHistoryRevision(sessionId: string, generation: number) {
  const client = await getAcpClient();
  return client.gosling.sessionCompactionsRevision_unstable({ sessionId, generation });
}

export async function getContextHistorySourceMessages(
  sessionId: string,
  sourceMessageIds: string[],
  sourceMessageCount: number
): Promise<ContextHistorySourceMessages> {
  const orderedIds = [...new Set(sourceMessageIds)];
  if (orderedIds.length === 0) {
    return { messages: [], unavailableCount: sourceMessageCount };
  }

  const pendingIds = new Set(orderedIds);
  const messagesById = new Map<string, Message>();
  const firstSourceMessageId = orderedIds[0];
  let beforeCursor: string | null = null;

  do {
    const page = await acpListSessionMessages(sessionId, beforeCursor);
    let reachedFirstSourceMessage = false;
    for (const message of page.messages) {
      if (!message.id) continue;
      if (message.id === firstSourceMessageId) {
        reachedFirstSourceMessage = true;
      }
      if (pendingIds.delete(message.id)) {
        messagesById.set(message.id, message);
      }
    }

    beforeCursor = page.nextBeforeCursor;
    if (pendingIds.size === 0 || reachedFirstSourceMessage) {
      break;
    }
  } while (beforeCursor !== null);

  const messages = orderedIds.flatMap((id) => {
    const message = messagesById.get(id);
    return message ? [message] : [];
  });
  return {
    messages,
    unavailableCount: Math.max(0, sourceMessageCount - messages.length),
  };
}

export async function setContextHistoryPinned(
  sessionId: string,
  generation: number,
  pinned: boolean
) {
  const client = await getAcpClient();
  return client.gosling.sessionCompactionsPin_unstable({ sessionId, generation, pinned });
}

export async function deleteContextHistoryRevision(sessionId: string, generation: number) {
  const client = await getAcpClient();
  return client.gosling.sessionCompactionsDelete_unstable({ sessionId, generation });
}

export async function purgeContextHistory(
  sessionId: string,
  mode: CompactionHistoryPurgeMode = 'expired'
) {
  const client = await getAcpClient();
  return client.gosling.sessionCompactionsPurge_unstable({ sessionId, mode });
}

export async function readContextHistoryPolicy() {
  const client = await getAcpClient();
  return client.gosling.contextHistoryPolicy_unstable({});
}

export async function previewContextHistoryPolicy(policy: CompactionHistoryPolicyDto) {
  const client = await getAcpClient();
  return client.gosling.contextHistoryPolicyPreview_unstable({ policy });
}

export async function applyContextHistoryPolicy(
  policy: CompactionHistoryPolicyDto,
  expectedPreviewHash: string
) {
  const client = await getAcpClient();
  return client.gosling.contextHistoryPolicyApply_unstable({ policy, expectedPreviewHash });
}
