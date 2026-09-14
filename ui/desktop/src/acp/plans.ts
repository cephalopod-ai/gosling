import type {
  AddSessionPlanFeedbackRequest_unstable,
  PlanSnapshotDto,
  SessionPlanResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import { getAcpClient, getAcpConnectionGeneration } from './acpConnection';
import { acpChatSessionActions } from './chatSessionStore';
import { describeAcpError } from './errors';

interface RefreshState {
  epoch: number;
  inFlight: Promise<void> | null;
}

interface ApproveAndImplementResult {
  response: SessionPlanResponse_unstable;
  implementationStarted: boolean;
  implementationFailure?: 'missing_reference' | 'submission_failed';
}

const refreshBySessionId = new Map<string, RefreshState>();
const approvalImplementationByRevision = new Map<string, Promise<ApproveAndImplementResult>>();

export async function acpGetSessionPlan(sessionId: string): Promise<SessionPlanResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.sessionPlanGet_unstable({ sessionId });
}

export async function acpStartSessionPlan(
  sessionId: string,
  expectedGeneration?: number
): Promise<SessionPlanResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.sessionPlanStart_unstable({ sessionId, expectedGeneration });
}

function exactRevision(snapshot: PlanSnapshotDto) {
  const revision = snapshot.activeRevision;
  if (!revision) {
    throw new Error(`Plan generation ${snapshot.plan.generation} has no active revision`);
  }
  return {
    expectedGeneration: snapshot.plan.generation,
    expectedRevisionId: revision.id,
    expectedRevisionSha256: revision.contentSha256,
    expectedSourceHash: revision.sourceHash,
    expectedScopeHash: revision.scopeHash,
  };
}

export async function acpAddSessionPlanFeedback(
  sessionId: string,
  snapshot: PlanSnapshotDto,
  feedback: Pick<
    AddSessionPlanFeedbackRequest_unstable,
    'body' | 'startLine' | 'endLine' | 'selectedText'
  >
): Promise<SessionPlanResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.sessionPlanFeedback_unstable({
    sessionId,
    ...feedback,
    ...exactRevision(snapshot),
  });
}

export async function acpApproveSessionPlan(
  sessionId: string,
  snapshot: PlanSnapshotDto,
  decisionNote?: string
): Promise<SessionPlanResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.sessionPlanApprove_unstable({
    sessionId,
    decisionNote,
    ...exactRevision(snapshot),
  });
}

export async function acpAbandonSessionPlan(
  sessionId: string,
  snapshot: PlanSnapshotDto
): Promise<SessionPlanResponse_unstable> {
  const client = await getAcpClient();
  return client.gosling.sessionPlanAbandon_unstable({
    sessionId,
    expectedGeneration: snapshot.plan.generation,
  });
}

export async function acpExportSessionPlan(
  sessionId: string,
  snapshot: PlanSnapshotDto
): Promise<string> {
  const client = await getAcpClient();
  const revision = exactRevision(snapshot);
  const response = await client.gosling.sessionPlanExport_unstable({
    sessionId,
    expectedGeneration: revision.expectedGeneration,
    expectedRevisionId: revision.expectedRevisionId,
    expectedRevisionSha256: revision.expectedRevisionSha256,
    expectedStatus: snapshot.plan.status,
  });
  return response.markdown;
}

/** Compact plan notifications invalidate cache; only this fenced read supplies authority. */
export function invalidateAcpSessionPlan(sessionId: string): void {
  const state = refreshBySessionId.get(sessionId) ?? { epoch: 0, inFlight: null };
  state.epoch += 1;
  refreshBySessionId.set(sessionId, state);
  acpChatSessionActions.startPlanLoad(sessionId);
  void refreshAcpSessionPlan(sessionId);
}

export function refreshAcpSessionPlan(sessionId: string): Promise<void> {
  const state = refreshBySessionId.get(sessionId) ?? { epoch: 0, inFlight: null };
  refreshBySessionId.set(sessionId, state);
  if (state.inFlight) return state.inFlight;

  const load = (async () => {
    for (;;) {
      const requestedEpoch = state.epoch;
      const requestedConnection = getAcpConnectionGeneration();
      try {
        const response = await acpGetSessionPlan(sessionId);
        if (
          requestedEpoch !== state.epoch ||
          requestedConnection !== getAcpConnectionGeneration()
        ) {
          continue;
        }
        acpChatSessionActions.setPlanResponse(sessionId, response);
        return;
      } catch (error) {
        if (
          requestedEpoch !== state.epoch ||
          requestedConnection !== getAcpConnectionGeneration()
        ) {
          continue;
        }
        acpChatSessionActions.failPlanLoad(sessionId, describeAcpError(error));
        return;
      }
    }
  })().finally(() => {
    if (state.inFlight === load) state.inFlight = null;
  });
  state.inFlight = load;
  return load;
}

export function approveAndImplementSessionPlan(input: {
  sessionId: string;
  snapshot: PlanSnapshotDto;
  onApproved?(response: SessionPlanResponse_unstable): void;
  submit(prompt: string): Promise<boolean>;
}): Promise<ApproveAndImplementResult> {
  const revision = exactRevision(input.snapshot);
  const key = `${input.sessionId}:${revision.expectedGeneration}:${revision.expectedRevisionId}:${revision.expectedRevisionSha256}`;
  const current = approvalImplementationByRevision.get(key);
  if (current) return current;

  const operation: Promise<ApproveAndImplementResult> =
    (async (): Promise<ApproveAndImplementResult> => {
      const response = await acpApproveSessionPlan(
        input.sessionId,
        input.snapshot,
        'approved and submitted for implementation from Desktop'
      );
      const approved = response.snapshot;
      if (!approved || approved.plan.status !== 'approved') {
        throw new Error('The server did not return an approved plan snapshot');
      }
      input.onApproved?.(response);
      const reference = response.implementationReference;
      if (typeof reference !== 'string' || reference.length === 0) {
        return {
          response,
          implementationStarted: false,
          implementationFailure: 'missing_reference',
        };
      }
      try {
        const implementationStarted = await input.submit(reference);
        return {
          response,
          implementationStarted,
          ...(implementationStarted ? {} : { implementationFailure: 'submission_failed' as const }),
        };
      } catch {
        // Approval is already durable. The caller must surface this explicit partial success.
        return {
          response,
          implementationStarted: false,
          implementationFailure: 'submission_failed',
        };
      }
    })().finally(() => approvalImplementationByRevision.delete(key));
  approvalImplementationByRevision.set(key, operation);
  return operation;
}
