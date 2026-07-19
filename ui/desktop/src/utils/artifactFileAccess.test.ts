import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { assertArtifactFileAccess } from './artifactFileAccess';

const temporaryDirectories: string[] = [];

async function temporaryDirectory(): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-artifact-access-'));
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

describe('assertArtifactFileAccess', () => {
  it('allows a file inside a validated workspace output root', async () => {
    const approvedRoot = await temporaryDirectory();
    const outputRoot = await temporaryDirectory();
    const filePath = path.join(outputRoot, 'report.md');
    await fs.writeFile(filePath, '# report');

    await expect(
      assertArtifactFileAccess(filePath, undefined, [approvedRoot], [outputRoot], new Set())
    ).resolves.toBe(await fs.realpath(filePath));
  });

  it('continues to allow a file selected through a direct file grant', async () => {
    const approvedRoot = await temporaryDirectory();
    const selectedRoot = await temporaryDirectory();
    const filePath = path.join(selectedRoot, 'selected.txt');
    await fs.writeFile(filePath, 'selected');
    const canonicalPath = await fs.realpath(filePath);

    await expect(
      assertArtifactFileAccess(filePath, undefined, [approvedRoot], [], new Set([canonicalPath]))
    ).resolves.toBe(canonicalPath);
  });

  it('rejects a file outside both renderer grants and workspace outputs', async () => {
    const approvedRoot = await temporaryDirectory();
    const outputRoot = await temporaryDirectory();
    const outsideRoot = await temporaryDirectory();
    const filePath = path.join(outsideRoot, 'secret.txt');
    await fs.writeFile(filePath, 'secret');

    await expect(
      assertArtifactFileAccess(filePath, undefined, [approvedRoot], [outputRoot], new Set())
    ).rejects.toThrow('outside approved roots');
  });
});
