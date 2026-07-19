import path from 'node:path';
import { assertPathWithinRoots, canonicalizePotentialPath } from './rendererFileAccess';

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
