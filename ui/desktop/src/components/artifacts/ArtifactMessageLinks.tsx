import { useMemo } from 'react';
import { FileOutput } from 'lucide-react';
import { defineMessages, useIntl } from '../../i18n';
import { useArtifactWorkbench } from '../../contexts/ArtifactWorkbenchContext';
import { artifactTitleFromPath, viewableFilePathsFromMarkdown } from './artifactUtils';

const i18n = defineMessages({
  openDeliverable: {
    id: 'artifactMessageLinks.openDeliverable',
    defaultMessage: 'Open {name} in Outputs',
  },
});

interface ArtifactMessageLinksProps {
  baseDirectory?: string;
  content: string;
  workspaceId?: string;
}

export function ArtifactMessageLinks({
  content,
  baseDirectory,
  workspaceId,
}: ArtifactMessageLinksProps) {
  const intl = useIntl();
  const { openFile } = useArtifactWorkbench();
  const paths = useMemo(() => viewableFilePathsFromMarkdown(content), [content]);

  if (paths.length === 0) return null;

  return (
    <div className="mt-2 flex flex-wrap gap-2">
      {paths.map((filePath) => {
        const label = intl.formatMessage(i18n.openDeliverable, {
          name: artifactTitleFromPath(filePath),
        });
        return (
          <button
            key={filePath}
            type="button"
            aria-label={label}
            title={label}
            onClick={() => openFile(filePath, baseDirectory, workspaceId)}
            className="inline-flex max-w-full items-center gap-1.5 rounded-md border border-border-primary bg-background-secondary px-2 py-1 font-mono text-xs text-text-secondary transition-colors hover:bg-background-primary hover:text-text-primary"
          >
            <FileOutput className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">{filePath}</span>
          </button>
        );
      })}
    </div>
  );
}
