import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { IntlProvider } from 'react-intl';
import type { AcpChatPlanState } from './acp/chatSessionStore';
import { PlanReviewDialog } from './components/plans/PlanReviewDialog';
import { PlanStatusControl } from './components/plans/PlanStatusControl';
import { applyThemeTokens } from './theme/theme-tokens';
import './styles/main.css';

declare global {
  interface Window {
    planReviewPlaytest: {
      lastAction: string | null;
    };
  }
}

window.planReviewPlaytest = { lastAction: null };

const initialPlan: AcpChatPlanState = {
  snapshot: {
    plan: {
      id: 'plan-playtest',
      generation: 4,
      status: 'awaiting_review',
      sourceThroughRowId: 87,
      sourceHash: 'source-hash-playtest',
      scopeHash: 'scope-hash-playtest',
      capabilityPolicyVersion: 1,
      plannerProvider: 'openai',
      plannerModel: 'gpt-5',
      createdAt: '2026-09-13T12:00:00Z',
      updatedAt: '2026-09-13T12:01:00Z',
    },
    activeRevision: {
      id: 'revision-playtest',
      revision: 7,
      contentMarkdown: [
        '# Persisted plan review',
        '',
        '1. Load the exact active revision from the host.',
        '2. Keep feedback attached to revision 7.',
        '3. Require compare-and-swap approval.',
        '4. Submit one ordinary prompt only after approval.',
        '5. Keep the approved state if prompt submission fails.',
        '',
        '## Validation',
        '',
        '- Check compact and standard window layouts.',
        '- Keep every review action keyboard reachable.',
        '- Preserve the operator feedback draft.',
      ].join('\n'),
      contentSha256: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
      plannerProvider: 'openai',
      plannerModel: 'gpt-5',
      sourceThroughRowId: 87,
      sourceHash: 'source-hash-playtest',
      scopeHash: 'scope-hash-playtest',
      createdAt: '2026-09-13T12:01:00Z',
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
  feedbackDraft: {
    body: 'Retain this draft while the review window is open.',
    startLine: 2,
    endLine: 4,
    revisionId: 'revision-playtest',
  },
  actionPending: null,
  workflowMessage: 'Revision 7 is ready for review.',
};

function PlanReviewPlaytest() {
  const [plan, setPlan] = useState(initialPlan);
  const [open, setOpen] = useState(true);
  const recordAction = (action: string) => {
    window.planReviewPlaytest.lastAction = action;
  };

  return (
    <main className="flex h-screen items-end justify-center bg-background-primary p-4 text-text-primary">
      <PlanStatusControl
        plan={plan}
        disabled={false}
        onStart={() => recordAction('start')}
        onOpen={() => setOpen(true)}
      />
      <PlanReviewDialog
        open={open}
        plan={plan}
        authorizationMode="manual"
        sessionBusy={false}
        onOpenChange={setOpen}
        onDraftChange={(draft) =>
          setPlan((current) => ({
            ...current,
            feedbackDraft: { ...current.feedbackDraft, ...draft },
          }))
        }
        onRequestChanges={() => recordAction('request-changes')}
        onApprove={() => recordAction('approve')}
        onApproveAndImplement={() => recordAction('approve-and-implement')}
        onAbandon={() => recordAction('abandon')}
        onExport={() => recordAction('export')}
        onRefresh={() => recordAction('refresh')}
        onStartAnother={() => recordAction('start-another')}
      />
    </main>
  );
}

document.documentElement.classList.add('dark');
applyThemeTokens('dark');

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <IntlProvider locale="en" defaultLocale="en" messages={{}}>
      <PlanReviewPlaytest />
    </IntlProvider>
  </StrictMode>
);
