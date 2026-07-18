import type {
  CredentialProfile,
  CredentialProfileCreateRequest_unstable,
  CredentialProfileUpdateRequest_unstable,
  Workspace,
  WorkspaceMutation,
  WorkspaceValidationReport,
  WorkspaceWithValidation,
} from '@repo-makeover/gosling-sdk';
import { getAcpClient } from './acpConnection';

export type {
  CredentialProfile,
  CredentialProfileCreateRequest_unstable,
  CredentialProfileUpdateRequest_unstable,
  ProductOutputFolder,
  ProductType,
  Workspace,
  WorkspaceFolder,
  WorkspaceMutation,
  WorkspaceValidationReport,
  WorkspaceWithValidation,
} from '@repo-makeover/gosling-sdk';

export async function acpListWorkspaces() {
  const client = await getAcpClient();
  return client.gosling.workspacesList_unstable({});
}

export async function acpCreateWorkspace(workspace: WorkspaceMutation) {
  const client = await getAcpClient();
  return client.gosling.workspacesCreate_unstable({ workspace });
}

export async function acpUpdateWorkspace(workspaceId: string, workspace: WorkspaceMutation) {
  const client = await getAcpClient();
  return client.gosling.workspacesUpdate_unstable({ workspaceId, workspace });
}

export async function acpDuplicateWorkspace(workspaceId: string) {
  const client = await getAcpClient();
  return client.gosling.workspacesDuplicate_unstable({ workspaceId });
}

export async function acpDeleteWorkspace(workspaceId: string) {
  const client = await getAcpClient();
  return client.gosling.workspacesDelete_unstable({ workspaceId });
}

export async function acpSetActiveWorkspace(workspaceId: string) {
  const client = await getAcpClient();
  return client.gosling.workspacesActiveSet_unstable({ workspaceId });
}

export async function acpValidateWorkspace(
  workspace: WorkspaceMutation,
  workspaceId?: string
): Promise<WorkspaceValidationReport> {
  const client = await getAcpClient();
  const response = await client.gosling.workspacesValidate_unstable({ workspace, workspaceId });
  return response.validation;
}

export async function acpExportWorkspace(workspaceId: string): Promise<string> {
  const client = await getAcpClient();
  const response = await client.gosling.workspacesExport_unstable({ workspaceId });
  return response.document;
}

export async function acpCreateWorkspaceOutput(workspaceId: string, outputFolderId: string) {
  const client = await getAcpClient();
  return client.gosling.workspacesOutputCreate_unstable({ workspaceId, outputFolderId });
}

export async function acpListCredentialProfiles(): Promise<CredentialProfile[]> {
  const client = await getAcpClient();
  const response = await client.gosling.credentialProfilesList_unstable({});
  return response.profiles;
}

export async function acpCreateCredentialProfile(
  request: CredentialProfileCreateRequest_unstable
): Promise<CredentialProfile> {
  const client = await getAcpClient();
  const response = await client.gosling.credentialProfilesCreate_unstable(request);
  return response.profile;
}

export async function acpUpdateCredentialProfile(
  request: CredentialProfileUpdateRequest_unstable
): Promise<CredentialProfile> {
  const client = await getAcpClient();
  const response = await client.gosling.credentialProfilesUpdate_unstable(request);
  return response.profile;
}

export async function acpDeleteCredentialProfile(
  profileId: string,
  confirmReferenced = false
): Promise<void> {
  const client = await getAcpClient();
  await client.gosling.credentialProfilesDelete_unstable({ profileId, confirmReferenced });
}

export async function acpCredentialProfileUsage(profileId: string) {
  const client = await getAcpClient();
  return client.gosling.credentialProfilesUsage_unstable({ profileId });
}

export function workspaceToMutation(workspace: Workspace): WorkspaceMutation {
  return {
    name: workspace.name,
    description: workspace.description,
    icon: workspace.icon,
    workingFolder: workspace.workingFolder,
    folders: workspace.folders ?? [],
    productOutputFolders: workspace.productOutputFolders,
    credentialBindings: workspace.credentialBindings ?? [],
    defaultCredentialBindingId: workspace.defaultCredentialBindingId,
    defaultProvider: workspace.defaultProvider,
    defaultModel: workspace.defaultModel,
  };
}

export function workspaceHasWarnings(item: WorkspaceWithValidation): boolean {
  return (item.validation.issues ?? []).some((issue) => issue.severity !== undefined);
}
