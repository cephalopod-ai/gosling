import type { PlanSnapshotDto, SessionPlanResponse_unstable } from '@repo-makeover/gosling-sdk';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getAcpClient, getAcpConnectionGeneration } from '../acpConnection';
import { acpChatSessionActions, acpChatSessionStore } from '../chatSessionStore';
import {
  acpAbandonSessionPlan,
  acpAddSessionPlanFeedback,
  acpApproveSessionPlan,
  acpExportSessionPlan,
  approveAndImplementSessionPlan,
  invalidateAcpSessionPlan,
  refreshAcpSessionPlan,
} from '../plans';

vi.mock('../acpConnection', () => ({
  getAcpClient: vi.fn(),
  getAcpConnectionGeneration: vi.fn(),
}));

function snapshot(revision: number, status: PlanSnapshotDto['plan']['status'] = 'awaiting_review') {
  return {
    plan: {
      id: 'plan-1',
      generation: 4,
      status,
      sourceThroughRowId: 42,
      sourceHash: 'source-hash',
      scopeHash: 'scope-hash',
      capabilityPolicyVersion: 1,
      plannerProvider: 'openai',
      plannerModel: 'gpt-5',
      createdAt: '2026-09-13T00:00:00Z',
      updatedAt: '2026-09-13T00:00:01Z',
    },
    activeRevision: {
      id: `revision-${revision}`,
      revision,
      contentMarkdown: `# Plan ${revision}\n\nKeep the authority boundary.`,
      contentSha256: `sha-${revision}`,
      plannerProvider: 'openai',
      plannerModel: 'gpt-5',
      sourceThroughRowId: 42,
      sourceHash: 'source-hash',
      scopeHash: 'scope-hash',
      createdAt: '2026-09-13T00:00:01Z',
    },
    feedback: [],
    recentEvents: [],
  } satisfies PlanSnapshotDto;
}

function response(
  revision: number,
  status: PlanSnapshotDto['plan']['status'] = 'awaiting_review'
): SessionPlanResponse_unstable {
  return {
    snapshot: snapshot(revision, status),
    providerSupportsHostEnforcedPlanning: true,
    permittedCapabilities: ['workspace_read_text', 'plan_update'],
    ...(status === 'approved'
      ? { implementationReference: `server-authored-reference-${revision}` }
      : {}),
  };
}

function createClient() {
  return {
    prompt: vi.fn(),
    gosling: {
      sessionPlanGet_unstable: vi.fn(),
      sessionPlanFeedback_unstable: vi.fn().mockResolvedValue(response(1)),
      sessionPlanApprove_unstable: vi.fn().mockResolvedValue(response(1)),
      sessionPlanAbandon_unstable: vi.fn().mockResolvedValue(response(1)),
      sessionPlanExport_unstable: vi.fn().mockResolvedValue({ markdown: '# Export' }),
    },
  };
}

