import fs from 'node:fs';
import path from 'node:path';
import { writeJsonFileAtomicSync, readJsonFileWithRecoverySync } from './atomicJsonStore';

interface PersistedDirectoryGrants {
  schemaVersion: 1;
  roots: string[];
}

function isPersistedDirectoryGrants(value: unknown): value is PersistedDirectoryGrants {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return (
    record.schemaVersion === 1 &&
    Array.isArray(record.roots) &&
    record.roots.length <= 256 &&
    record.roots.every((root) => typeof root === 'string' && root.length > 0 && root.length <= 4096)
  );
}

function canonicalDirectory(selectedPath: string): string {
  const absolutePath = path.resolve(selectedPath);
  const selectedStats = fs.lstatSync(absolutePath);
  if (selectedStats.isSymbolicLink())
    throw new Error('Symbolic-link directory grants are not allowed');
  const directoryPath = selectedStats.isDirectory() ? absolutePath : path.dirname(absolutePath);
  const directoryStats = fs.lstatSync(directoryPath);
  if (!directoryStats.isDirectory() || directoryStats.isSymbolicLink()) {
    throw new Error('The selected grant root is not a directory');
  }
  return fs.realpathSync.native(directoryPath);
}

export class RendererDirectoryGrantRegistry {
  private readonly persistedRoots = new Set<string>();
  private readonly transientRoots = new Map<number, Set<string>>();

  constructor(private readonly storagePath: string) {}

  load(): void {
    this.persistedRoots.clear();
    const stored = readJsonFileWithRecoverySync(this.storagePath, isPersistedDirectoryGrants);
    if (!stored) return;

    for (const root of stored.value.roots) {
      try {
        this.persistedRoots.add(canonicalDirectory(root));
      } catch {
        // Missing or moved roots remain untrusted until the user selects them again.
      }
    }

    if (this.persistedRoots.size !== stored.value.roots.length) this.persist();
  }

  grantSelectedPath(webContentsId: number, selectedPath: string, persist = true): string {
    const root = canonicalDirectory(selectedPath);
    const roots = this.transientRoots.get(webContentsId) ?? new Set<string>();
    roots.add(root);
    this.transientRoots.set(webContentsId, roots);
    if (persist) {
      this.persistedRoots.add(root);
      this.persist();
    }
    return root;
  }

  rootsFor(webContentsId: number): string[] {
    const transientRoots = [...(this.transientRoots.get(webContentsId) ?? [])];
    return webContentsId === 0 ? [...this.persistedRoots, ...transientRoots] : transientRoots;
  }

  isGrantedDirectory(webContentsId: number, directoryPath: string): boolean {
    let candidate: string;
    try {
      candidate = canonicalDirectory(directoryPath);
    } catch {
      return false;
    }
    return this.rootsFor(webContentsId).some((root) => {
      const relative = path.relative(root, candidate);
      return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
    });
  }

  clearTransient(webContentsId: number): void {
    this.transientRoots.delete(webContentsId);
  }

  private persist(): void {
    writeJsonFileAtomicSync(this.storagePath, {
      schemaVersion: 1,
      roots: [...this.persistedRoots].sort(),
    } satisfies PersistedDirectoryGrants);
  }
}
