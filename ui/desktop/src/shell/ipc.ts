import type {
  DomainActionConfirmResponse_unstable,
  DomainActionResponse_unstable,
  DomainSnapshotResponse_unstable,
  ShellHandoffEnvelope,
  ShellHandoffPrepareRequest_unstable,
} from '@repo-makeover/gosling-sdk';
import type { ShellLifecycleStateName } from './lifecycle';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';
import type { ShellSessionRecord, ShellSessionUpdate } from './sessionController';
import type { ShellInteraction } from './interactionController';

export const shellIpcChannels = {
  runtimeRead: 'runtime.read',
  runtimeRetry: 'runtime.retry',
  runtimeStop: 'runtime.stop',
  sessionCreate: 'session.create',
  sessionResume: 'session.resume',
  promptSubmit: 'prompt.submit',
  promptCancel: 'prompt.cancel',
  permissionRespond: 'permission.respond',
  elicitationRespond: 'elicitation.respond',
  domainSnapshot: 'domain.snapshot',
  domainAction: 'domain.action',
  confirmationRespond: 'confirmation.respond',
  diagnosticsSave: 'diagnostics.save',
  handoffPrepare: 'handoff.prepare',
  handoffConfirm: 'handoff.confirm',
  externalOpen: 'external.open',
  runtimeChanged: 'runtime.changed',
  sessionUpdated: 'session.updated',
  permissionRequested: 'permission.requested',
  elicitationRequested: 'elicitation.requested',
} as const;

export type ShellIpcInvokeChannel =
  | (typeof shellIpcChannels)['runtimeRead']
  | (typeof shellIpcChannels)['runtimeRetry']
  | (typeof shellIpcChannels)['runtimeStop']
  | (typeof shellIpcChannels)['sessionCreate']
  | (typeof shellIpcChannels)['sessionResume']
  | (typeof shellIpcChannels)['promptSubmit']
  | (typeof shellIpcChannels)['promptCancel']
  | (typeof shellIpcChannels)['permissionRespond']
  | (typeof shellIpcChannels)['elicitationRespond']
  | (typeof shellIpcChannels)['domainSnapshot']
  | (typeof shellIpcChannels)['domainAction']
  | (typeof shellIpcChannels)['confirmationRespond']
  | (typeof shellIpcChannels)['diagnosticsSave']
  | (typeof shellIpcChannels)['handoffPrepare']
  | (typeof shellIpcChannels)['handoffConfirm']
  | (typeof shellIpcChannels)['externalOpen'];

export type ShellIpcEventChannel =
  | (typeof shellIpcChannels)['runtimeChanged']
  | (typeof shellIpcChannels)['sessionUpdated']
  | (typeof shellIpcChannels)['permissionRequested']
  | (typeof shellIpcChannels)['elicitationRequested'];

export interface ShellGenerationRequest {
  generation: number;
}

export interface ShellActionResult {
  accepted: boolean;
  generation: number;
  state: ShellLifecycleStateName;
}

export interface ShellDiagnosticsSaveRequest extends ShellGenerationRequest {
  userGesture: true;
}

export interface ShellSessionResumeRequest extends ShellGenerationRequest {
  sessionId: string;
}

export interface ShellPromptSubmitRequest extends ShellSessionResumeRequest {
  text: string;
}

export interface ShellPromptCancelRequest extends ShellSessionResumeRequest {
  promptAttemptId: string;
}

export interface ShellPermissionRespondRequest extends ShellSessionResumeRequest {
  actionId: string;
  allowOnce: boolean;
}

export interface ShellElicitationRespondRequest extends ShellSessionResumeRequest {
  actionId: string;
  action: 'submit' | 'cancel';
  fields?: Record<string, unknown>;
}

export interface ShellDomainSnapshotRequest extends ShellGenerationRequest {
  input?: unknown;
}

