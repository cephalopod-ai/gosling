import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';
import { ShellApp } from './ShellApp';
import { createFakeShellApi, type FakeShellApi } from './fakeShellApi';
import { createShellStore, type ShellStore } from './state/store';
import {
  ALL_CAPABILITIES,
  activeSession,
  confirmInteraction,
  elicitationInteraction,
  permissionInteraction,
  settings,
  snapshot,
  update,
} from './testSupport';
import type { ShellRuntimeSnapshot } from '../shell/runtimeSnapshot';
import type { ShellLifecycleStateName } from '../shell/lifecycle';

const PRODUCT = 'Default Shell Template';

async function mount(
  overrides: Partial<ShellRuntimeSnapshot> = {},
  fakeOverrides: Partial<Parameters<typeof createFakeShellApi>[0]> = {}
): Promise<{ fake: FakeShellApi; store: ShellStore }> {
  const fake = createFakeShellApi({ snapshot: snapshot(overrides), ...fakeOverrides });
  const store = createShellStore(fake.api);
  render(<ShellApp store={store} productName={PRODUCT} />);
  await act(async () => {
    await store.start();
  });
  return { fake, store };
}

beforeEach(() => {
  document.documentElement.removeAttribute('data-gsh-theme');
});

describe('state matrix', () => {
  const LIFECYCLE_STATES: ShellLifecycleStateName[] = [
    'booting',
    'validating',
    'ready',
    'busy',
    'degraded',
    'relink_required',
    'incompatible',
    'offline',
    'stopping',
    'stopped',
    'fatal',
  ];

  it.each(LIFECYCLE_STATES)(
    'renders lifecycle state %s without throwing',
    async (lifecycleState) => {
      await mount({ lifecycleState });
      expect(document.querySelector('.gsh-app')).not.toBeNull();
    }
  );

  it.each([
    ['unselected', null],
    ['selected', '/work/project'],
    ['missing', '/work/gone'],
    ['invalid', '/work/bad'],
  ] as const)('renders directory state %s', async (state, path) => {
    await mount({
      directory: {
        state,
        path,
        label: path ? 'project' : null,
        reasonCode: state === 'invalid' ? 'directory_not_allowed' : null,
        remembered: state === 'missing',
      },
    });
    expect(document.querySelector('.gsh-app')).not.toBeNull();
  });

  it.each(['available', 'denied', 'unavailable'] as const)(
    'renders credential catalog status %s',
    async (catalogStatus) => {
      await mount({
        credentials: {
          catalogStatus,
          profiles: [],
          selectedProfileId: null,
          selectionStatus: 'none',
        },
        session: null,
      });
      expect(document.querySelector('.gsh-app')).not.toBeNull();
    }
  );

  it.each(['none', 'configured', 'relink_required', 'missing'] as const)(
    'renders credential selection status %s',
    async (selectionStatus) => {
      await mount({
        credentials: {
          catalogStatus: 'available',
          profiles: [
            { id: 'cred-1', name: 'Work', providerOrServiceId: 'anthropic', status: 'configured' },
          ],
          selectedProfileId: selectionStatus === 'none' ? null : 'cred-1',
          selectionStatus,
        },
      });
      expect(document.querySelector('.gsh-app')).not.toBeNull();
    }
  );

  it.each(['loaded', 'absent', 'unsupported_schema', 'malformed', 'unreadable'] as const)(
    'keeps settings controls unavailable for recovery status %s',
    async (status) => {
      const { store } = await mount({ settingsRecovery: { status, schemaVersion: 1 } });
      act(() => {
        store.dispatch({
          type: 'settings/replaced',
          settings: settings({ recovery: { status, schemaVersion: 1 } }),
        });
        store.actions.setView('settings');
      });
      expect(document.querySelector('.gsh-settings')).toBeNull();
      expect(screen.queryByRole('button', { name: 'Settings' })).toBeNull();
    }
  );

  it.each(['ready', 'unavailable', 'incompatible'] as const)(
    'renders module status %s',
    async (status) => {
      await mount({
        modules: [
          { id: 'core:session', kind: 'core', status: 'ready' },
          { id: 'files', kind: 'extension', status },
        ],
      });
      expect(document.querySelector('.gsh-modules')).not.toBeNull();
    }
  );

  it.each(['ready', 'crashed', 'hung', 'incompatible'] as const)(
    'renders adapter status %s',
    async (status) => {
      await mount({
        adapter: { descriptorId: 'example', protocolVersion: '1', actions: ['read'], status },
      });
      expect(document.querySelector('.gsh-modules')).not.toBeNull();
    }
  );

  it('hides the modules strip when only core:session is present', async () => {
    await mount({
      modules: [{ id: 'core:session', kind: 'core', status: 'ready' }],
      adapter: null,
    });
    expect(document.querySelector('.gsh-modules')).toBeNull();
  });

  it('shows provisioning issues without leaking a raw backend error', async () => {
    await mount({ provisioningIssues: [{ code: 'missing_skill', path: 'session.skillIds[0]' }] });
    expect(screen.getByText(/setup couldn't be applied/i)).toBeInTheDocument();
  });
});

describe('default dashboard', () => {
  it('uses the Gosling desktop navigation without global administration links', async () => {
    await mount({ session: null });

    expect(screen.getByRole('button', { name: 'New Chat' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Session History' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Settings' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Extensions' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Skills' })).toBeNull();
  });

  it('shows no account catalog or controls when credentials are owned by main Gosling', async () => {
    await mount({
      session: null,
      declaredCapabilities: ALL_CAPABILITIES.filter(
        (capability) => capability !== 'credential.select'
      ),
      credentials: {
        catalogStatus: 'denied',
        profiles: [],
        selectedProfileId: null,
        selectionStatus: 'none',
      },
    });

    expect(screen.queryByText('Account')).toBeNull();
    expect(screen.queryByText('No account chosen')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Change account' })).toBeNull();
    expect(screen.queryByRole('heading', { name: 'Choose an account' })).toBeNull();
  });

  it('renders workspace, task, and recent-task panels with their declared actions', async () => {
    await mount({ session: null });

    expect(screen.getByRole('navigation', { name: 'Workspace' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Tasks' })).toBeInTheDocument();
    expect(screen.getAllByRole('complementary')).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Start new task' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeInTheDocument();
  });

  it('starts a session from the New Chat navigation action', async () => {
    const { fake } = await mount({ session: null });

    await userEvent.click(screen.getByRole('button', { name: 'New Chat' }));

    await waitFor(() => {
      expect(fake.calls.some((call) => call.operation === 'session.create')).toBe(true);
    });
  });

  it('detaches an active session before starting a new chat', async () => {
    const { fake } = await mount();
    const callsBeforeClick = fake.calls.length;

    await userEvent.click(screen.getByRole('button', { name: 'New Chat' }));

    await waitFor(() => {
      expect(
        fake.calls
          .slice(callsBeforeClick)
          .map((call) => call.operation)
          .filter((operation) =>
            ['session.detach', 'session.list', 'session.create'].includes(operation)
          )
      ).toEqual(['session.detach', 'session.list', 'session.create']);
    });
  });

  it('does not create a new chat when detaching the active session fails', async () => {
    const { fake } = await mount();
    fake.failNext('session.detach', new Error('detach failed'));

    await userEvent.click(screen.getByRole('button', { name: 'New Chat' }));

    await waitFor(() => {
      expect(fake.calls.some((call) => call.operation === 'session.detach')).toBe(true);
    });
    expect(fake.calls.some((call) => call.operation === 'session.create')).toBe(false);
  });

  it('routes dashboard actions through the existing typed operations', async () => {
    const { fake } = await mount({ session: null });
    await userEvent.click(screen.getByRole('button', { name: 'Change folder' }));
    await userEvent.click(screen.getByRole('button', { name: 'Change account' }));

    await waitFor(() => {
      expect(fake.calls.map((call) => call.operation)).toEqual(
        expect.arrayContaining(['directory.select', 'credential.select'])
      );
    });

    const second = await mount({ session: null });
    const startButtons = screen.getAllByRole('button', { name: 'Start new task' });
    await userEvent.click(startButtons[startButtons.length - 1]);
    await waitFor(() => {
      expect(second.fake.calls.some((call) => call.operation === 'session.create')).toBe(true);
    });
  });
});

describe('lifecycle actions derive from allowedActions', () => {
  it.each([
    ['relink_required', ['stop', 'diagnostics']],
    ['incompatible', ['stop', 'diagnostics']],
    ['fatal', ['stop', 'diagnostics']],
  ] as const)('offers no retry in %s', async (lifecycleState, allowedActions) => {
    await mount({ lifecycleState, allowedActions: [...allowedActions] });
    expect(screen.queryByRole('button', { name: 'Retry' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Save diagnostics' })).toBeInTheDocument();
  });

  it.each([
    ['degraded', ['retry', 'stop', 'diagnostics', 'handoff']],
    ['offline', ['retry', 'stop', 'diagnostics']],
  ] as const)('offers retry in %s', async (lifecycleState, allowedActions) => {
    await mount({ lifecycleState, allowedActions: [...allowedActions] });
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
  });

  it('offers restart from stopped, which the host models as a new generation', async () => {
    const { fake } = await mount({ lifecycleState: 'stopped', allowedActions: [] });
    await userEvent.click(screen.getByRole('button', { name: 'Restart' }));
    await waitFor(() => {
      expect(fake.calls.some((call) => call.operation === 'runtime.retry')).toBe(true);
    });
  });
});

describe('permission requests', () => {
  it('renders only the buttons the summary permits', async () => {
    const { fake } = await mount();
    act(() =>
      fake.emitInteraction(
        permissionInteraction({
          summary: {
            toolTitle: 'read_file',
            effect: 'read',
            targets: ['README.md'],
            inputFields: ['path'],
            allowOnce: true,
            deny: false,
          },
        })
      )
    );
    expect(screen.getByRole('button', { name: 'Allow once' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Deny' })).toBeNull();
  });

  it('never offers an always-allow control', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(permissionInteraction()));
    expect(screen.queryByRole('button', { name: /always/i })).toBeNull();
  });

  it('survives streamed tool progress', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(permissionInteraction()));
    act(() =>
      fake.emitSessionUpdate(
        update({
          updateSeq: 5,
          kind: 'stream',
          stream: {
            type: 'tool',
            toolCallId: 't1',
            title: 'write',
            toolKind: 'edit',
            status: 'pending',
          },
        })
      )
    );
    expect(screen.getByRole('button', { name: 'Allow once' })).toBeInTheDocument();
  });

  it('sends exactly one response for a double click', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(permissionInteraction()));
    const allow = screen.getByRole('button', { name: 'Allow once' });
    await userEvent.dblClick(allow);
    await waitFor(() => {
      expect(fake.calls.filter((call) => call.operation === 'permission.respond')).toHaveLength(1);
    });
  });

  it('presents one interaction and counts the rest', async () => {
    const { fake } = await mount();
    act(() => {
      fake.emitInteraction(permissionInteraction({ actionId: 'a1' }));
      fake.emitInteraction(confirmInteraction({ actionId: 'a2' }));
    });
    expect(screen.getByText('1 more waiting')).toBeInTheDocument();
    expect(screen.getAllByRole('region', { name: 'Pending request' })).toHaveLength(1);
  });

  it('blocks the composer while a request is pending', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(permissionInteraction()));
    expect(screen.getByLabelText('Your request')).toBeDisabled();
  });
});

describe('elicitation forms', () => {
  it('blocks submit until required fields are answered', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(elicitationInteraction()));
    const submit = screen.getByRole('button', { name: 'Submit' });
    expect(submit).toBeDisabled();
    await userEvent.type(screen.getByLabelText(/Version/), '1.4.0');
    expect(screen.getByRole('button', { name: 'Submit' })).toBeEnabled();
  });

  it('sends decline and cancel as distinct terminal answers', async () => {
    const { fake } = await mount();
    act(() => fake.emitInteraction(elicitationInteraction()));
    await userEvent.click(screen.getByRole('button', { name: 'Decline' }));
    await waitFor(() => {
      const call = fake.calls.find((entry) => entry.operation === 'elicitation.respond');
      expect(call?.request).toMatchObject({ action: 'decline' });
    });
  });

  it('coerces numeric fields before submitting', async () => {
    const { fake } = await mount();
    act(() =>
      fake.emitInteraction(
        elicitationInteraction({
          summary: {
            message: 'How many?',
            title: null,
            description: null,
            toolCallId: null,
            fields: [
              { name: 'count', label: 'Count', description: null, required: true, type: 'integer' },
            ],
          },
        })
      )
    );
    await userEvent.type(screen.getByLabelText(/Count/), '7');
    await userEvent.click(screen.getByRole('button', { name: 'Submit' }));
    await waitFor(() => {
      const call = fake.calls.find((entry) => entry.operation === 'elicitation.respond');
      expect(call?.request).toMatchObject({ action: 'submit', fields: { count: 7 } });
    });
  });
});

describe('composer', () => {
  it('sends the draft and clears it on success', async () => {
    const { fake } = await mount();
    await userEvent.type(screen.getByLabelText('Your request'), 'do the thing');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => {
      const call = fake.calls.find((entry) => entry.operation === 'prompt.submit');
      expect(call?.request).toMatchObject({ text: 'do the thing', sessionId: 'sess-1' });
    });
    expect(screen.getByLabelText('Your request')).toHaveValue('');
  });

  it('restores the exact submitted text when the failure preserves the draft', async () => {
    const { fake } = await mount();
    fake.failNext('prompt.submit', {
      code: 'RUNTIME_UNAVAILABLE',
      message: 'The shell backend is not currently available.',
      retrySafe: true,
      recovery: 'retry',
      preservesDraft: true,
    });
    await userEvent.type(screen.getByLabelText('Your request'), 'keep me');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => {
      expect(screen.getByLabelText('Your request')).toHaveValue('keep me');
    });
    expect(screen.getByRole('alert')).toHaveTextContent('RUNTIME_UNAVAILABLE');
  });

  it('shows Stop with the exact prompt attempt id while streaming', async () => {
    const { fake } = await mount({
      session: activeSession({ promptAttempt: { id: 'attempt-9', phase: 'streaming' } }),
    });
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    await waitFor(() => {
      const call = fake.calls.find((entry) => entry.operation === 'prompt.cancel');
      expect(call?.request).toMatchObject({ promptAttemptId: 'attempt-9' });
    });
  });

  it('disables the stop control while cancelling', async () => {
    await mount({
      session: activeSession({ promptAttempt: { id: 'attempt-9', phase: 'cancelling' } }),
    });
    expect(screen.getByRole('button', { name: 'Stopping…' })).toBeDisabled();
  });

  it('blocks send past the prompt byte limit', async () => {
    const { store } = await mount();
    act(() => store.actions.setDraft('x'.repeat(64 * 1024 + 1)));
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    expect(screen.getByText(/65,537 \/ 65,536 bytes/)).toBeInTheDocument();
  });
});

