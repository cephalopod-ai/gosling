import type {
  ShellDiagnosticsSaveRequest,
  ShellDiagnosticsSaveResult,
  ShellGenerationRequest,
  ShellActionResult,
  ShellHandoffConfirmRequest,
  ShellHandoffPrepareRequest,
  ShellHandoffPrepareResult,
  ShellOpenResult,
} from './ipc';
import type { ShellLifecycleState } from './lifecycle';

export interface GoslingShellAPI {
  runtime: {
    read(): Promise<ShellLifecycleState>;
    retry(request: ShellGenerationRequest): Promise<ShellActionResult>;
    stop(request: ShellGenerationRequest): Promise<ShellActionResult>;
    onChanged(listener: (state: ShellLifecycleState) => void): () => void;
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
