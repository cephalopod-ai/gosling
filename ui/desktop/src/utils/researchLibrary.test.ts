import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
  defaultResearchLibraryPath,
  listResearchLibraryFiles,
  RESEARCH_LIBRARY_FOLDER_NAME,
} from './researchLibrary';

const temporaryDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((directory) => fs.rm(directory, { recursive: true }))
  );
});

describe('research library', () => {
  it('defaults to a named folder inside Documents', () => {
    expect(defaultResearchLibraryPath('/Users/tester/Documents')).toBe(
      `/Users/tester/Documents/${RESEARCH_LIBRARY_FOLDER_NAME}`
    );
  });

  it('lists bounded document files recursively without following hidden or symlinked entries', async () => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-research-library-'));
    temporaryDirectories.push(root);
    await fs.mkdir(path.join(root, 'bayesian-networks'));
    await fs.writeFile(path.join(root, 'bayesian-networks', 'report.md'), '# Report');
    await fs.writeFile(path.join(root, 'notes.txt'), 'Notes');
    await fs.writeFile(path.join(root, 'model.py'), 'print(1)');
    await fs.writeFile(path.join(root, '.private.md'), 'hidden');
    await fs.symlink(path.join(root, 'bayesian-networks'), path.join(root, 'linked'));

    const files = await listResearchLibraryFiles(root, ['md', 'txt']);

    expect(files.map((file) => file.relativePath).sort()).toEqual([
      'bayesian-networks/report.md',
      'notes.txt',
    ]);
  });
});
