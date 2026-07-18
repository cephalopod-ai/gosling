import type { Session } from 'electron';
import fs from 'node:fs';
import path from 'node:path';
import type { ArtifactRoutingConfig } from '../types/artifactRouter';
import {
  inferArtifactProductType,
  safeArtifactFileName,
  selectArtifactOutput,
} from './artifactRouting';

const installedSessions = new WeakSet<Session>();

export function availableDownloadPath(
  directory: string,
  suggestedName: string,
  exists: (candidate: string) => boolean = fs.existsSync
): string {
  const fileName = safeArtifactFileName(suggestedName);
  const initial = path.join(directory, fileName);
  if (!exists(initial)) return initial;

  const extension = path.extname(fileName);
  const stem = path.basename(fileName, extension);
  for (let index = 1; index < 10_000; index += 1) {
    const candidate = path.join(directory, `${stem} (${index})${extension}`);
    if (!exists(candidate)) return candidate;
  }
  throw new Error('Unable to find an available artifact download name');
}

export function routedDownloadPath(
  config: ArtifactRoutingConfig,
  fileName: string,
  mimeType?: string,
  exists?: (candidate: string) => boolean
): string | null {
  const productType = inferArtifactProductType({ suggestedName: fileName, mimeType });
  const output = selectArtifactOutput(config.outputs, productType);
  return output ? availableDownloadPath(output.path, fileName, exists) : null;
}

export function installArtifactDownloadRouter(
  electronSession: Session,
  configForWebContents: (webContentsId: number) => ArtifactRoutingConfig | undefined,
  onUnrouted: (webContentsId: number, fileName: string) => void
): void {
  if (installedSessions.has(electronSession)) return;
  installedSessions.add(electronSession);
  const reservedPaths = new Set<string>();
  electronSession.on('will-download', (_event, item, webContents) => {
    const config = configForWebContents(webContents.id);
    if (!config) {
      onUnrouted(webContents.id, item.getFilename());
      return;
    }
    try {
      const destination = routedDownloadPath(
        config,
        item.getFilename(),
        item.getMimeType(),
        (candidate) => reservedPaths.has(candidate) || fs.existsSync(candidate)
      );
      if (!destination) {
        onUnrouted(webContents.id, item.getFilename());
        return;
      }
      reservedPaths.add(destination);
      item.setSavePath(destination);
      item.once('done', () => reservedPaths.delete(destination));
    } catch {
      onUnrouted(webContents.id, item.getFilename());
    }
  });
}