describe('capability gating', () => {
  it('hides controls the consumer did not declare', async () => {
    await mount({ declaredCapabilities: [] });
    expect(screen.queryByRole('button', { name: 'Change' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
  });

  it('never invokes an undeclared capability', async () => {
    const { fake, store } = await mount({ declaredCapabilities: [] });
    await act(async () => {
      await store.actions.selectDirectory();
      await store.actions.createSession();
      await store.actions.submitPrompt();
    });
    const gated = fake.calls.filter((call) =>
      ['directory.select', 'session.create', 'prompt.submit'].includes(call.operation)
    );
    expect(gated).toEqual([]);
  });

  it('declares every capability the neutral template ships with', async () => {
    const { store } = await mount();
    for (const capability of ALL_CAPABILITIES) {
      expect(store.getState().snapshot?.declaredCapabilities).toContain(capability);
    }
  });
});

describe('accessibility', () => {
  it('moves focus to the dock when a request arrives and returns it afterwards (A-2, A-3)', async () => {
    const { fake } = await mount();
    const composer = screen.getByLabelText('Your request');
    composer.focus();
    expect(document.activeElement).toBe(composer);

    act(() => fake.emitInteraction(permissionInteraction()));
    const dock = screen.getByRole('region', { name: 'Pending request' });
    expect(document.activeElement).toBe(dock);

    await userEvent.click(screen.getByRole('button', { name: 'Allow once' }));
    act(() =>
      fake.emitSessionUpdate(update({ updateSeq: 7, kind: 'completed', stream: undefined }))
    );
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByLabelText('Your request'));
    });
  });

  it('returns focus to the composer when the generation advances (A-4)', async () => {
    const { fake, store } = await mount();
    act(() => store.actions.setDraft('unsent work'));
    act(() => fake.emitRuntime(snapshot({ generation: 2 })));
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByLabelText('Your request'));
    });
    expect(screen.getByLabelText('Your request')).toHaveValue('unsent work');
  });

  it('announces lifecycle failures assertively (A-8)', async () => {
    await mount({ lifecycleState: 'offline', allowedActions: ['retry', 'stop', 'diagnostics'] });
    expect(screen.getByRole('alert')).toHaveTextContent(/can't reach its backend/i);
  });

  it('reports status with text rather than colour alone (A-9)', async () => {
    await mount({ lifecycleState: 'busy' });
    expect(screen.getAllByText('working')).toHaveLength(2);
  });

  it('applies the theme and text scale from settings (A-5)', async () => {
    const { store } = await mount();
    act(() => {
      store.dispatch({
        type: 'settings/replaced',
        settings: settings({ appearance: { theme: 'dark', textScale: 2 } }),
      });
    });
    await waitFor(() => {
      expect(document.documentElement.getAttribute('data-gsh-theme')).toBe('dark');
    });
    expect(document.documentElement.style.getPropertyValue('--gsh-text-scale')).toBe('2');
  });

  it('does not expose provider or model settings controls', async () => {
    const { store } = await mount();
    act(() => {
      store.dispatch({
        type: 'settings/replaced',
        settings: settings({
          modelSelection: {
            status: 'available',
            providerId: 'anthropic',
            modelId: 'claude-sonnet-4-5',
            options: [
              {
                providerId: 'anthropic',
                providerName: 'Anthropic',
                modelId: 'claude-sonnet-4-5',
                modelName: 'Claude Sonnet 4.5',
              },
            ],
          },
        }),
      });
      store.actions.setView('settings');
    });
    expect(screen.queryByRole('button', { name: 'Apply model' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Settings' })).toBeNull();
  });

  it('labels the transcript gap notice as a live status', async () => {
    const { store } = await mount();
    act(() =>
      store.dispatch({
        type: 'transcript/loaded',
        transcript: {
          generation: 1,
          sessionId: 'sess-1',
          integrity: 'incomplete',
          firstSeq: 4,
          lastSeq: 6,
          truncated: true,
          updates: [],
        },
      })
    );
    const status = screen.getAllByRole('status');
    expect(status.some((node) => within(node).queryByText(/isn't shown/i))).toBe(true);
  });
});

describe('sessions and transcript', () => {
  it('sends selected library references even when the text composer is empty', async () => {
    const { fake, store } = await mount();
    act(() => {
      store.dispatch({
        type: 'library/loaded',
        items: [
          {
            id: 'lib-reference',
            name: 'Reference.pdf',
            kind: 'file',
            scope: 'project',
            status: 'available',
            mimeType: 'application/pdf',
            sizeBytes: 1024,
          },
        ],
      });
    });
    await userEvent.click(screen.getByRole('checkbox'));
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(fake.calls.find((call) => call.operation === 'prompt.submit')?.request).toEqual({
      generation: 1,
      sessionId: 'sess-1',
      text: '',
      libraryItemIds: ['lib-reference'],
    });
  });

  it('omits the library when the consumer does not declare library read access', async () => {
    const capabilities = ALL_CAPABILITIES.filter(
      (capability) => !capability.startsWith('session.library.')
    );
    const { fake } = await mount({ declaredCapabilities: capabilities });
    expect(screen.queryByRole('heading', { name: 'Library' })).toBeNull();
    expect(fake.calls.some((call) => call.operation === 'session.library.read')).toBe(false);
  });

  it('loads and renders only the safe Outputs projection for the active session', async () => {
    const { fake } = await mount();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Outputs' })).toBeInTheDocument();
      expect(screen.getByText('summary.md')).toBeInTheDocument();
    });
    expect(fake.calls.find((call) => call.operation === 'session.artifacts.read')?.request).toEqual(
      {
        generation: 1,
        sessionId: 'sess-1',
      }
    );
    expect(document.body.textContent).not.toContain('/work/project');

    fake.emitSessionUpdate(update({ updateSeq: 7, kind: 'completed', stream: undefined }));
    await waitFor(() => {
      expect(fake.calls.filter((call) => call.operation === 'session.artifacts.read')).toHaveLength(
        2
      );
    });
  });

  it('does not render or request Outputs when the consumer omits the capability', async () => {
    const capabilities = ALL_CAPABILITIES.filter(
      (capability) => capability !== 'session.artifacts.read'
    );
    const { fake } = await mount({ declaredCapabilities: capabilities });
    expect(screen.queryByRole('heading', { name: 'Outputs' })).toBeNull();
    expect(fake.calls.some((call) => call.operation === 'session.artifacts.read')).toBe(false);
  });

  it('states the twenty-session cap in the heading', async () => {
    await mount({ session: null });
    await waitFor(() => {
      expect(screen.getByText('Recent tasks in this folder (up to 20)')).toBeInTheDocument();
    });
  });

  it('reads the transcript after resuming', async () => {
    const { fake, store } = await mount({ session: null });
    await act(async () => {
      await store.actions.resumeSession('sess-1');
    });
    expect(fake.calls.some((call) => call.operation === 'session.resume')).toBe(true);
    expect(fake.calls.some((call) => call.operation === 'session.transcript.read')).toBe(true);
  });

  it('renders the history and live seam once', async () => {
    const { fake } = await mount();
    act(() => {
      fake.emitSessionUpdate(
        update({
          updateSeq: 1,
          delivery: 'history',
          stream: { type: 'content', role: 'user', messageId: 'm1', text: 'old' },
        })
      );
      fake.emitSessionUpdate(
        update({
          updateSeq: 2,
          delivery: 'live',
          stream: { type: 'content', role: 'assistant', messageId: 'm2', text: 'new' },
        })
      );
    });
    expect(screen.getAllByText('resumed here')).toHaveLength(1);
  });

  it('never renders an agent thought, because main drops it before the renderer', async () => {
    const { fake } = await mount();
    act(() =>
      fake.emitSessionUpdate(
        update({
          updateSeq: 3,
          stream: { type: 'content', role: 'assistant', messageId: 'm', text: 'visible' },
        })
      )
    );
    expect(screen.getByText('visible')).toBeInTheDocument();
    expect(screen.queryByText(/thinking/i)).toBeNull();
  });
});

describe('teardown', () => {
  it('removes every event subscription on dispose', async () => {
    const { fake, store } = await mount();
    expect(Object.values(fake.listenerCounts()).every((count) => count === 1)).toBe(true);
    store.dispose();
    expect(Object.values(fake.listenerCounts()).every((count) => count === 0)).toBe(true);
  });
});

describe('audit regressions', () => {
  // SHP-DEF-055: states without a live ACP connection and session must not advertise handoff.
  it.each([
    ['relink_required', ['stop', 'diagnostics']],
    ['incompatible', ['stop', 'diagnostics']],
  ] as const)('offers no dead handoff button in %s', async (lifecycleState, allowedActions) => {
    await mount({
      lifecycleState,
      allowedActions: [...allowedActions],
      identity: null,
      session: null,
    });
    expect(screen.queryByRole('button', { name: 'Open in Gosling' })).toBeNull();
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('offers handoff when a live connection and session exist', async () => {
    const { fake } = await mount({
      lifecycleState: 'degraded',
      allowedActions: ['retry', 'stop', 'diagnostics', 'handoff'],
    });
    const button = screen.getByRole('button', { name: 'Open in Gosling' });
    await userEvent.click(button);
    await waitFor(() => {
      const call = fake.calls.find((entry) => entry.operation === 'handoff.prepare');
      expect(call?.request).toMatchObject({ sessionId: 'sess-1' });
    });
  });

  it('refuses handoff without a session instead of sending an empty sessionId', async () => {
    const { fake, store } = await mount({ session: null });
    await act(async () => {
      await store.actions.prepareHandoff('q', 'credential.relink');
    });
    expect(fake.calls.find((entry) => entry.operation === 'handoff.prepare')).toBeUndefined();
    expect(store.getState().failure).toMatchObject({
      code: 'CAPABILITY_UNAVAILABLE',
      recovery: 'none',
    });
  });

  it('keeps a failure banner visible while a background refresh runs', async () => {
    const { fake, store } = await mount({ session: null });
    fake.failNext('session.list', {
      code: 'RUNTIME_UNAVAILABLE',
      message: 'The shell backend is not currently available.',
      retrySafe: true,
      recovery: 'retry',
      preservesDraft: false,
    });
    await act(async () => {
      await store.actions.listSessions();
    });
    expect(store.getState().failure?.code).toBe('RUNTIME_UNAVAILABLE');
    await act(async () => {
      await store.actions.refreshRuntime();
    });
    expect(store.getState().failure?.code).toBe('RUNTIME_UNAVAILABLE');
  });

  it('clears the failure once an operation succeeds', async () => {
    const { fake, store } = await mount({ session: null });
    fake.failNext('session.list', {
      code: 'RUNTIME_UNAVAILABLE',
      message: 'x',
      retrySafe: true,
      recovery: 'retry',
      preservesDraft: false,
    });
    await act(async () => {
      await store.actions.listSessions();
    });
    expect(store.getState().failure).not.toBeNull();
    await act(async () => {
      await store.actions.listSessions();
    });
    expect(store.getState().failure).toBeNull();
  });

  it('refreshes the session list after detaching', async () => {
    const { fake, store } = await mount();
    fake.calls.length = 0;
    await act(async () => {
      await store.actions.detachSession();
    });
    expect(fake.calls.map((call) => call.operation)).toEqual(['session.detach', 'session.list']);
  });

  it('renders the usage meter with a real width', async () => {
    const { fake } = await mount();
    act(() =>
      fake.emitSessionUpdate(
        update({ updateSeq: 4, stream: { type: 'usage', used: 25, size: 100 } })
      )
    );
    const fill = document.querySelector<HTMLElement>('.gsh-usage__fill');
    expect(fill?.style.width).toBe('25%');
    expect(screen.getByText(/Context 25%/)).toBeInTheDocument();
  });
});
