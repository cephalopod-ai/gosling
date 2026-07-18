import { acpExportSession, type SessionListItem } from '../acp/sessions';
import type { RoutedArtifactSaveInput, RoutedArtifactSaveResult } from '../types/artifactRouter';

export async function saveSessionArtifactExport(
  session: SessionListItem,
  saveArtifact: (input: RoutedArtifactSaveInput) => Promise<RoutedArtifactSaveResult>
): Promise<RoutedArtifactSaveResult> {
  const json = await acpExportSession(session.id);
  return saveArtifact({
    workspaceId: session.workspaceId,
    productType: 'export',
    suggestedName: `${session.name}.json`,
    title: 'Export session',
    filters: [{ name: 'Gosling session', extensions: ['json'] }],
    source: { type: 'content', content: json, encoding: 'utf8' },
  });
}
