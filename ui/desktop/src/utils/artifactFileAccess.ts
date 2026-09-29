import fs from 'node:fs/promises';
import path from 'node:path';
import {
  assertPathWithinRoots,
  canonicalizePotentialPath,
  isPathWithinRoot,
} from './rendererFileAccess';

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

async function canonicalRoots(roots: string[]): Promise<string[]> {
  const results = await Promise.allSettled(roots.map(canonicalizePotentialPath));
  return results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
}

// A deliverable outside every approved root may still become an exact-file capability, but a
// reference that enters an approved root and leaves it through a symlink must not: the renderer
// would otherwise gain the link target, which nobody approved.
async function leavesApprovedRootThroughSymlink(
  filePath: string,
  resolvedPath: string,
  roots: string[]
): Promise<boolean> {
  if (roots.some((root) => isPathWithinRoot(resolvedPath, root))) return false;
  let ancestor = path.dirname(path.resolve(filePath));
  while (true) {
    const canonicalAncestor = await canonicalizePotentialPath(ancestor);
    if (roots.some((root) => isPathWithinRoot(canonicalAncestor, root))) return true;
    const parent = path.dirname(ancestor);
    if (parent === ancestor) return false;
    ancestor = parent;
  }
}

export async function resolveArtifactFileCapability(
  filePath: string,
  approvedRoots: string[]
): Promise<string | null> {
  if (!ARTIFACT_CAPABILITY_EXTENSIONS.has(path.extname(filePath).toLowerCase())) return null;
  const resolvedPath = await canonicalizePotentialPath(filePath);
  if (!(await fs.stat(resolvedPath)).isFile()) return null;
  const roots = await canonicalRoots(approvedRoots);
  return (await leavesApprovedRootThroughSymlink(filePath, resolvedPath, roots))
    ? null
    : resolvedPath;
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
