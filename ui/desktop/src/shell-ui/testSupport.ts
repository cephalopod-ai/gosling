import type { ShellSettingsSnapshot } from '../shell/ipc';
import type { ShellInteraction } from '../shell/interactionController';
import type { ShellLifecycleStateName } from '../shell/lifecycle';
import type { ShellRuntimeSnapshot } from '../shell/runtimeSnapshot';
import type { ShellSessionRecord, ShellSessionUpdate } from '../shell/sessionController';

export const ALL_CAPABILITIES = [
  'credential.select',
  'directory.select',
  'elicitation.respond',
  'permission.respond',
  'prompt.cancel',
  'prompt.submit',
  'session.artifacts.read',
  'session.create',
  'session.detach',
  'session.list',
  'session.resume',
  'session.transcript.read',
] as const;

export function activeSession(overrides: Partial<ShellSessionRecord> = {}): ShellSessionRecord {
  return {
    sessionId: 'sess-1',
    status: 'active',
    resumeKind: 'fresh',
    resumeIntegrity: 'not_applicable',
    workingDir: '/work/project',
    title: 'Changelog cleanup',
    providerId: 'anthropic',
    modelId: 'model-x',
    promptAttempt: null,
    ...overrides,
  };
}

export function snapshot(overrides: Partial<ShellRuntimeSnapshot> = {}): ShellRuntimeSnapshot {
  const lifecycleState: ShellLifecycleStateName = overrides.lifecycleState ?? 'ready';
  return {
    generation: 1,
    name: lifecycleState,
    lifecycleState,
    enteredAt: '2026-08-18T00:00:00.000Z',
    allowedActions: ['stop', 'diagnostics', 'handoff'],
    identity: { id: 'template', displayName: 'Default Shell Template', version: '0.0.0' },
    runtimeNamespace: 'default-shell-template',
    declaredCapabilities: [...ALL_CAPABILITIES],
    compatibility: { status: 'compatible' },
    provisioningIssues: [],
    directory: {
      state: 'selected',
      path: '/work/project',
      label: 'project',
      reasonCode: null,
      remembered: true,
    },
    settingsRecovery: { status: 'loaded', schemaVersion: 1 },
    credentials: {
      catalogStatus: 'available',
      profiles: [
        {
          id: 'cred-1',
          name: 'Work Anthropic',
          providerOrServiceId: 'anthropic',
          status: 'configured',
        },
      ],
      selectedProfileId: 'cred-1',
      selectionStatus: 'configured',
    },
    modules: [{ id: 'core:session', kind: 'core', status: 'ready' }],
    session: activeSession(),
    adapter: null,
    pendingInteractions: [],
    ...overrides,
  };
}

export function settings(overrides: Partial<ShellSettingsSnapshot> = {}): ShellSettingsSnapshot {
  return {
    appearance: { theme: 'light', textScale: 1 },
    recovery: { status: 'loaded', schemaVersion: 1 },
    ...overrides,
  };
}

export function update(overrides: Partial<ShellSessionUpdate> = {}): ShellSessionUpdate {
  return {
    generation: 1,
    sessionId: 'sess-1',
    promptAttemptId: 'attempt-1',
    updateSeq: 1,
    kind: 'stream',
    delivery: 'live',
    stream: { type: 'content', role: 'assistant', messageId: 'm1', text: 'hello' },
    ...overrides,
  };
}

export function permissionInteraction(
  overrides: Partial<Extract<ShellInteraction, { kind: 'permission' }>> = {}
): Extract<ShellInteraction, { kind: 'permission' }> {
  return {
    actionId: 'action-1',
    generation: 1,
    expiresAtGeneration: 1,
    sessionId: 'sess-1',
    promptAttemptId: 'attempt-1',
    kind: 'permission',
    summary: {
      toolTitle: 'write_file',
      effect: 'write',
      targets: ['CHANGELOG.md'],
      inputFields: ['path', 'content'],
      allowOnce: true,
      deny: true,
    },
    ...overrides,
  };
}

export function elicitationInteraction(
  overrides: Partial<Extract<ShellInteraction, { kind: 'elicitation' }>> = {}
): Extract<ShellInteraction, { kind: 'elicitation' }> {
  return {
    actionId: 'action-2',
    generation: 1,
    expiresAtGeneration: 1,
    sessionId: 'sess-1',
    promptAttemptId: 'attempt-1',
    kind: 'elicitation',
    summary: {
      message: 'Which release should this go under?',
      title: null,
      description: null,
      toolCallId: 'tool-1',
      fields: [
        {
          name: 'version',
          label: 'Version',
          description: null,
          required: true,
          type: 'string',
        },
        {
          name: 'breaking',
          label: 'Include breaking changes',
          description: null,
          required: false,
          type: 'boolean',
        },
      ],
    },
    ...overrides,
  };
}

export function confirmInteraction(
  overrides: Partial<Extract<ShellInteraction, { kind: 'confirm' }>> = {}
): Extract<ShellInteraction, { kind: 'confirm' }> {
  return {
    actionId: 'action-3',
    generation: 1,
    expiresAtGeneration: 1,
    sessionId: 'sess-1',
    promptAttemptId: 'attempt-1',
    kind: 'confirm',
    summary: { action: 'toggle', inputFields: ['target'] },
    ...overrides,
  };
}
