import { constants } from 'node:fs';
import fs from 'node:fs/promises';
import path from 'node:path';

export const RESEARCH_LIBRARY_FOLDER_NAME = 'Gosling Research Library';
export const RESEARCH_LIBRARY_FILE_LIMIT = 500;
const RESEARCH_LIBRARY_MAX_DEPTH = 6;
const RESEARCH_LIBRARY_IMPORT_NAME_ATTEMPTS = 100;

export interface ResearchLibraryFile {
  modifiedAt: string;
  name: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
}

export interface ResearchLibraryListing {
  files: ResearchLibraryFile[];
  truncated: boolean;
}

export interface ResearchLibraryImportResult {
  canceled: boolean;
  imported: string[];
  failed: string[];
}

export function defaultResearchLibraryPath(documentsPath: string): string {
  return path.join(documentsPath, RESEARCH_LIBRARY_FOLDER_NAME);
}

// COPYFILE_EXCL makes each attempt decide the collision, so a name taken between
// the check and the copy retries instead of overwriting an existing document.
async function copyIntoResearchLibrary(root: string, sourcePath: string): Promise<string> {
  const extension = path.extname(sourcePath);
  const stem = path.basename(sourcePath, extension);
  for (let attempt = 1; attempt <= RESEARCH_LIBRARY_IMPORT_NAME_ATTEMPTS; attempt += 1) {
    const name = attempt === 1 ? `${stem}${extension}` : `${stem} (${attempt})${extension}`;
    const destination = path.join(root, name);
    try {
      await fs.copyFile(sourcePath, destination, constants.COPYFILE_EXCL);
      return destination;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error;
    }
  }
  throw new Error(`The research library already holds too many copies of ${stem}${extension}`);
}

export async function importResearchLibraryFiles(
  root: string,
  sourcePaths: string[]
): Promise<Omit<ResearchLibraryImportResult, 'canceled'>> {
  const imported: string[] = [];
  const failed: string[] = [];
  for (const sourcePath of sourcePaths) {
    try {
      imported.push(await copyIntoResearchLibrary(root, sourcePath));
    } catch (error) {
      console.error(`Failed to add ${sourcePath} to the research library:`, error);
      failed.push(path.basename(sourcePath));
    }
  }
  return { imported, failed };
}

export async function listResearchLibraryFiles(
  root: string,
  extensions: string[],
  limit = RESEARCH_LIBRARY_FILE_LIMIT
): Promise<ResearchLibraryListing> {
  const allowed = new Set(extensions.map((extension) => extension.toLowerCase()));
  const files: ResearchLibraryFile[] = [];
  const scanLimit = limit + 1;

  async function visit(directory: string, depth: number): Promise<void> {
    if (depth > RESEARCH_LIBRARY_MAX_DEPTH || files.length >= scanLimit) return;
    const entries = await fs.readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));

    for (const entry of entries) {
      if (files.length >= scanLimit) break;
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
  const sorted = files.sort(
    (left, right) =>
      Date.parse(right.modifiedAt) - Date.parse(left.modifiedAt) ||
      left.relativePath.localeCompare(right.relativePath)
  );
  return {
    files: sorted.slice(0, limit),
    truncated: sorted.length > limit,
  };
}
