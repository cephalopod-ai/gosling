import type {
  ShellHandoffEnvelope,
  ShellHandoffPrepareRequest_unstable,
} from '@repo-makeover/gosling-sdk';
import type { ShellLifecycleState, ShellLifecycleStateName } from './lifecycle';

export const shellIpcChannels = {
  runtimeRead: 'runtime.read',
  runtimeRetry: 'runtime.retry',
  runtimeStop: 'runtime.stop',
  diagnosticsSave: 'diagnostics.save',
  handoffPrepare: 'handoff.prepare',
  handoffConfirm: 'handoff.confirm',
  externalOpen: 'external.open',
  runtimeChanged: 'runtime.changed',
} as const;

export type ShellIpcInvokeChannel =
  | (typeof shellIpcChannels)['runtimeRead']
  | (typeof shellIpcChannels)['runtimeRetry']
  | (typeof shellIpcChannels)['runtimeStop']
  | (typeof shellIpcChannels)['diagnosticsSave']
  | (typeof shellIpcChannels)['handoffPrepare']
  | (typeof shellIpcChannels)['handoffConfirm']
  | (typeof shellIpcChannels)['externalOpen'];

export type ShellIpcEventChannel = (typeof shellIpcChannels)['runtimeChanged'];

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
  [shellIpcChannels.diagnosticsSave]: ShellDiagnosticsSaveRequest;
  [shellIpcChannels.handoffPrepare]: ShellHandoffPrepareRequest;
  [shellIpcChannels.handoffConfirm]: ShellHandoffConfirmRequest;
  [shellIpcChannels.externalOpen]: string;
}

export interface ShellIpcResponseMap {
  [shellIpcChannels.runtimeRead]: ShellLifecycleState;
  [shellIpcChannels.runtimeRetry]: ShellActionResult;
  [shellIpcChannels.runtimeStop]: ShellActionResult;
  [shellIpcChannels.diagnosticsSave]: ShellDiagnosticsSaveResult;
  [shellIpcChannels.handoffPrepare]: ShellHandoffPrepareResult;
  [shellIpcChannels.handoffConfirm]: ShellOpenResult;
  [shellIpcChannels.externalOpen]: ShellOpenResult;
}

export interface ShellIpcEventMap {
  [shellIpcChannels.runtimeChanged]: ShellLifecycleState;
}
