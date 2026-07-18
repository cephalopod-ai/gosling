import fs from 'node:fs/promises';
import path from 'node:path';

function isMissingPathError(error: unknown): boolean {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    (error as NodeJS.ErrnoException).code === 'ENOENT'
  );
}

export async function canonicalizePotentialPath(filePath: string): Promise<string> {
  let existingAncestor = path.resolve(filePath);
  const missingSegments: string[] = [];

  while (true) {
    try {
      const canonicalAncestor = await fs.realpath(existingAncestor);
      return path.resolve(canonicalAncestor, ...missingSegments.reverse());
    } catch (error) {
      if (!isMissingPathError(error)) {
        throw error;
      }

      try {
        await fs.lstat(existingAncestor);
      } catch (lstatError) {
        if (!isMissingPathError(lstatError)) {
          throw lstatError;
        }
        const parent = path.dirname(existingAncestor);
        if (parent === existingAncestor) {
          throw error;
        }
        missingSegments.push(path.basename(existingAncestor));
        existingAncestor = parent;
        continue;
      }

      throw new Error(`Cannot authorize a path through a dangling symbolic link: ${filePath}`);
    }
  }
}

function comparisonPath(filePath: string): string {
  const normalized = path.normalize(filePath);
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
}

function isPathWithinRoot(targetPath: string, rootPath: string): boolean {
  const relative = path.relative(comparisonPath(rootPath), comparisonPath(targetPath));
  return (
    relative === '' ||
    (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))
  );
}

export async function assertPathWithinRoots(
  filePath: string,
  approvedRoots: string[]
): Promise<string> {
  const canonicalPath = await canonicalizePotentialPath(filePath);
  const rootResults = await Promise.allSettled(approvedRoots.map(canonicalizePotentialPath));
  const canonicalRoots = rootResults.flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : []
  );

  if (!canonicalRoots.some((root) => isPathWithinRoot(canonicalPath, root))) {
    throw new Error('Renderer file access denied for path outside approved roots');
  }
  return canonicalPath;
}