describe('ACP session plan helpers', () => {
  let client: ReturnType<typeof createClient>;

  beforeEach(() => {
    vi.clearAllMocks();
    client = createClient();
    vi.mocked(getAcpClient).mockResolvedValue(
      client as unknown as Awaited<ReturnType<typeof getAcpClient>>
    );
    vi.mocked(getAcpConnectionGeneration).mockReturnValue(1);
  });

  afterEach(() => {
    acpChatSessionActions.deleteSnapshot('plan-test');
  });

  it('binds feedback, approval, and export to the exact immutable revision', async () => {
    const current = snapshot(3);

    await acpAddSessionPlanFeedback('session-1', current, {
      body: 'Clarify the validation step',
      startLine: 2,
      endLine: 2,
      selectedText: 'Keep the authority boundary.',
    });
    await acpApproveSessionPlan('session-1', current, 'reviewed');
    await acpExportSessionPlan('session-1', current);
    await acpAbandonSessionPlan('session-1', current);

    const revisionFence = {
      expectedGeneration: 4,
      expectedRevisionId: 'revision-3',
      expectedRevisionSha256: 'sha-3',
      expectedSourceHash: 'source-hash',
      expectedScopeHash: 'scope-hash',
    };
    expect(client.gosling.sessionPlanFeedback_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      body: 'Clarify the validation step',
      startLine: 2,
      endLine: 2,
      selectedText: 'Keep the authority boundary.',
      ...revisionFence,
    });
    expect(client.gosling.sessionPlanApprove_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      decisionNote: 'reviewed',
      ...revisionFence,
    });
    expect(client.gosling.sessionPlanExport_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      expectedGeneration: 4,
      expectedRevisionId: 'revision-3',
      expectedRevisionSha256: 'sha-3',
      expectedStatus: 'awaiting_review',
    });
    expect(client.gosling.sessionPlanAbandon_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      expectedGeneration: 4,
    });
  });

  it('does not dispatch revision-bound mutations without an active revision', async () => {
    const current = { ...snapshot(1), activeRevision: null };

    await expect(acpApproveSessionPlan('session-1', current)).rejects.toThrow(
      'Plan generation 4 has no active revision'
    );
    expect(client.gosling.sessionPlanApprove_unstable).not.toHaveBeenCalled();
  });

  it('discards an obsolete refresh after a newer invalidation', async () => {
    let resolveFirst: (value: SessionPlanResponse_unstable) => void = () => undefined;
    const first = new Promise<SessionPlanResponse_unstable>((resolve) => {
      resolveFirst = resolve;
    });
    client.gosling.sessionPlanGet_unstable
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce(response(2));

    invalidateAcpSessionPlan('plan-test');
    await vi.waitFor(() => expect(client.gosling.sessionPlanGet_unstable).toHaveBeenCalledTimes(1));
    invalidateAcpSessionPlan('plan-test');
    resolveFirst(response(1));
    await refreshAcpSessionPlan('plan-test');

    expect(client.gosling.sessionPlanGet_unstable).toHaveBeenCalledTimes(2);
    expect(acpChatSessionStore.getSnapshot('plan-test')?.plan.snapshot?.activeRevision?.id).toBe(
      'revision-2'
    );
  });

  it('discards an obsolete refresh failure after reconnecting', async () => {
    let rejectFirst: (reason: Error) => void = () => undefined;
    const first = new Promise<SessionPlanResponse_unstable>((_resolve, reject) => {
      rejectFirst = reject;
    });
    client.gosling.sessionPlanGet_unstable
      .mockReturnValueOnce(first)
      .mockResolvedValueOnce(response(2));

    const refresh = refreshAcpSessionPlan('plan-test');
    await vi.waitFor(() => expect(client.gosling.sessionPlanGet_unstable).toHaveBeenCalledTimes(1));
    vi.mocked(getAcpConnectionGeneration).mockReturnValue(2);
    rejectFirst(new Error('old connection closed'));
    await refresh;

    expect(client.gosling.sessionPlanGet_unstable).toHaveBeenCalledTimes(2);
    expect(acpChatSessionStore.getSnapshot('plan-test')?.plan).toMatchObject({
      invalidated: false,
      loadError: undefined,
    });
    expect(acpChatSessionStore.getSnapshot('plan-test')?.plan.snapshot?.activeRevision?.id).toBe(
      'revision-2'
    );
  });

  it('plain approval submits no implementation prompt', async () => {
    await acpApproveSessionPlan('session-1', snapshot(1));
    expect(client.gosling.sessionPlanApprove_unstable).toHaveBeenCalledOnce();
    expect(client.prompt).not.toHaveBeenCalled();
  });

  it('single-flights approve and implement and publishes approval before one visible prompt', async () => {
    client.gosling.sessionPlanApprove_unstable.mockResolvedValue(response(1, 'approved'));
    const order: string[] = [];
    const submit = vi.fn(async () => {
      order.push('submit');
      return true;
    });
    const input = {
      sessionId: 'session-1',
      snapshot: snapshot(1),
      onApproved: () => order.push('approved'),
      submit,
    };

    const [first, second] = await Promise.all([
      approveAndImplementSessionPlan(input),
      approveAndImplementSessionPlan(input),
    ]);

    expect(client.gosling.sessionPlanApprove_unstable).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledOnce();
    expect(submit).toHaveBeenCalledWith('server-authored-reference-1');
    expect(submit).not.toHaveBeenCalledWith(expect.stringContaining('# Plan 1'));
    expect(order).toEqual(['approved', 'submit']);
    expect(first.implementationStarted).toBe(true);
    expect(second).toEqual(first);
  });

  it('returns explicit partial success when the post-approval prompt fails', async () => {
    client.gosling.sessionPlanApprove_unstable.mockResolvedValue(response(1, 'approved'));
    const result = await approveAndImplementSessionPlan({
      sessionId: 'session-2',
      snapshot: snapshot(1),
      submit: vi.fn().mockRejectedValue(new Error('offline')),
    });

    expect(result.response.snapshot?.plan.status).toBe('approved');
    expect(result.implementationStarted).toBe(false);
    expect(result.implementationFailure).toBe('submission_failed');
  });

  it('preserves approval and does not submit when the server omits its reference', async () => {
    client.gosling.sessionPlanApprove_unstable.mockResolvedValue({
      ...response(1, 'approved'),
      implementationReference: null,
    });
    const submit = vi.fn();
    const result = await approveAndImplementSessionPlan({
      sessionId: 'session-3',
      snapshot: snapshot(1),
      submit,
    });

    expect(result.response.snapshot?.plan.status).toBe('approved');
    expect(result).toMatchObject({
      implementationStarted: false,
      implementationFailure: 'missing_reference',
    });
    expect(submit).not.toHaveBeenCalled();
  });
});
