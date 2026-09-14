import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { AcpChatPlanState } from '../../acp/chatSessionStore';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { PlanReviewDialog, type PlanReviewDialogProps } from './PlanReviewDialog';

function plan(overrides: Partial<AcpChatPlanState> = {}): AcpChatPlanState {
  return {
    snapshot: {
      plan: {
        id: 'plan-1',
        generation: 2,
        status: 'awaiting_review',
        sourceThroughRowId: 42,
        sourceHash: 'source',
        scopeHash: 'scope',
        capabilityPolicyVersion: 1,
        plannerProvider: 'openai',
        plannerModel: 'gpt-5',
        createdAt: '2026-09-13T00:00:00Z',
        updatedAt: '2026-09-13T00:01:00Z',
      },
      activeRevision: {
        id: 'revision-3',
        revision: 3,
        contentMarkdown: '# Plan\n\nFirst step\nSecond step',
        contentSha256: '0123456789abcdef',
        plannerProvider: 'openai',
        plannerModel: 'gpt-5',
        sourceThroughRowId: 42,
        sourceHash: 'source',
        scopeHash: 'scope',
        createdAt: '2026-09-13T00:01:00Z',
      },
      feedback: [],
      recentEvents: [],
    },
    providerSupportsHostEnforcedPlanning: true,
    permittedCapabilities: ['workspace_read_text'],
    loading: false,
    invalidated: false,
    loadError: undefined,
    latestUpdate: null,
    feedbackDraft: { body: '', startLine: null, endLine: null, revisionId: null },
    actionPending: null,
    workflowMessage: undefined,
    ...overrides,
  };
}

function props(overrides: Partial<PlanReviewDialogProps> = {}): PlanReviewDialogProps {
  return {
    open: true,
    plan: plan(),
    authorizationMode: 'manual',
    sessionBusy: false,
    onOpenChange: vi.fn(),
    onDraftChange: vi.fn(),
    onRequestChanges: vi.fn(),
    onApprove: vi.fn(),
    onApproveAndImplement: vi.fn(),
    onAbandon: vi.fn(),
    onExport: vi.fn(),
    onRefresh: vi.fn(),
    onStartAnother: vi.fn(),
    ...overrides,
  };
}

describe('PlanReviewDialog', () => {
  it('renders exact revision provenance and explains unchanged authorization', () => {
    render(<PlanReviewDialog {...props()} />, { wrapper: IntlTestWrapper });
    expect(screen.getByRole('dialog', { name: 'Plan review' })).toHaveAccessibleDescription(
      /Revision 3 · 0123456789 · source through 42 · gpt-5/
    );
    expect(screen.getByText(/unchanged current authorization mode: manual/)).toBeInTheDocument();
  });

  it('supports keyboard-entered line feedback and all review actions', async () => {
    const user = userEvent.setup();
    const callbacks = props({
      plan: plan({
        feedbackDraft: {
          body: 'Existing feedback.',
          startLine: null,
          endLine: null,
          revisionId: 'revision-3',
        },
      }),
    });
    render(<PlanReviewDialog {...callbacks} />, { wrapper: IntlTestWrapper });

    await user.type(screen.getByLabelText('Start line (optional)'), '3');
    await user.type(screen.getByLabelText('End line (optional)'), '4');
    await user.type(screen.getByLabelText('Feedback'), 'Add validation.');
    expect(callbacks.onDraftChange).toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Request changes' }));
    await user.click(screen.getByRole('button', { name: 'Approve' }));
    await user.click(screen.getByRole('button', { name: 'Approve and implement' }));
    await user.click(screen.getByRole('button', { name: 'Abandon' }));
    await user.click(screen.getByRole('button', { name: 'Export Markdown' }));
    expect(callbacks.onRequestChanges).toHaveBeenCalledOnce();
    expect(callbacks.onApprove).toHaveBeenCalledOnce();
    expect(callbacks.onApproveAndImplement).toHaveBeenCalledOnce();
    expect(callbacks.onAbandon).toHaveBeenCalledOnce();
    expect(callbacks.onExport).toHaveBeenCalledOnce();
  });

  it('disables stale actions, announces the newer revision, and retains draft text', async () => {
    const user = userEvent.setup();
    const onRefresh = vi.fn();
    render(
      <PlanReviewDialog
        {...props({
          onRefresh,
          plan: plan({
            invalidated: true,
            feedbackDraft: {
              body: 'Do not lose this draft.',
              startLine: 2,
              endLine: 3,
              revisionId: 'revision-2',
            },
          }),
        })}
      />,
      { wrapper: IntlTestWrapper }
    );
    expect(screen.getByText(/A newer revision exists/)).toBeVisible();
    expect(screen.getByLabelText('Feedback')).toHaveValue('Do not lose this draft.');
    expect(screen.getByRole('button', { name: 'Approve' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(onRefresh).toHaveBeenCalledOnce();
  });

  it('keeps an approved plan exportable and offers a new generation', async () => {
    const user = userEvent.setup();
    const callbacks = props({
      plan: plan({
        snapshot: {
          ...plan().snapshot!,
          plan: { ...plan().snapshot!.plan, status: 'approved' },
        },
      }),
    });
    render(<PlanReviewDialog {...callbacks} />, { wrapper: IntlTestWrapper });

    await user.click(screen.getByRole('button', { name: 'Export Markdown' }));
    await user.click(screen.getByRole('button', { name: 'Start new plan' }));
    expect(callbacks.onExport).toHaveBeenCalledOnce();
    expect(callbacks.onStartAnother).toHaveBeenCalledOnce();
    expect(screen.queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  });
});
