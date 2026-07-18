import type { SaveDialogOptions, SaveDialogReturnValue } from 'electron';
import { Buffer } from 'node:buffer';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { ArtifactSaveRequest, ArtifactSaveResponse } from '../types/artifactRouter';

export const MAX_ARTIFACT_CONTENT_LENGTH = 256 * 1024 * 1024;

interface ArtifactSaveDependencies {
  maxContentLength?: number;
  resolveSource(path: string, baseDirectory?: string): Promise<string>;
  showSaveDialog(options: SaveDialogOptions): Promise<SaveDialogReturnValue>;
}

function decodeBase64(content: string): Buffer {
  const normalized = content.replace(/\s/g, '');
  if (
    normalized.length % 4 === 1 ||
    !/^[a-zA-Z0-9+/]*={0,2}$/.test(normalized) ||
    /=/.test(normalized.slice(0, -2))
  ) {
    throw new Error('The artifact contains invalid base64 data');
  }
  const decoded = Buffer.from(normalized, 'base64');
  if (decoded.toString('base64').replace(/=+$/g, '') !== normalized.replace(/=+$/g, '')) {
    throw new Error('The artifact contains invalid base64 data');
  }
  return decoded;
}

export async function saveArtifactWithDialog(
  request: ArtifactSaveRequest,
  dependencies: ArtifactSaveDependencies
): Promise<ArtifactSaveResponse> {
  if (
    request.source.type === 'content' &&
    request.source.content.length > (dependencies.maxContentLength ?? MAX_ARTIFACT_CONTENT_LENGTH)
  ) {
    throw new Error('The in-memory artifact is too large to save safely');
  }
  const result = await dependencies.showSaveDialog({
    defaultPath: request.defaultPath,
    filters: request.filters,
    title: request.title,
  });
  if (result.canceled || !result.filePath) return { canceled: true };

  const targetPath = path.resolve(result.filePath);
  if (request.source.type === 'file') {
    const sourcePath = await dependencies.resolveSource(
      request.source.path,
      request.source.baseDirectory
    );
    if (path.resolve(sourcePath) !== targetPath) {
      await fs.copyFile(sourcePath, targetPath);
    }
  } else {
    const content =
      request.source.encoding === 'base64'
        ? decodeBase64(request.source.content)
        : request.source.content;
    await fs.writeFile(targetPath, content);
  }
  return { canceled: false, filePath: targetPath };
}
