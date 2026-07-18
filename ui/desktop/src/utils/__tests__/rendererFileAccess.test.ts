import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { assertPathWithinRoots } from '../rendererFileAccess';

const temporaryDirectories: string[] = [];

async function temporaryDirectory(): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-renderer-access-'));
  temporaryDirectories.push(directory);
  return directory;
}

afterEach(async () => {
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { recursive: true, force: true }))
  );
});

describe('assertPathWithinRoots', () => {
  it('allows an existing file and a missing descendant inside an approved root', async () => {
    const root = await temporaryDirectory();
    const existing = path.join(root, 'existing.txt');
    await fs.writeFile(existing, 'safe');

    await expect(assertPathWithinRoots(existing, [root])).resolves.toBe(
      await fs.realpath(existing)
    );
    await expect(assertPathWithinRoots(path.join(root, 'new', 'file.txt'), [root])).resolves.toBe(
      path.join(await fs.realpath(root), 'new', 'file.txt')
    );
    await expect(assertPathWithinRoots(path.join(root, '..named-file'), [root])).resolves.toBe(
      path.join(await fs.realpath(root), '..named-file')
    );
  });

  it.skipIf(process.platform === 'win32')(
    'rejects existing and missing paths redirected through a symbolic link',
    async () => {
      const root = await temporaryDirectory();
      const outside = await temporaryDirectory();
      const outsideFile = path.join(outside, 'secret.txt');
      await fs.writeFile(outsideFile, 'secret');
      await fs.symlink(outside, path.join(root, 'redirect'));

      await expect(
        assertPathWithinRoots(path.join(root, 'redirect', 'secret.txt'), [root])
      ).rejects.toThrow('outside approved roots');
      await expect(
        assertPathWithinRoots(path.join(root, 'redirect', 'new.txt'), [root])
      ).rejects.toThrow('outside approved roots');
    }
  );

  it.skipIf(process.platform === 'win32')('rejects a dangling symbolic-link ancestor', async () => {
    const root = await temporaryDirectory();
    await fs.symlink(path.join(root, 'missing-target'), path.join(root, 'dangling'));

    await expect(
      assertPathWithinRoots(path.join(root, 'dangling', 'new.txt'), [root])
    ).rejects.toThrow('dangling symbolic link');
  });
});
