import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkspaceMutation } from '@repo-makeover/gosling-sdk';
import {
  acpCreateWorkspace,
  acpListCredentialProfiles,
  acpListWorkspaces,
  acpSetActiveWorkspace,
} from '../acp/workspaces';
import { useWorkspace, WorkspaceProvider } from './WorkspaceContext';

vi.mock('../acp/workspaces', () => ({
  acpCreateCredentialProfile: vi.fn(),
  acpCreateWorkspace: vi.fn(),
  acpDeleteCredentialProfile: vi.fn(),
  acpDeleteWorkspace: vi.fn(),
  acpDuplicateWorkspace: vi.fn(),
  acpListCredentialProfiles: vi.fn(),
  acpListWorkspaces: vi.fn(),
  acpSetActiveWorkspace: vi.fn(),
  acpUpdateCredentialProfile: vi.fn(),
  acpUpdateWorkspace: vi.fn(),
  acpValidateWorkspace: vi.fn(),
}));

const workspace = {
  id: 'workspace-1',
  schemaVersion: 1,
  name: 'Project',
  workingFolder: '/workspace/project',
  productOutputFolders: [
    {
      id: 'output-1',
      label: 'Outputs',
      path: '/workspace/project/Outputs',
      productTypes: ['document' as const],
      isDefault: true,
      createIfMissing: true,
    },
  ],
  createdAt: '2026-07-18T00:00:00Z',
  updatedAt: '2026-07-18T00:00:00Z',
  lastOpenedAt: '2026-07-18T00:00:00Z',
};

function Probe({ onValue }: { onValue(value: ReturnType<typeof useWorkspace>): void }) {
  const value = useWorkspace();
  onValue(value);
  return <div>{value.activeWorkspace?.name ?? (value.loading ? 'Loading' : 'Missing')}</div>;
}

describe('WorkspaceContext', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    Object.assign(window.electron, {
      broadcastWorkspaceChange: vi.fn(),
      on: vi.fn(),
      off: vi.fn(),
    });
    vi.mocked(acpListWorkspaces).mockResolvedValue({
      workspaces: [
        {
          workspace,
          validation: { validForSession: true, issues: [] },
        },
      ],
      activeWorkspaceId: workspace.id,
      defaultWorkspaceId: workspace.id,
    });
    vi.mocked(acpListCredentialProfiles).mockResolvedValue([]);
    vi.mocked(acpSetActiveWorkspace).mockResolvedValue({
      workspace,
      validation: { validForSession: true, issues: [] },
    });
    vi.mocked(acpCreateWorkspace).mockResolvedValue({
      workspace,
      validation: { validForSession: true, issues: [] },
    });
  });

  it('loads the backend workspace source of truth and defaults chat filtering to active', async () => {
    let context!: ReturnType<typeof useWorkspace>;
    render(
      <WorkspaceProvider>
        <Probe onValue={(value) => (context = value)} />
      </WorkspaceProvider>
    );

    expect(await screen.findByText('Project')).toBeInTheDocument();
    expect(context.activeWorkspace?.workingFolder).toBe('/workspace/project');
    expect(context.sessionWorkspaceFilterId).toBe('workspace-1');
    expect(acpListWorkspaces).toHaveBeenCalled();
  });

  it('switches future-session defaults without touching a visible session store', async () => {
    let context!: ReturnType<typeof useWorkspace>;
    render(
      <WorkspaceProvider>
        <Probe onValue={(value) => (context = value)} />
      </WorkspaceProvider>
    );
    await screen.findByText('Project');

    await act(() => context.setActiveWorkspace('workspace-1'));

    expect(acpSetActiveWorkspace).toHaveBeenCalledWith('workspace-1');
    expect(window.electron.broadcastWorkspaceChange).toHaveBeenCalled();
    expect(window.localStorage.getItem('workspace_session_filter')).toBe('workspace-1');
  });

  it('creates workspaces through the backend and refreshes shared state', async () => {
    let context!: ReturnType<typeof useWorkspace>;
    render(
      <WorkspaceProvider>
        <Probe onValue={(value) => (context = value)} />
      </WorkspaceProvider>
    );
    await screen.findByText('Project');
    const mutation: WorkspaceMutation = {
      name: 'Project',
      workingFolder: '/workspace/project',
      productOutputFolders: workspace.productOutputFolders,
    };

    await act(() => context.createWorkspace(mutation));

    expect(acpCreateWorkspace).toHaveBeenCalledWith(mutation);
    await waitFor(() => expect(acpListWorkspaces).toHaveBeenCalledTimes(2));
  });

  it('repairs a persisted session filter that references a deleted workspace', async () => {
    window.localStorage.setItem('workspace_session_filter', 'deleted-workspace');
    let context!: ReturnType<typeof useWorkspace>;
    render(
      <WorkspaceProvider>
        <Probe onValue={(value) => (context = value)} />
      </WorkspaceProvider>
    );

    await screen.findByText('Project');
    await waitFor(() => expect(context.sessionWorkspaceFilterId).toBe('workspace-1'));
    expect(window.localStorage.getItem('workspace_session_filter')).toBe('workspace-1');
  });

  it('refreshes when another Desktop window broadcasts a workspace mutation', async () => {
    render(
      <WorkspaceProvider>
        <Probe onValue={vi.fn()} />
      </WorkspaceProvider>
    );
    await screen.findByText('Project');
    const listener = vi
      .mocked(window.electron.on)
      .mock.calls.find(([channel]) => channel === 'workspaces-changed')?.[1];

    expect(listener).toBeDefined();
    await act(async () => listener?.({} as never));

    await waitFor(() => expect(acpListWorkspaces).toHaveBeenCalledTimes(2));
  });
});
