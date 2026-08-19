import type Electron from 'electron';
import {
  shellIpcChannels,
  type ShellActionResult,
  type ShellDiagnosticsSaveRequest,
  type ShellDiagnosticsSaveResult,
  type ShellDomainActionConfirmRequest,
  type ShellDomainActionRequest,
  type ShellDomainSnapshotRequest,
  type ShellCredentialSelectRequest,
  type ShellDirectorySelectRequest,
  type ShellElicitationRespondRequest,
  type ShellSessionDetachResult,
  type ShellGenerationRequest,
  type ShellHandoffConfirmRequest,
  type ShellHandoffPrepareRequest,
  type ShellHandoffPrepareResult,
  type ShellIpcEventChannel,
  type ShellIpcInvokeChannel,
  type ShellOpenResult,
  type ShellPermissionRespondRequest,
  type ShellPromptCancelRequest,
  type ShellPromptSubmitRequest,
  type ShellPromptSubmitResult,
  type ShellSessionResumeRequest,
  type ShellSettingsAppearanceUpdateRequest,
  type ShellSettingsResetRequest,
  type ShellSettingsSnapshot,
} from './ipc';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';
import type { ShellArtifactListResponse_unstable } from '@repo-makeover/gosling-sdk';
import type { ShellSessionSummary } from './acpRuntime';
import type {
  ShellSessionRecord,
  ShellSessionUpdate,
  ShellTranscriptSnapshot,
} from './sessionController';
import type { ShellInteraction } from './interactionController';
import type { ShellCredentialSnapshot } from './credentialController';
import type { ShellDirectorySelectResult } from './directoryController';
import { isValidShellTheme, isValidShellTextScale } from './localSettings';
import { classifyShellOperationFailure, encodeShellOperationFailure } from './operationFailure';

const MAX_INVOKE_BYTES = 64 * 1024;
const MAX_RESPONSE_BYTES = 64 * 1024;
const MAX_SMALL_RESPONSE_BYTES = 8 * 1024;
const MAX_EXTERNAL_URL_BYTES = 2 * 1024;
const MAX_REFERENCES = 128;
const CAPABILITY_BY_CHANNEL: Partial<Record<ShellIpcInvokeChannel, string>> = {
  [shellIpcChannels.directorySelect]: 'directory.select',
  [shellIpcChannels.credentialSelect]: 'credential.select',
  [shellIpcChannels.sessionCreate]: 'session.create',
  [shellIpcChannels.sessionList]: 'session.list',
  [shellIpcChannels.sessionResume]: 'session.resume',
  [shellIpcChannels.sessionTranscriptRead]: 'session.transcript.read',
  [shellIpcChannels.sessionArtifactsRead]: 'session.artifacts.read',
  [shellIpcChannels.sessionDetach]: 'session.detach',
  [shellIpcChannels.promptSubmit]: 'prompt.submit',
  [shellIpcChannels.promptCancel]: 'prompt.cancel',
  [shellIpcChannels.permissionRespond]: 'permission.respond',
  [shellIpcChannels.elicitationRespond]: 'elicitation.respond',
  [shellIpcChannels.domainSnapshot]: 'domain.snapshot',
  [shellIpcChannels.domainAction]: 'domain.action',
  [shellIpcChannels.confirmationRespond]: 'confirmation.respond',
};

type ShellIpcMainEvent = Pick<Electron.IpcMainInvokeEvent, 'sender' | 'senderFrame'>;
interface ShellWebContents {
  id: number;
  mainFrame: unknown;
  isDestroyed(): boolean;
  send(channel: string, ...args: unknown[]): void;
}

export interface ShellIpcMainAdapter {
  handle(
    channel: string,
    listener: (event: ShellIpcMainEvent, request?: unknown) => Promise<unknown> | unknown
  ): void;
  removeHandler(channel: string): void;
}

