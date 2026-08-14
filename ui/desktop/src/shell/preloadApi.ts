import type {
  ShellDiagnosticsSaveRequest,
  ShellDiagnosticsSaveResult,
  ShellDomainActionConfirmRequest,
  ShellDomainActionRequest,
  ShellDomainSnapshotRequest,
  ShellGenerationRequest,
  ShellActionResult,
  ShellHandoffConfirmRequest,
  ShellHandoffPrepareRequest,
  ShellHandoffPrepareResult,
  ShellOpenResult,
  ShellPromptCancelRequest,
  ShellPromptSubmitRequest,
  ShellPromptSubmitResult,
  ShellPermissionRespondRequest,
  ShellElicitationRespondRequest,
  ShellSessionResumeRequest,
} from './ipc';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';
import type { ShellSessionRecord, ShellSessionUpdate } from './sessionController';
import type { ShellInteraction } from './interactionController';
import type {
  DomainActionConfirmResponse_unstable,
  DomainActionResponse_unstable,
  DomainSnapshotResponse_unstable,
} from '@repo-makeover/gosling-sdk';

export interface GoslingShellAPI {
  runtime: {
    read(): Promise<ShellRuntimeSnapshot>;
    retry(request: ShellGenerationRequest): Promise<ShellActionResult>;
    stop(request: ShellGenerationRequest): Promise<ShellActionResult>;
    onChanged(listener: (state: ShellRuntimeSnapshot) => void): () => void;
  };
  session: {
    create(request: ShellGenerationRequest): Promise<ShellSessionRecord>;
    resume(request: ShellSessionResumeRequest): Promise<ShellSessionRecord>;
    onUpdated(listener: (update: ShellSessionUpdate) => void): () => void;
  };
  prompt: {
    submit(request: ShellPromptSubmitRequest): Promise<ShellPromptSubmitResult>;
    cancel(request: ShellPromptCancelRequest): Promise<void>;
  };
  permission: {
    respond(request: ShellPermissionRespondRequest): Promise<void>;
    onRequested(
      listener: (interaction: Extract<ShellInteraction, { kind: 'permission' }>) => void
    ): () => void;
  };
  elicitation: {
    respond(request: ShellElicitationRespondRequest): Promise<void>;
    onRequested(
      listener: (interaction: Extract<ShellInteraction, { kind: 'elicitation' }>) => void
    ): () => void;
  };
  domain: {
    snapshot(request: ShellDomainSnapshotRequest): Promise<DomainSnapshotResponse_unstable>;
    action(request: ShellDomainActionRequest): Promise<DomainActionResponse_unstable>;
    confirm(
      request: ShellDomainActionConfirmRequest
    ): Promise<DomainActionConfirmResponse_unstable>;
  };
  diagnostics: {
    save(request: ShellDiagnosticsSaveRequest): Promise<ShellDiagnosticsSaveResult>;
  };
  handoff: {
    prepare(request: ShellHandoffPrepareRequest): Promise<ShellHandoffPrepareResult>;
    confirm(request: ShellHandoffConfirmRequest): Promise<ShellOpenResult>;
  };
  external: {
    open(url: string): Promise<ShellOpenResult>;
  };
}

declare global {
  interface Window {
    goslingShell: GoslingShellAPI;
  }
}
