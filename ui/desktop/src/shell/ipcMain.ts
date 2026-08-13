import type Electron from 'electron';
import {
  shellIpcChannels,
  type ShellActionResult,
  type ShellDiagnosticsSaveRequest,
  type ShellDiagnosticsSaveResult,
  type ShellGenerationRequest,
  type ShellHandoffConfirmRequest,
  type ShellHandoffPrepareRequest,
  type ShellHandoffPrepareResult,
  type ShellIpcInvokeChannel,
  type ShellOpenResult,
} from './ipc';
import type { ShellLifecycleState } from './lifecycle';

const MAX_INVOKE_BYTES = 64 * 1024;
const MAX_RESPONSE_BYTES = 64 * 1024;
const MAX_SMALL_RESPONSE_BYTES = 8 * 1024;
const MAX_EXTERNAL_URL_BYTES = 2 * 1024;
const MAX_REFERENCES = 128;

type ShellIpcMainEvent = Pick<Electron.IpcMainInvokeEvent, 'sender' | 'senderFrame'>;
interface ShellWebContents {
  id: number;
  mainFrame: unknown;
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
  runtimeRead(): Promise<ShellLifecycleState> | ShellLifecycleState;
  runtimeRetry(request: ShellGenerationRequest): Promise<ShellActionResult> | ShellActionResult;
  runtimeStop(request: ShellGenerationRequest): Promise<ShellActionResult> | ShellActionResult;
  diagnosticsSave(
    request: ShellDiagnosticsSaveRequest
  ): Promise<ShellDiagnosticsSaveResult> | ShellDiagnosticsSaveResult;
  handoffPrepare(
    request: ShellHandoffPrepareRequest
  ): Promise<ShellHandoffPrepareResult> | ShellHandoffPrepareResult;
  handoffConfirm(request: ShellHandoffConfirmRequest): Promise<ShellOpenResult> | ShellOpenResult;
  externalOpen(url: string): Promise<ShellOpenResult> | ShellOpenResult;
}

export interface RegisteredShellIpc {
  publishRuntimeChanged(state: ShellLifecycleState): boolean;
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
  return channel === shellIpcChannels.runtimeRead || channel === shellIpcChannels.handoffPrepare
    ? MAX_RESPONSE_BYTES
    : MAX_SMALL_RESPONSE_BYTES;
}

export function registerShellIpc(input: {
  ipcMain: ShellIpcMainAdapter;
  renderer: ShellWebContents;
  operations: ShellIpcOperations;
  allowedExternalOrigins: ReadonlySet<string>;
}): RegisteredShellIpc {
  const { ipcMain, renderer, operations, allowedExternalOrigins } = input;
  let lastPublishedGeneration = 0;
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
  ];

  for (const [channel, operation] of registrations) {
    ipcMain.handle(channel, async (event, request) => {
      assertTrustedSender(event, renderer);
      if (channel === shellIpcChannels.runtimeRead && request !== undefined) {
        throw new Error('runtime.read does not accept a request');
      }
      const response = await operation(request);
      assertResponseBytes(response, responseLimit(channel));
      return response;
    });
  }

  return {
    publishRuntimeChanged(state) {
      if (state.generation < lastPublishedGeneration) {
        return false;
      }
      assertResponseBytes(state, MAX_RESPONSE_BYTES);
      lastPublishedGeneration = state.generation;
      renderer.send(shellIpcChannels.runtimeChanged, state);
      return true;
    },
    dispose() {
      for (const [channel] of registrations) {
        ipcMain.removeHandler(channel);
      }
    },
  };
}