export interface ShellIpcOperations {
  runtimeRead(): Promise<ShellRuntimeSnapshot> | ShellRuntimeSnapshot;
  runtimeRetry(request: ShellGenerationRequest): Promise<ShellActionResult> | ShellActionResult;
  runtimeStop(request: ShellGenerationRequest): Promise<ShellActionResult> | ShellActionResult;
  directorySelect(
    request: ShellDirectorySelectRequest
  ): Promise<ShellDirectorySelectResult> | ShellDirectorySelectResult;
  credentialSelect(
    request: ShellCredentialSelectRequest
  ): Promise<ShellCredentialSnapshot> | ShellCredentialSnapshot;
  sessionCreate(request: ShellGenerationRequest): Promise<ShellSessionRecord> | ShellSessionRecord;
  sessionList(
    request: ShellGenerationRequest
  ): Promise<ShellSessionSummary[]> | ShellSessionSummary[];
  sessionResume(
    request: ShellSessionResumeRequest
  ): Promise<ShellSessionRecord> | ShellSessionRecord;
  sessionTranscriptRead(
    request: ShellSessionResumeRequest
  ): Promise<ShellTranscriptSnapshot> | ShellTranscriptSnapshot;
  sessionArtifactsRead(
    request: ShellSessionResumeRequest
  ): Promise<ShellArtifactListResponse_unstable> | ShellArtifactListResponse_unstable;
  sessionDetach(
    request: ShellGenerationRequest
  ): Promise<ShellSessionDetachResult> | ShellSessionDetachResult;
  promptSubmit(
    request: ShellPromptSubmitRequest
  ): Promise<ShellPromptSubmitResult> | ShellPromptSubmitResult;
  promptCancel(request: ShellPromptCancelRequest): Promise<void> | void;
  permissionRespond(request: ShellPermissionRespondRequest): Promise<void> | void;
  elicitationRespond(request: ShellElicitationRespondRequest): Promise<void> | void;
  domainSnapshot(request: ShellDomainSnapshotRequest): Promise<unknown> | unknown;
  domainAction(request: ShellDomainActionRequest): Promise<unknown> | unknown;
  confirmationRespond(request: ShellDomainActionConfirmRequest): Promise<unknown> | unknown;
  diagnosticsSave(
    request: ShellDiagnosticsSaveRequest
  ): Promise<ShellDiagnosticsSaveResult> | ShellDiagnosticsSaveResult;
  handoffPrepare(
    request: ShellHandoffPrepareRequest
  ): Promise<ShellHandoffPrepareResult> | ShellHandoffPrepareResult;
  handoffConfirm(request: ShellHandoffConfirmRequest): Promise<ShellOpenResult> | ShellOpenResult;
  externalOpen(url: string): Promise<ShellOpenResult> | ShellOpenResult;
  settingsRead(): Promise<ShellSettingsSnapshot> | ShellSettingsSnapshot;
  settingsAppearanceUpdate(
    request: ShellSettingsAppearanceUpdateRequest
  ): Promise<ShellSettingsSnapshot> | ShellSettingsSnapshot;
  settingsReset(
    request: ShellSettingsResetRequest
  ): Promise<ShellSettingsSnapshot> | ShellSettingsSnapshot;
}

export interface RegisteredShellIpc {
  publishRuntimeChanged(state: ShellRuntimeSnapshot): boolean;
  publishSessionUpdated(update: ShellSessionUpdate): boolean;
  publishInteractionRequested(interaction: ShellInteraction): boolean;
  dispose(): void;
}

function assertObject(value: unknown, field: string): asserts value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${field} must be an object`);
  }
}

function assertExactKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
  field: string
): void {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${field} contains unsupported fields`);
  }
}

function assertString(value: unknown, field: string, maxLength: number): asserts value is string {
  if (typeof value !== 'string' || value.length === 0 || value.length > maxLength) {
    throw new Error(`${field} must be a non-empty bounded string`);
  }
}

function assertOptionalString(
  value: unknown,
  field: string,
  maxLength: number
): asserts value is string | null | undefined {
  if (value !== undefined && value !== null) {
    assertString(value, field, maxLength);
  }
}