export interface ShellDomainActionRequest extends ShellSessionResumeRequest {
  action: string;
  input?: unknown;
}

export interface ShellDomainActionConfirmRequest extends ShellSessionResumeRequest {
  actionId: string;
  approve: boolean;
}

export interface ShellPromptSubmitResult {
  promptAttemptId: string;
}

export type ShellDiagnosticsSaveResult =
  | { status: 'canceled' }
  | { status: 'saved'; fileName: string };

export interface ShellHandoffPrepareRequest extends ShellHandoffPrepareRequest_unstable {
  generation: number;
}

export interface ShellHandoffPrepareResult {
  generation: number;
  handoff: ShellHandoffEnvelope;
}

export interface ShellHandoffConfirmRequest extends ShellGenerationRequest {
  handoffId: string;
}

export interface ShellOpenResult {
  opened: boolean;
}

export interface ShellIpcRequestMap {
  [shellIpcChannels.runtimeRead]: undefined;
  [shellIpcChannels.runtimeRetry]: ShellGenerationRequest;
  [shellIpcChannels.runtimeStop]: ShellGenerationRequest;
  [shellIpcChannels.sessionCreate]: ShellGenerationRequest;
  [shellIpcChannels.sessionResume]: ShellSessionResumeRequest;
  [shellIpcChannels.promptSubmit]: ShellPromptSubmitRequest;
  [shellIpcChannels.promptCancel]: ShellPromptCancelRequest;
  [shellIpcChannels.permissionRespond]: ShellPermissionRespondRequest;
  [shellIpcChannels.elicitationRespond]: ShellElicitationRespondRequest;
  [shellIpcChannels.domainSnapshot]: ShellDomainSnapshotRequest;
  [shellIpcChannels.domainAction]: ShellDomainActionRequest;
  [shellIpcChannels.confirmationRespond]: ShellDomainActionConfirmRequest;
  [shellIpcChannels.diagnosticsSave]: ShellDiagnosticsSaveRequest;
  [shellIpcChannels.handoffPrepare]: ShellHandoffPrepareRequest;
  [shellIpcChannels.handoffConfirm]: ShellHandoffConfirmRequest;
  [shellIpcChannels.externalOpen]: string;
}

export interface ShellIpcResponseMap {
  [shellIpcChannels.runtimeRead]: ShellRuntimeSnapshot;
  [shellIpcChannels.runtimeRetry]: ShellActionResult;
  [shellIpcChannels.runtimeStop]: ShellActionResult;
  [shellIpcChannels.sessionCreate]: ShellSessionRecord;
  [shellIpcChannels.sessionResume]: ShellSessionRecord;
  [shellIpcChannels.promptSubmit]: ShellPromptSubmitResult;
  [shellIpcChannels.promptCancel]: undefined;
  [shellIpcChannels.permissionRespond]: undefined;
  [shellIpcChannels.elicitationRespond]: undefined;
  [shellIpcChannels.domainSnapshot]: DomainSnapshotResponse_unstable;
  [shellIpcChannels.domainAction]: DomainActionResponse_unstable;
  [shellIpcChannels.confirmationRespond]: DomainActionConfirmResponse_unstable;
  [shellIpcChannels.diagnosticsSave]: ShellDiagnosticsSaveResult;
  [shellIpcChannels.handoffPrepare]: ShellHandoffPrepareResult;
  [shellIpcChannels.handoffConfirm]: ShellOpenResult;
  [shellIpcChannels.externalOpen]: ShellOpenResult;
}

export interface ShellIpcEventMap {
  [shellIpcChannels.runtimeChanged]: ShellRuntimeSnapshot;
  [shellIpcChannels.sessionUpdated]: ShellSessionUpdate;
  [shellIpcChannels.permissionRequested]: Extract<ShellInteraction, { kind: 'permission' }>;
  [shellIpcChannels.elicitationRequested]: Extract<ShellInteraction, { kind: 'elicitation' }>;
}
