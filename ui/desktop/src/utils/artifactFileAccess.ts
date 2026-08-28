import fs from 'node:fs/promises';
import path from 'node:path';
import { assertPathWithinRoots, canonicalizePotentialPath } from './rendererFileAccess';

const ARTIFACT_CAPABILITY_EXTENSIONS = new Set([
  '.csv',
  '.doc',
  '.docx',
  '.json',
  '.jsonl',
  '.md',
  '.markdown',
  '.mdown',
  '.ods',
  '.odt',
  '.pdf',
  '.ppt',
  '.pptx',
  '.rtf',
  '.tsv',
  '.txt',
  '.xls',
  '.xlsx',
]);

export async function resolveArtifactFileCapability(filePath: string): Promise<string | null> {
  if (!ARTIFACT_CAPABILITY_EXTENSIONS.has(path.extname(filePath).toLowerCase())) return null;
  const resolvedPath = await canonicalizePotentialPath(filePath);
  return (await fs.stat(resolvedPath)).isFile() ? resolvedPath : null;
}

export async function assertArtifactFileAccess(
  filePath: string,
  baseDirectory: string | undefined,
  approvedRoots: string[],
  routedOutputRoots: string[],
  grantedFiles: Set<string>
): Promise<string> {
  const candidate =
    baseDirectory && !path.isAbsolute(filePath) ? path.join(baseDirectory, filePath) : filePath;
  const resolvedPath = await canonicalizePotentialPath(candidate);
  if (grantedFiles.has(resolvedPath)) return resolvedPath;
  return assertPathWithinRoots(resolvedPath, [...approvedRoots, ...routedOutputRoots]);
}