function assertGeneration(value: unknown): asserts value is number {
  if (!Number.isSafeInteger(value) || (value as number) < 1) {
    throw new Error('generation must be a positive integer');
  }
}

function requestBytes(value: unknown): number {
  if (value === undefined) {
    return 0;
  }
  try {
    return Buffer.byteLength(JSON.stringify(value), 'utf8');
  } catch {
    throw new Error('request must be serializable');
  }
}

function assertRequestBytes(value: unknown, limit: number): void {
  if (requestBytes(value) > limit) {
    throw new Error('request exceeds the channel size limit');
  }
}

function assertResponseBytes(value: unknown, limit: number): void {
  if (requestBytes(value) > limit) {
    throw new Error('response exceeds the channel size limit');
  }
}

function parseGenerationRequest(value: unknown): ShellGenerationRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation'], 'request');
  assertGeneration(value.generation);
  return { generation: value.generation };
}

function parseDiagnosticsSaveRequest(value: unknown): ShellDiagnosticsSaveRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'userGesture'], 'request');
  assertGeneration(value.generation);
  if (value.userGesture !== true) {
    throw new Error('diagnostics.save requires an explicit user gesture');
  }
  return { generation: value.generation, userGesture: true };
}

function parseDirectorySelectRequest(value: unknown): ShellDirectorySelectRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'userGesture'], 'request');
  assertGeneration(value.generation);
  if (value.userGesture !== true) {
    throw new Error('directory.select requires an explicit user gesture');
  }
  return { generation: value.generation, userGesture: true };
}

function parseCredentialSelectRequest(value: unknown): ShellCredentialSelectRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'profileId'], 'request');
  assertGeneration(value.generation);
  if (value.profileId !== null) {
    assertString(value.profileId, 'profileId', 256);
  }
  return { generation: value.generation, profileId: value.profileId as string | null };
}

function parseSessionResumeRequest(value: unknown): ShellSessionResumeRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'sessionId'], 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  return { generation: value.generation, sessionId: value.sessionId };
}

function parsePromptSubmitRequest(value: unknown): ShellPromptSubmitRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'sessionId', 'text'], 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', MAX_SMALL_RESPONSE_BYTES);
  assertString(value.text, 'text', MAX_INVOKE_BYTES);
  return { generation: value.generation, sessionId: value.sessionId, text: value.text };
}

function parsePromptCancelRequest(value: unknown): ShellPromptCancelRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'sessionId', 'promptAttemptId'], 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  assertString(value.promptAttemptId, 'promptAttemptId', 512);
  return {
    generation: value.generation,
    sessionId: value.sessionId,
    promptAttemptId: value.promptAttemptId,
  };
}

function parsePermissionRespondRequest(value: unknown): ShellPermissionRespondRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'sessionId', 'actionId', 'allowOnce'], 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  assertString(value.actionId, 'actionId', 512);
  if (typeof value.allowOnce !== 'boolean') throw new Error('allowOnce must be boolean');
  return {
    generation: value.generation,
    sessionId: value.sessionId,
    actionId: value.actionId,
    allowOnce: value.allowOnce,
  };
}

function parseElicitationRespondRequest(value: unknown): ShellElicitationRespondRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertGeneration(value.generation);
  assertString(value.actionId, 'actionId', 512);
  if (value.action !== 'submit' && value.action !== 'decline' && value.action !== 'cancel') {
    throw new Error('action must be submit, decline, or cancel');
  }
  const keys =
    value.action === 'submit'
      ? ['generation', 'sessionId', 'actionId', 'action', 'fields']
      : ['generation', 'sessionId', 'actionId', 'action'];
  assertExactKeys(value, keys, 'request');
  assertString(value.sessionId, 'sessionId', 512);
  if (value.action === 'submit') {
    assertObject(value.fields, 'fields');
  }
  return value as unknown as ShellElicitationRespondRequest;
}

