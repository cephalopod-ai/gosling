import type { ShellModuleSummary } from '@repo-makeover/gosling-sdk';
import type { ShellProvisioningIssueSummary } from './acpRuntime';
import type { ShellCredentialSnapshot } from './credentialController';
import type { ShellDirectorySnapshot } from './directoryController';
import type { ShellSettingsRecovery } from './localSettings';
import type { ShellInteraction } from './interactionController';
import type { ShellLifecycleState, ShellLifecycleStateName } from './lifecycle';
import type { ShellSessionRecord } from './sessionController';

export interface ShellRuntimeSnapshot extends ShellLifecycleState {
  lifecycleState: ShellLifecycleStateName;
  identity: { id: string; displayName: string; version: string } | null;
  runtimeNamespace: string | null;
  compatibility: { status: 'unverified' | 'compatible' | 'incompatible' };
  provisioningIssues: ShellProvisioningIssueSummary[];
  directory: ShellDirectorySnapshot;
  settingsRecovery: ShellSettingsRecovery;
  credentials: ShellCredentialSnapshot;
  modules: ShellModuleSummary[];
  session: ShellSessionRecord | null;
  adapter: {
    descriptorId: string;
    protocolVersion: string;
    actions: string[];
    status: 'ready' | 'crashed' | 'hung' | 'incompatible';
  } | null;
  pendingInteractions: ShellInteraction[];
}
