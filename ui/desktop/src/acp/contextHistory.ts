import type {
  CompactionHistoryPolicyDto,
  CompactionHistoryPurgeMode,
} from '@repo-makeover/gosling-sdk';
import { getAcpClient } from './acpConnection';

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