function parseDomainSnapshotRequest(value: unknown): ShellDomainSnapshotRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  const keys = value.input === undefined ? ['generation'] : ['generation', 'input'];
  assertExactKeys(value, keys, 'request');
  assertGeneration(value.generation);
  return value as unknown as ShellDomainSnapshotRequest;
}

function parseDomainActionRequest(value: unknown): ShellDomainActionRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  const keys =
    value.input === undefined
      ? ['generation', 'sessionId', 'action']
      : ['generation', 'sessionId', 'action', 'input'];
  assertExactKeys(value, keys, 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  assertString(value.action, 'action', 512);
  return value as unknown as ShellDomainActionRequest;
}

function parseDomainActionConfirmRequest(value: unknown): ShellDomainActionConfirmRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'sessionId', 'actionId', 'approve'], 'request');
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  assertString(value.actionId, 'actionId', 512);
  if (typeof value.approve !== 'boolean') throw new Error('approve must be boolean');
  return value as unknown as ShellDomainActionConfirmRequest;
}

function parseHandoffPrepareRequest(value: unknown): ShellHandoffPrepareRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  const allowed = [
    'generation',
    'sessionId',
    'question',
    'requestedCapability',
    'references',
    'returnDestination',
    'allowMutation',
  ];
  if (Object.keys(value).some((key) => !allowed.includes(key))) {
    throw new Error('request contains unsupported fields');
  }
  for (const required of ['generation', 'sessionId', 'question', 'requestedCapability']) {
    if (!(required in value)) {
      throw new Error('request is missing a required field');
    }
  }
  assertGeneration(value.generation);
  assertString(value.sessionId, 'sessionId', 512);
  assertString(value.question, 'question', 16 * 1024);
  assertString(value.requestedCapability, 'requestedCapability', 512);
  assertOptionalString(value.returnDestination, 'returnDestination', 2 * 1024);
  if (value.allowMutation !== undefined && typeof value.allowMutation !== 'boolean') {
    throw new Error('allowMutation must be boolean');
  }
  if (value.references !== undefined) {
    if (!Array.isArray(value.references) || value.references.length > MAX_REFERENCES) {
      throw new Error('references must be a bounded array');
    }
    for (const reference of value.references) {
      assertObject(reference, 'reference');
      const referenceKeys = Object.keys(reference);
      if (
        !referenceKeys.includes('kind') ||
        !referenceKeys.includes('id') ||
        referenceKeys.some((key) => !['kind', 'id', 'uri'].includes(key))
      ) {
        throw new Error('reference contains unsupported fields');
      }
      assertString(reference.kind, 'reference.kind', 256);
      assertString(reference.id, 'reference.id', 512);
      assertOptionalString(reference.uri, 'reference.uri', 2 * 1024);
    }
  }
  return value as unknown as ShellHandoffPrepareRequest;
}

function parseHandoffConfirmRequest(value: unknown): ShellHandoffConfirmRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'handoffId'], 'request');
  assertGeneration(value.generation);
  assertString(value.handoffId, 'handoffId', 512);
  return { generation: value.generation, handoffId: value.handoffId };
}

function parseSettingsAppearanceUpdateRequest(
  value: unknown
): ShellSettingsAppearanceUpdateRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  const optionalKeys = ['theme', 'textScale'].filter((key) => key in value);
  assertExactKeys(value, ['generation', ...optionalKeys], 'request');
  assertGeneration(value.generation);
  if ('theme' in value && !isValidShellTheme(value.theme)) {
    throw new Error('theme must be system, light, or dark');
  }
  if ('textScale' in value && !isValidShellTextScale(value.textScale)) {
    throw new Error('textScale must be a number between 0.8 and 2');
  }
  return value as unknown as ShellSettingsAppearanceUpdateRequest;
}

function parseSettingsResetRequest(value: unknown): ShellSettingsResetRequest {
  assertRequestBytes(value, MAX_INVOKE_BYTES);
  assertObject(value, 'request');
  assertExactKeys(value, ['generation', 'userGesture'], 'request');
  assertGeneration(value.generation);
  if (value.userGesture !== true) {
    throw new Error('settings.reset requires an explicit user gesture');
  }
  return { generation: value.generation, userGesture: true };
}

