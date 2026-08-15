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
  ShellCredentialSelectRequest,
  ShellDirectorySelectRequest,
  ShellSessionDetachResult,
  ShellSessionResumeRequest,
  ShellSettingsAppearanceUpdateRequest,
  ShellSettingsResetRequest,
  ShellSettingsSnapshot,
} from './ipc';
import type { ShellCredentialSnapshot } from './credentialController';
import type { ShellDirectorySelectResult } from './directoryController';
import type { ShellRuntimeSnapshot } from './runtimeSnapshot';
import type { ShellSessionSummary } from './acpRuntime';
import type {
  ShellSessionRecord,
  ShellSessionUpdate,
  ShellTranscriptSnapshot,
} from './sessionController';
import type { ShellInteraction } from './interactionController';
import type {
  DomainActionConfirmResponse_unstable,
  DomainActionResponse_unstable,
  DomainSnapshotResponse_unstable,
} from '@repo-makeover/gosling-sdk';

export type { ShellOperationFailure, ShellRecoveryAction } from './operationFailure';

export interface GoslingShellAPI {
  runtime: {
    read(): Promise<ShellRuntimeSnapshot>;
    retry(request: ShellGenerationRequest): Promise<ShellActionResult>;
    stop(request: ShellGenerationRequest): Promise<ShellActionResult>;
    onChanged(listener: (state: ShellRuntimeSnapshot) => void): () => void;
  };
  directory: {
    select(request: ShellDirectorySelectRequest): Promise<ShellDirectorySelectResult>;
  };
  credential: {
    select(request: ShellCredentialSelectRequest): Promise<ShellCredentialSnapshot>;
  };
  session: {
    create(request: ShellGenerationRequest): Promise<ShellSessionRecord>;
    list(request: ShellGenerationRequest): Promise<ShellSessionSummary[]>;
    resume(request: ShellSessionResumeRequest): Promise<ShellSessionRecord>;
    readTranscript(request: ShellSessionResumeRequest): Promise<ShellTranscriptSnapshot>;
    detach(request: ShellGenerationRequest): Promise<ShellSessionDetachResult>;
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
    onConfirmationRequested(
      listener: (interaction: Extract<ShellInteraction, { kind: 'confirm' }>) => void
    ): () => void;
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
  settings: {
    read(): Promise<ShellSettingsSnapshot>;
    updateAppearance(request: ShellSettingsAppearanceUpdateRequest): Promise<ShellSettingsSnapshot>;
    reset(request: ShellSettingsResetRequest): Promise<ShellSettingsSnapshot>;
  };
}

declare global {
  interface Window {
    goslingShell: GoslingShellAPI;
  }
}
