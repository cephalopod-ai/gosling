import { act, render, waitFor } from '@testing-library/react';
import type { Workspace, WorkspaceWithValidation } from '@repo-makeover/gosling-sdk';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'react-toastify';
import { acpCreateWorkspaceOutput } from '../acp/workspaces';
import { ArtifactRouterProvider, useArtifactRouter } from './ArtifactRouterContext';
import { useWorkspace } from './WorkspaceContext';
import { IntlTestWrapper } from '../i18n/test-utils';

vi.mock('./WorkspaceContext', () => ({ useWorkspace: vi.fn() }));
vi.mock('../acp/workspaces', () => ({ acpCreateWorkspaceOutput: vi.fn() }));
vi.mock('react-toastify', () => ({ toast: { warning: vi.fn() } }));

function makeWorkspace(id: string, root: string): Workspace {
  return {
    id,
    schemaVersion: 1,
    name: id,
    workingFolder: root,
    folders: [],
    productOutputFolders: [
      {
        id: `${id}-documents`,
        label: 'Documents',
        path: `${root}/Documents`,
        productTypes: ['document', 'export'],
        isDefault: true,
        createIfMissing: false,
      },
      {
        id: `${id}-images`,
        label: 'Images',
        path: `${root}/Images`,
        productTypes: ['image'],
        isDefault: false,
        createIfMissing: false,
      },
      {
        id: `${id}-slides`,
        label: 'Slides',
        path: `${root}/Slides`,
        productTypes: ['presentation'],
        isDefault: false,
        createIfMissing: false,
      },
    ],
    credentialBindings: [],
    createdAt: '2026-07-18T00:00:00Z',
    updatedAt: '2026-07-18T00:00:00Z',
    lastOpenedAt: '2026-07-18T00:00:00Z',
  };
}

const active = makeWorkspace('active', '/active');
const pinned = makeWorkspace('pinned', '/pinned');

describe('ArtifactRouterProvider', () => {
  let router: ReturnType<typeof useArtifactRouter>;
  let saveArtifact: ReturnType<typeof vi.fn>;
  let setArtifactRoutingConfig: ReturnType<typeof vi.fn>;
  let refreshWorkspaces: ReturnType<typeof vi.fn>;
  let unroutedHandler: ((event: unknown, fileName: string) => void) | undefined;

  function Harness() {
    router = useArtifactRouter();
    return null;
  }

  function renderRouter() {
    return render(
      <IntlTestWrapper>
        <ArtifactRouterProvider>
          <Harness />
        </ArtifactRouterProvider>
      </IntlTestWrapper>
    );
  }

  function setWorkspaceContext(items: WorkspaceWithValidation[]) {
    vi.mocked(useWorkspace).mockReturnValue({
      activeWorkspace: active,
      activeWorkspaceId: active.id,
      defaultWorkspaceId: active.id,
      workspaces: items,
      credentialProfiles: [],
      loading: false,
      error: null,
      sessionWorkspaceFilterId: null,
      refreshWorkspaces,
    } as unknown as ReturnType<typeof useWorkspace>);
  }

  beforeEach(() => {
    vi.clearAllMocks();
    saveArtifact = vi.fn().mockResolvedValue({ canceled: false, filePath: '/saved' });
    setArtifactRoutingConfig = vi.fn().mockResolvedValue(true);
    refreshWorkspaces = vi.fn().mockResolvedValue(undefined);
    unroutedHandler = undefined;
    Object.assign(window.electron, {
      saveArtifact,
      setArtifactRoutingConfig,
      on: vi.fn((channel: string, callback: typeof unroutedHandler) => {
        if (channel === 'artifact-download-unrouted') unroutedHandler = callback;
      }),
      off: vi.fn(),
    });
    setWorkspaceContext([
      { workspace: active, validation: { validForSession: true, issues: [] } },
      { workspace: pinned, validation: { validForSession: true, issues: [] } },
    ]);
  });

  it('configures native downloads and routes active-workspace images', async () => {
    renderRouter();

    await waitFor(() => expect(setArtifactRoutingConfig).toHaveBeenCalled());
    await act(() =>
      router.saveArtifact({
        suggestedName: 'hero.png',
        mimeType: 'image/png',
        source: { type: 'content', content: 'AA==', encoding: 'base64' },
      })
    );

    expect(saveArtifact).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: '/active/Images/hero.png' })
    );
    expect(setArtifactRoutingConfig).toHaveBeenCalledWith(
      expect.objectContaining({ workspaceId: 'active' })
    );
  });

  it('routes an existing session artifact through its pinned workspace', async () => {
    renderRouter();
    await act(() =>
      router.saveArtifact({
        workspaceId: 'pinned',
        suggestedName: 'brief.pptx',
        source: { type: 'file', path: '/pinned/brief.pptx' },
      })
    );
    expect(saveArtifact).toHaveBeenCalledWith(
      expect.objectContaining({ defaultPath: '/pinned/Slides/brief.pptx' })
    );
  });

  it('keeps native downloads pinned to the visible session workspace', async () => {
    renderRouter();
    await act(() => router.setVisibleSessionWorkspaceId('pinned'));
    await waitFor(() =>
      expect(setArtifactRoutingConfig).toHaveBeenLastCalledWith(
        expect.objectContaining({ workspaceId: 'pinned' })
      )
    );
  });

  it('warns instead of silently falling back when a native download is unroutable', () => {
    renderRouter();
    act(() => unroutedHandler?.({}, 'brief.pdf'));
    expect(toast.warning).toHaveBeenCalledWith(expect.stringContaining('brief.pdf'));
    expect(toast.warning).toHaveBeenCalledWith(expect.stringContaining('Relink'));
  });

  it('does not silently fall back when a pinned workspace was deleted', async () => {
    renderRouter();
    await expect(
      router.saveArtifact({
        workspaceId: 'deleted',
        suggestedName: 'report.pdf',
        source: { type: 'content', content: 'pdf', encoding: 'utf8' },
      })
    ).rejects.toThrow('session workspace was deleted');
    expect(saveArtifact).not.toHaveBeenCalled();
  });

  it('requires confirmation before creating a missing configured output', async () => {
    const withMissingOutput = makeWorkspace('active', '/active');
    withMissingOutput.productOutputFolders[1].createIfMissing = true;
    setWorkspaceContext([
      {
        workspace: withMissingOutput,
        validation: {
          validForSession: true,
          issues: [
            {
              code: 'missing_output_folder',
              severity: 'warning',
              message: 'Output folder is missing',
              targetId: 'active-images',
            },
          ],
        },
      },
    ]);
    Object.assign(window.electron, {
      showMessageBox: vi.fn().mockResolvedValue({ response: 1 }),
    });
    vi.mocked(acpCreateWorkspaceOutput).mockResolvedValue({
      validation: { validForSession: true, issues: [] },
    });
    renderRouter();

    await act(() =>
      router.saveArtifact({
        suggestedName: 'hero.png',
        source: { type: 'content', content: 'AA==', encoding: 'base64' },
      })
    );
    expect(window.electron.showMessageBox).toHaveBeenCalled();
    expect(acpCreateWorkspaceOutput).toHaveBeenCalledWith('active', 'active-images');
    expect(refreshWorkspaces).toHaveBeenCalled();
  });
});