function parseExternalUrl(value: unknown, allowedOrigins: ReadonlySet<string>): string {
  assertString(value, 'url', MAX_EXTERNAL_URL_BYTES);
  assertRequestBytes(value, MAX_EXTERNAL_URL_BYTES);
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error('url must be an absolute URL');
  }
  if (!['https:', 'http:'].includes(parsed.protocol) || !allowedOrigins.has(parsed.origin)) {
    throw new Error('url origin is not allowlisted');
  }
  if (parsed.username || parsed.password) {
    throw new Error('url credentials are not allowed');
  }
  return parsed.href;
}

function assertTrustedSender(event: ShellIpcMainEvent, trusted: ShellWebContents): void {
  if (event.sender.id !== trusted.id || event.senderFrame !== trusted.mainFrame) {
    throw new Error('untrusted shell renderer');
  }
}

function responseLimit(channel: ShellIpcInvokeChannel): number {
  return channel === shellIpcChannels.runtimeRead ||
    channel === shellIpcChannels.credentialSelect ||
    channel === shellIpcChannels.sessionList ||
    channel === shellIpcChannels.sessionTranscriptRead ||
    channel === shellIpcChannels.sessionArtifactsRead ||
    channel === shellIpcChannels.handoffPrepare ||
    channel === shellIpcChannels.domainSnapshot ||
    channel === shellIpcChannels.domainAction ||
    channel === shellIpcChannels.confirmationRespond
    ? MAX_RESPONSE_BYTES
    : MAX_SMALL_RESPONSE_BYTES;
}

