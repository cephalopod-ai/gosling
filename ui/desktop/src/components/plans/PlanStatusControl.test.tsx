import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { AcpChatPlanState } from '../../acp/chatSessionStore';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { PlanStatusControl } from './PlanStatusControl';

function emptyPlan(supported = true): AcpChatPlanState {
  return {
    snapshot: null,
    providerSupportsHostEnforcedPlanning: supported,
    permittedCapabilities: [],
    loading: false,
    invalidated: false,
    loadError: undefined,
    latestUpdate: null,
    feedbackDraft: { body: '', startLine: null, endLine: null, revisionId: null },
    actionPending: null,
    workflowMessage: undefined,
  };
}

describe('PlanStatusControl', () => {
  it('starts a plan from the compact keyboard-operable control', async () => {
    const user = userEvent.setup();
    const onStart = vi.fn();
    render(
      <PlanStatusControl plan={emptyPlan()} disabled={false} onStart={onStart} onOpen={vi.fn()} />,
      { wrapper: IntlTestWrapper }
    );
    const button = screen.getByRole('button', { name: 'Start plan' });
    button.focus();
    await user.keyboard('{Enter}');
    expect(onStart).toHaveBeenCalledOnce();
  });

  it('fails closed for a provider that cannot enforce planning', () => {
    render(
      <PlanStatusControl
        plan={emptyPlan(false)}
        disabled={false}
        onStart={vi.fn()}
        onOpen={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );
    expect(
      screen.getByRole('button', {
        name: 'Planning is unavailable because the current provider cannot enforce Gosling planning restrictions.',
      })
    ).toBeDisabled();
  });

  it('opens an approved plan instead of silently replacing it', async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    const approved = emptyPlan();
    approved.snapshot = {
      plan: {
        id: 'plan-1',
        generation: 1,
        status: 'approved',
        sourceThroughRowId: null,
        sourceHash: 'source',
        scopeHash: 'scope',
        capabilityPolicyVersion: 1,
        plannerProvider: 'openai',
        plannerModel: 'gpt-5',
        createdAt: '2026-09-13T00:00:00Z',
        updatedAt: '2026-09-13T00:01:00Z',
      },
      activeRevision: null,
      feedback: [],
      recentEvents: [],
    };
    render(
      <PlanStatusControl plan={approved} disabled={false} onStart={vi.fn()} onOpen={onOpen} />,
      { wrapper: IntlTestWrapper }
    );

    await user.click(screen.getByRole('button', { name: 'Plan approved' }));
    expect(onOpen).toHaveBeenCalledOnce();
  });
});
