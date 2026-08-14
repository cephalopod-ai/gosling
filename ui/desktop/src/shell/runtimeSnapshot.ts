import type { ShellProvisioningIssueSummary } from './acpRuntime';
import type { ShellInteraction } from './interactionController';
import type { ShellLifecycleState, ShellLifecycleStateName } from './lifecycle';
import type { ShellSessionRecord } from './sessionController';

export interface ShellRuntimeSnapshot extends ShellLifecycleState {
  lifecycleState: ShellLifecycleStateName;
  identity: { id: string; displayName: string; version: string } | null;
  runtimeNamespace: string | null;
  compatibility: { status: 'unverified' | 'compatible' | 'incompatible' };
  provisioningIssues: ShellProvisioningIssueSummary[];
  session: ShellSessionRecord | null;
  adapter: {
    descriptorId: string;
    protocolVersion: string;
    actions: string[];
    status: 'ready' | 'crashed' | 'hung' | 'incompatible';
  } | null;
  pendingInteractions: ShellInteraction[];
}