export function registerShellIpc(input: {
  ipcMain: ShellIpcMainAdapter;
  renderer: ShellWebContents;
  operations: ShellIpcOperations;
  allowedExternalOrigins: ReadonlySet<string>;
  declaredCapabilities?: ReadonlySet<string>;
}): RegisteredShellIpc {
  const { ipcMain, renderer, operations, allowedExternalOrigins, declaredCapabilities } = input;
  let lastPublishedGeneration = 0;
  const publish = (channel: ShellIpcEventChannel, value: unknown): boolean => {
    if (renderer.isDestroyed()) return false;
    try {
      renderer.send(channel, value);
      return true;
    } catch {
      return false;
    }
  };
  const registrations: Array<
    [ShellIpcInvokeChannel, (request: unknown) => Promise<unknown> | unknown]
  > = [
    [shellIpcChannels.runtimeRead, () => operations.runtimeRead()],
    [
      shellIpcChannels.runtimeRetry,
      (request) => operations.runtimeRetry(parseGenerationRequest(request)),
    ],
    [
      shellIpcChannels.runtimeStop,
      (request) => operations.runtimeStop(parseGenerationRequest(request)),
    ],
    [
      shellIpcChannels.directorySelect,
      (request) => operations.directorySelect(parseDirectorySelectRequest(request)),
    ],
    [
      shellIpcChannels.credentialSelect,
      (request) => operations.credentialSelect(parseCredentialSelectRequest(request)),
    ],
    [
      shellIpcChannels.sessionCreate,
      (request) => operations.sessionCreate(parseGenerationRequest(request)),
    ],
    [
      shellIpcChannels.sessionList,
      (request) => operations.sessionList(parseGenerationRequest(request)),
    ],
    [
      shellIpcChannels.sessionResume,
      (request) => operations.sessionResume(parseSessionResumeRequest(request)),
    ],
    [
      shellIpcChannels.sessionTranscriptRead,
      (request) => operations.sessionTranscriptRead(parseSessionResumeRequest(request)),
    ],
    [
      shellIpcChannels.sessionArtifactsRead,
      (request) => operations.sessionArtifactsRead(parseSessionResumeRequest(request)),
    ],
    [
      shellIpcChannels.sessionDetach,
      (request) => operations.sessionDetach(parseGenerationRequest(request)),
    ],
    [
      shellIpcChannels.promptSubmit,
      (request) => operations.promptSubmit(parsePromptSubmitRequest(request)),
    ],
    [
      shellIpcChannels.promptCancel,
      (request) => operations.promptCancel(parsePromptCancelRequest(request)),
    ],
    [
      shellIpcChannels.permissionRespond,
      (request) => operations.permissionRespond(parsePermissionRespondRequest(request)),
    ],
    [
      shellIpcChannels.elicitationRespond,
      (request) => operations.elicitationRespond(parseElicitationRespondRequest(request)),
    ],
    [
      shellIpcChannels.domainSnapshot,
      (request) => operations.domainSnapshot(parseDomainSnapshotRequest(request)),
    ],
    [
      shellIpcChannels.domainAction,
      (request) => operations.domainAction(parseDomainActionRequest(request)),
    ],
    [
      shellIpcChannels.confirmationRespond,
      (request) => operations.confirmationRespond(parseDomainActionConfirmRequest(request)),
    ],
    [
      shellIpcChannels.diagnosticsSave,
      (request) => operations.diagnosticsSave(parseDiagnosticsSaveRequest(request)),
    ],
    [
      shellIpcChannels.handoffPrepare,
      (request) => operations.handoffPrepare(parseHandoffPrepareRequest(request)),
    ],
    [
      shellIpcChannels.handoffConfirm,
      (request) => operations.handoffConfirm(parseHandoffConfirmRequest(request)),
    ],
    [
      shellIpcChannels.externalOpen,
      (request) => operations.externalOpen(parseExternalUrl(request, allowedExternalOrigins)),
    ],
    [shellIpcChannels.settingsRead, () => operations.settingsRead()],
    [
      shellIpcChannels.settingsAppearanceUpdate,
      (request) =>
        operations.settingsAppearanceUpdate(parseSettingsAppearanceUpdateRequest(request)),
    ],
    [
      shellIpcChannels.settingsReset,
      (request) => operations.settingsReset(parseSettingsResetRequest(request)),
    ],
  ];

  for (const [channel, operation] of registrations) {
    ipcMain.handle(channel, async (event, request) => {
      assertTrustedSender(event, renderer);
      try {
        const capability = CAPABILITY_BY_CHANNEL[channel];
        if (capability && declaredCapabilities && !declaredCapabilities.has(capability)) {
          throw new Error(`shell consumer did not declare ${capability}`);
        }
        const isParameterlessReadChannel =
          channel === shellIpcChannels.runtimeRead || channel === shellIpcChannels.settingsRead;
        if (isParameterlessReadChannel && request !== undefined) {
          throw new Error(`${channel} does not accept a request`);
        }
        const response = await operation(request);
        assertResponseBytes(response, responseLimit(channel));
        return response;
      } catch (error) {
        throw new Error(encodeShellOperationFailure(classifyShellOperationFailure(channel, error)));
      }
    });
  }

  return {
    publishRuntimeChanged(state) {
      if (state.generation < lastPublishedGeneration) {
        return false;
      }
      assertResponseBytes(state, MAX_RESPONSE_BYTES);
      if (!publish(shellIpcChannels.runtimeChanged, state)) return false;
      lastPublishedGeneration = state.generation;
      return true;
    },
    publishSessionUpdated(update) {
      assertResponseBytes(update, MAX_RESPONSE_BYTES);
      return publish(shellIpcChannels.sessionUpdated, update);
    },
    publishInteractionRequested(interaction) {
      assertResponseBytes(interaction, MAX_SMALL_RESPONSE_BYTES);
      const channel =
        interaction.kind === 'permission'
          ? shellIpcChannels.permissionRequested
          : interaction.kind === 'elicitation'
            ? shellIpcChannels.elicitationRequested
            : shellIpcChannels.confirmationRequested;
      return publish(channel, interaction);
    },
    dispose() {
      for (const [channel] of registrations) {
        ipcMain.removeHandler(channel);
      }
    },
  };
}
