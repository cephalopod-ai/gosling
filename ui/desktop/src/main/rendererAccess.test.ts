import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { RendererAccessController } from './rendererAccess';

const temporaryDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { force: true, recursive: true }))
  );
});

async function createController(): Promise<{
  controller: RendererAccessController;
  grantedDirectory: string;
}> {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-renderer-access-'));
  temporaryDirectories.push(root);
  const grantedDirectory = path.join(root, 'granted');
  await fs.mkdir(grantedDirectory);
  return {
    controller: new RendererAccessController(path.join(root, 'grants.json')),
    grantedDirectory,
  };
}

describe('RendererAccessController', () => {
  it('authorizes files only beneath a renderer grant', async () => {
    const { controller, grantedDirectory } = await createController();
    const filePath = path.join(grantedDirectory, 'result.txt');
    await fs.writeFile(filePath, 'result');

    controller.grantSelectedPath(7, grantedDirectory, false);

    await expect(controller.assertFileAccess(7, filePath)).resolves.toBe(
      await fs.realpath(filePath)
    );
    await expect(controller.assertFileAccess(8, filePath)).rejects.toThrow();
  });

  it('clears transient grants with the owning web contents', async () => {
    const { controller, grantedDirectory } = await createController();
    controller.grantSelectedPath(7, grantedDirectory, false);

    controller.clearWebContents(7);

    expect(controller.isGrantedDirectory(7, grantedDirectory)).toBe(false);
  });
});
