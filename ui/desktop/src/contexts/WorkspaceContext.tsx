import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type {
  CredentialProfile,
  CredentialProfileCreateRequest_unstable,
  CredentialProfileUpdateRequest_unstable,
  Workspace,
  WorkspaceMutation,
  WorkspaceValidationReport,
  WorkspaceWithValidation,
} from '@repo-makeover/gosling-sdk';
import {
  acpCreateCredentialProfile,
  acpCreateWorkspace,
  acpDeleteCredentialProfile,
  acpDeleteWorkspace,
  acpDuplicateWorkspace,
  acpListCredentialProfiles,
  acpListWorkspaces,
  acpSetActiveWorkspace,
  acpUpdateCredentialProfile,
  acpUpdateWorkspace,
  acpValidateWorkspace,
} from '../acp/workspaces';
import { workspaceErrorMessage } from '../utils/workspaceError';

interface WorkspaceContextValue {
  workspaces: WorkspaceWithValidation[];
  activeWorkspace: Workspace | null;
  activeWorkspaceId: string | null;
  defaultWorkspaceId: string | null;
  credentialProfiles: CredentialProfile[];
  loading: boolean;
  error: string | null;
  sessionWorkspaceFilterId: string | null;
  setSessionWorkspaceFilterId(workspaceId: string | null): void;
  refreshWorkspaces(): Promise<void>;
  createWorkspace(workspace: WorkspaceMutation): Promise<Workspace>;
  updateWorkspace(workspaceId: string, workspace: WorkspaceMutation): Promise<Workspace>;
  duplicateWorkspace(workspaceId: string): Promise<Workspace>;
  deleteWorkspace(workspaceId: string): Promise<void>;
  setActiveWorkspace(workspaceId: string): Promise<Workspace>;
  validateWorkspace(
    workspace: WorkspaceMutation,
    workspaceId?: string
  ): Promise<WorkspaceValidationReport>;
  createCredentialProfile(
    request: CredentialProfileCreateRequest_unstable
  ): Promise<CredentialProfile>;
  updateCredentialProfile(
    request: CredentialProfileUpdateRequest_unstable
  ): Promise<CredentialProfile>;
  deleteCredentialProfile(profileId: string, confirmReferenced?: boolean): Promise<void>;
}

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

