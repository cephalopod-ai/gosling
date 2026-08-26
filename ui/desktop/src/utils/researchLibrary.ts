import fs from 'node:fs/promises';
import path from 'node:path';

export const RESEARCH_LIBRARY_FOLDER_NAME = 'Gosling Research Library';
export const RESEARCH_LIBRARY_FILE_LIMIT = 500;
const RESEARCH_LIBRARY_MAX_DEPTH = 6;

export interface ResearchLibraryFile {
  modifiedAt: string;
  name: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
}

export function defaultResearchLibraryPath(documentsPath: string): string {
  return path.join(documentsPath, RESEARCH_LIBRARY_FOLDER_NAME);
}

export async function listResearchLibraryFiles(
  root: string,
  extensions: string[],
  limit = RESEARCH_LIBRARY_FILE_LIMIT
): Promise<ResearchLibraryFile[]> {
  const allowed = new Set(extensions.map((extension) => extension.toLowerCase()));
  const files: ResearchLibraryFile[] = [];

  async function visit(directory: string, depth: number): Promise<void> {
    if (depth > RESEARCH_LIBRARY_MAX_DEPTH || files.length >= limit) return;
    const entries = await fs.readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));

    for (const entry of entries) {
      if (files.length >= limit) break;
      if (entry.name.startsWith('.') || entry.isSymbolicLink()) continue;
      const filePath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(filePath, depth + 1);
        continue;
      }
      if (!entry.isFile()) continue;
      const extension = path.extname(entry.name).toLowerCase().replace(/^\./, '');
      if (!allowed.has(extension)) continue;
      const stats = await fs.stat(filePath);
      files.push({
        modifiedAt: stats.mtime.toISOString(),
        name: entry.name,
        path: filePath,
        relativePath: path.relative(root, filePath),
        sizeBytes: stats.size,
      });
    }
  }

  await visit(root, 0);
  return files.sort(
    (left, right) =>
      Date.parse(right.modifiedAt) - Date.parse(left.modifiedAt) ||
      left.relativePath.localeCompare(right.relativePath)
  );
}
