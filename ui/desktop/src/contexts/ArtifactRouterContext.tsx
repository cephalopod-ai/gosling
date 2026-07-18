import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import type { ProductOutputFolder, WorkspaceWithValidation } from '@repo-makeover/gosling-sdk';
import { toast } from 'react-toastify';
import { acpCreateWorkspaceOutput } from '../acp/workspaces';
import { defineMessages, useIntl } from '../i18n';
import type {
  ArtifactRoutingConfig,
  RoutedArtifactSaveInput,
  RoutedArtifactSaveResult,
} from '../types/artifactRouter';
import { resolveWorkspaceArtifact } from '../utils/artifactRouting';
import { useWorkspace } from './WorkspaceContext';

const UNAVAILABLE_OUTPUT_CODES = new Set([
  'inaccessible_folder',
  'missing_output_folder',
  'not_directory',
  'path_traversal',
  'relative_path',
  'invalid_output_configuration',
]);

const i18n = defineMessages({
  unroutedDownload: {
    id: 'artifactRouter.unroutedDownload',
    defaultMessage:
      '“{fileName}” could not use a workspace output folder. Relink the output and download again.',
  },
});

interface ArtifactRouterValue {
  saveArtifact(input: RoutedArtifactSaveInput): Promise<RoutedArtifactSaveResult>;
  setVisibleSessionWorkspaceId(workspaceId: string | null | undefined): void;
}

const ArtifactRouterContext = createContext<ArtifactRouterValue | null>(null);

function outputIssue(item: WorkspaceWithValidation, outputId: string) {
  return (item.validation.issues ?? []).find(
    (issue) => issue.targetId === outputId && UNAVAILABLE_OUTPUT_CODES.has(issue.code)
  );
}

function nativeRoutingConfig(item: WorkspaceWithValidation): ArtifactRoutingConfig | null {
  const outputs = item.workspace.productOutputFolders.filter(
    (output) => !outputIssue(item, output.id)
  );
  if (outputs.length === 0) return null;
  return {
    workspaceId: item.workspace.id,
    workspaceName: item.workspace.name,
    outputs: outputs.map((output) => ({
      id: output.id,
      isDefault: output.isDefault,
      path: output.path,
      productTypes: output.productTypes,
    })),
  };
}

export function ArtifactRouterProvider({ children }: { children: React.ReactNode }) {
  const intl = useIntl();
  const { activeWorkspaceId, refreshWorkspaces, workspaces } = useWorkspace();
  const [visibleSessionWorkspaceId, setVisibleSessionWorkspaceId] = useState<
    string | null | undefined
  >(undefined);

  const nativeWorkspace = useMemo(() => {
    const workspaceId = visibleSessionWorkspaceId ?? activeWorkspaceId;
    if (!workspaceId) return null;
    return workspaces.find((item) => item.workspace.id === workspaceId) ?? null;
  }, [activeWorkspaceId, visibleSessionWorkspaceId, workspaces]);

  useEffect(() => {
    const config = nativeWorkspace ? nativeRoutingConfig(nativeWorkspace) : null;
    void window.electron.setArtifactRoutingConfig(config).catch(() => {});
    return () => {
      void window.electron.setArtifactRoutingConfig(null).catch(() => {});
    };
  }, [nativeWorkspace]);

  useEffect(() => {
    const handleUnroutedDownload = (_event: unknown, fileName: string) => {
      toast.warning(intl.formatMessage(i18n.unroutedDownload, { fileName }));
    };
    window.electron.on('artifact-download-unrouted', handleUnroutedDownload);
    return () => window.electron.off('artifact-download-unrouted', handleUnroutedDownload);
  }, [intl]);

  const ensureOutputAvailable = useCallback(
    async (
      item: WorkspaceWithValidation,
      output: ProductOutputFolder
    ): Promise<'available' | 'canceled'> => {
      const issue = outputIssue(item, output.id);
      if (!issue) return 'available';
      if (issue.code !== 'missing_output_folder' || !output.createIfMissing) {
        throw new Error(`${issue.message}. Relink this output in the workspace editor.`);
      }

      const response = await window.electron.showMessageBox({
        type: 'question',
        buttons: ['Cancel', 'Create folder and save'],
        defaultId: 0,
        title: 'Create workspace output folder',
        message: `Create “${output.label}” at ${output.path}?`,
      });
      if (response.response !== 1) return 'canceled';
      await acpCreateWorkspaceOutput(item.workspace.id, output.id);
      await refreshWorkspaces();
      return 'available';
    },
    [refreshWorkspaces]
  );

  const saveArtifact = useCallback(
    async (input: RoutedArtifactSaveInput): Promise<RoutedArtifactSaveResult> => {
      const workspaceId = input.workspaceId ?? activeWorkspaceId;
      const item = workspaceId
        ? workspaces.find((candidate) => candidate.workspace.id === workspaceId)
        : null;
      if (!item) {
        throw new Error(
          input.workspaceId
            ? 'The session workspace was deleted. Relink it before saving this artifact.'
            : 'No active workspace is available for this artifact.'
        );
      }

      const route = resolveWorkspaceArtifact(item.workspace, input);
      if (!route.output) {
        throw new Error('This workspace has no product output folder. Add one before saving.');
      }
      if ((await ensureOutputAvailable(item, route.output)) === 'canceled') {
        return { canceled: true };
      }

      const response = await window.electron.saveArtifact({
        defaultPath: route.defaultPath,
        filters: input.filters,
        source: input.source,
        title: input.title,
      });
      return {
        ...response,
        outputFolderId: route.output.id,
        productType: route.productType,
        workspaceId: item.workspace.id,
      };
    },
    [activeWorkspaceId, ensureOutputAvailable, workspaces]
  );

  const value = useMemo<ArtifactRouterValue>(
    () => ({ saveArtifact, setVisibleSessionWorkspaceId }),
    [saveArtifact]
  );
  return <ArtifactRouterContext.Provider value={value}>{children}</ArtifactRouterContext.Provider>;
}

export function useArtifactRouter(): ArtifactRouterValue {
  const context = useContext(ArtifactRouterContext);
  if (!context) throw new Error('useArtifactRouter must be used within ArtifactRouterProvider');
  return context;
}