export function WorkspaceProvider({ children }: { children: React.ReactNode }) {
  const [workspaces, setWorkspaces] = useState<WorkspaceWithValidation[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(null);
  const [defaultWorkspaceId, setDefaultWorkspaceId] = useState<string | null>(null);
  const [credentialProfiles, setCredentialProfiles] = useState<CredentialProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sessionWorkspaceFilterId, setSessionWorkspaceFilterIdState] = useState<
    string | null | undefined
  >(() => {
    const stored = window.localStorage.getItem('workspace_session_filter');
    if (stored === null) return undefined;
    return stored === '__all__' ? null : stored;
  });

  const refreshWorkspaces = useCallback(async () => {
    try {
      const [workspaceResponse, profiles] = await Promise.all([
        acpListWorkspaces(),
        acpListCredentialProfiles(),
      ]);
      setWorkspaces(workspaceResponse.workspaces);
      setActiveWorkspaceId(workspaceResponse.activeWorkspaceId);
      setDefaultWorkspaceId(workspaceResponse.defaultWorkspaceId);
      setSessionWorkspaceFilterIdState((current) => {
        const remainsAvailable =
          current === null ||
          workspaceResponse.workspaces.some((item) => item.workspace.id === current);
        const next =
          current === undefined || !remainsAvailable
            ? workspaceResponse.activeWorkspaceId
            : current;
        if (next !== current) {
          window.localStorage.setItem('workspace_session_filter', next ?? '__all__');
        }
        return next;
      });
      setCredentialProfiles(profiles);
      setError(null);
    } catch (cause) {
      setError(workspaceErrorMessage(cause, 'Unable to load workspaces'));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refreshWorkspaces();
  }, [refreshWorkspaces]);

  useEffect(() => {
    const handleWorkspaceChanged = () => void refreshWorkspaces();
    window.electron.on('workspaces-changed', handleWorkspaceChanged);
    return () => window.electron.off('workspaces-changed', handleWorkspaceChanged);
  }, [refreshWorkspaces]);

  const notifyChanged = useCallback(async () => {
    await refreshWorkspaces();
    window.electron.broadcastWorkspaceChange();
  }, [refreshWorkspaces]);

  const setSessionWorkspaceFilterId = useCallback((workspaceId: string | null) => {
    setSessionWorkspaceFilterIdState(workspaceId);
    window.localStorage.setItem('workspace_session_filter', workspaceId ?? '__all__');
  }, []);

  const createWorkspace = useCallback(
    async (workspace: WorkspaceMutation) => {
      const response = await acpCreateWorkspace(workspace);
      await notifyChanged();
      return response.workspace;
    },
    [notifyChanged]
  );

  const updateWorkspace = useCallback(
    async (workspaceId: string, workspace: WorkspaceMutation) => {
      const response = await acpUpdateWorkspace(workspaceId, workspace);
      await notifyChanged();
      return response.workspace;
    },
    [notifyChanged]
  );

  const duplicateWorkspace = useCallback(
    async (workspaceId: string) => {
      const response = await acpDuplicateWorkspace(workspaceId);
      await notifyChanged();
      return response.workspace;
    },
    [notifyChanged]
  );

  const deleteWorkspace = useCallback(
    async (workspaceId: string) => {
      await acpDeleteWorkspace(workspaceId);
      await notifyChanged();
    },
    [notifyChanged]
  );

  const setActiveWorkspace = useCallback(
    async (workspaceId: string) => {
      const response = await acpSetActiveWorkspace(workspaceId);
      setSessionWorkspaceFilterId(workspaceId);
      await notifyChanged();
      return response.workspace;
    },
    [notifyChanged, setSessionWorkspaceFilterId]
  );

  const createCredentialProfile = useCallback(
    async (request: CredentialProfileCreateRequest_unstable) => {
      const profile = await acpCreateCredentialProfile(request);
      await notifyChanged();
      return profile;
    },
    [notifyChanged]
  );

  const updateCredentialProfile = useCallback(
    async (request: CredentialProfileUpdateRequest_unstable) => {
      const profile = await acpUpdateCredentialProfile(request);
      await notifyChanged();
      return profile;
    },
    [notifyChanged]
  );

  const deleteCredentialProfile = useCallback(
    async (profileId: string, confirmReferenced = false) => {
      await acpDeleteCredentialProfile(profileId, confirmReferenced);
      await notifyChanged();
    },
    [notifyChanged]
  );

  const activeWorkspace = useMemo(
    () => workspaces.find((item) => item.workspace.id === activeWorkspaceId)?.workspace ?? null,
    [activeWorkspaceId, workspaces]
  );

  const value = useMemo<WorkspaceContextValue>(
    () => ({
      workspaces,
      activeWorkspace,
      activeWorkspaceId,
      defaultWorkspaceId,
      credentialProfiles,
      loading,
      error,
      sessionWorkspaceFilterId: sessionWorkspaceFilterId ?? null,
      setSessionWorkspaceFilterId,
      refreshWorkspaces,
      createWorkspace,
      updateWorkspace,
      duplicateWorkspace,
      deleteWorkspace,
      setActiveWorkspace,
      validateWorkspace: acpValidateWorkspace,
      createCredentialProfile,
      updateCredentialProfile,
      deleteCredentialProfile,
    }),
    [
      workspaces,
      activeWorkspace,
      activeWorkspaceId,
      defaultWorkspaceId,
      credentialProfiles,
      loading,
      error,
      sessionWorkspaceFilterId,
      setSessionWorkspaceFilterId,
      refreshWorkspaces,
      createWorkspace,
      updateWorkspace,
      duplicateWorkspace,
      deleteWorkspace,
      setActiveWorkspace,
      createCredentialProfile,
      updateCredentialProfile,
      deleteCredentialProfile,
    ]
  );

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace(): WorkspaceContextValue {
  const context = useContext(WorkspaceContext);
  if (!context) {
    throw new Error('useWorkspace must be used within WorkspaceProvider');
  }
  return context;
}

export function useOptionalWorkspace(): WorkspaceContextValue | undefined {
  return useContext(WorkspaceContext) ?? undefined;
}
