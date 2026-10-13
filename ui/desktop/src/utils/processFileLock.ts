import { randomUUID } from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';

const OWNER_PATTERN = /^(\d+)-[a-f0-9-]{36}\.owner$/;
const LOCK_TIMEOUT_MS = 10_000;

function ownerIsRunning(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code !== 'ESRCH';
  }
}

async function removeEmptyDirectory(directory: string): Promise<void> {
  try {
    await fs.rmdir(directory);
  } catch (error) {
    if (!['ENOENT', 'ENOTEMPTY', 'EEXIST'].includes((error as NodeJS.ErrnoException).code ?? '')) {
      throw error;
    }
  }
}

async function recoverAbandonedLock(directory: string): Promise<void> {
  let entries: string[];
  try {
    const metadata = await fs.lstat(directory);
    if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
      throw new Error('Process registry lock is not a real directory');
    }
    entries = await fs.readdir(directory);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return;
    throw error;
  }
  if (entries.length === 0) {
    await removeEmptyDirectory(directory);
    return;
  }
  if (entries.length !== 1) return;
  const owner = entries[0];
  const match = OWNER_PATTERN.exec(owner);
  if (!match || ownerIsRunning(Number(match[1]))) return;
  // Remove the exact old owner token. A newly published owner's different file
  // makes rmdir fail safely, even when recovery races another acquisition.
  await fs.unlink(path.join(directory, owner)).catch((error: NodeJS.ErrnoException) => {
    if (error.code !== 'ENOENT') throw error;
  });
  await removeEmptyDirectory(directory);
}

/** Publish a nonempty lock directory atomically, then read/update/write under it.
 * Renaming onto another nonempty directory fails on both POSIX and Windows;
 * the owner marker exists before publication, so there is no missing-PID window.
 */
export async function withProcessFileLock<T>(
  filePath: string,
  operation: () => Promise<T>
): Promise<T> {
  const parent = path.dirname(filePath);
  await fs.mkdir(parent, { recursive: true, mode: 0o700 });
  const prefix = `.${path.basename(filePath)}.lock-candidate-`;
  for (const entry of await fs.readdir(parent, { withFileTypes: true })) {
    if (!entry.isDirectory() || !entry.name.startsWith(prefix)) continue;
    const owner = /^(\d+)-[a-f0-9-]{36}$/.exec(entry.name.slice(prefix.length));
    if (owner && !ownerIsRunning(Number(owner[1]))) {
      await recoverAbandonedLock(path.join(parent, entry.name));
    }
  }

  const token = `${process.pid}-${randomUUID()}`;
  const ownerFile = `${token}.owner`;
  const staged = path.join(parent, `${prefix}${token}`);
  const lock = `${filePath}.lock`;
  let acquired = false;
  await fs.mkdir(staged, { mode: 0o700 });
  try {
    await fs.writeFile(path.join(staged, ownerFile), '', { flag: 'wx', mode: 0o600 });
    const deadline = Date.now() + LOCK_TIMEOUT_MS;
    while (!acquired) {
      try {
        await fs.rename(staged, lock);
        acquired = true;
      } catch (error) {
        if (
          !['EEXIST', 'ENOTEMPTY', 'EPERM'].includes((error as NodeJS.ErrnoException).code ?? '')
        ) {
          throw error;
        }
        await recoverAbandonedLock(lock);
        if (Date.now() >= deadline)
          throw new Error('Timed out waiting for the process registry lock');
        await new Promise<void>((resolve) => setTimeout(resolve, 25));
      }
    }
    return await operation();
  } finally {
    if (acquired) {
      await fs.unlink(path.join(lock, ownerFile)).catch((error: NodeJS.ErrnoException) => {
        if (error.code !== 'ENOENT') throw error;
      });
      await removeEmptyDirectory(lock);
    } else {
      await fs.rm(staged, { recursive: true, force: true });
    }
  }
}
