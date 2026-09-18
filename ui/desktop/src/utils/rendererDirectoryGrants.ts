import fs from 'node:fs';
import os from 'node:os';
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

/// A grant on the home directory or a filesystem root would subsume every other
/// entry, so remembering it turns "folders you approved" into "everything".
/// Such a root still works for the window that picked it; it is never stored.
function isOverlyBroadRoot(root: string): boolean {
  return root === path.parse(root).root || root === path.resolve(os.homedir());
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
        const canonical = canonicalDirectory(root);
        if (!isOverlyBroadRoot(canonical)) this.persistedRoots.add(canonical);
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
    if (persist && !isOverlyBroadRoot(root)) {
      this.persistedRoots.add(root);
      this.persist();
    }
    return root;
  }

  /// Folders the user approved stay approved across restarts and windows.
  /// Scoping them to `webContentsId === 0` made the stored list inert for the UI,
  /// so every launch re-prompted for the same folders. `isOverlyBroadRoot` is what
  /// keeps that durable set from widening into the whole home directory.
  rootsFor(webContentsId: number): string[] {
    return [...this.persistedRoots, ...(this.transientRoots.get(webContentsId) ?? [])];
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
