import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { readBoundedSessionImportFile } from './sessionImport';

const tempDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    tempDirectories.splice(0).map((directory) => fs.rm(directory, { recursive: true, force: true }))
  );
});

async function fixture(contents: Buffer | string): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-session-import-'));
  tempDirectories.push(directory);
  const filePath = path.join(directory, 'session.json');
  await fs.writeFile(filePath, contents);
  return filePath;
}

describe('readBoundedSessionImportFile', () => {
  it('reads valid UTF-8 within the byte budget', async () => {
    const filePath = await fixture('{"ok":true}');
    await expect(readBoundedSessionImportFile(filePath, 32)).resolves.toBe('{"ok":true}');
  });

  it('rejects a file that is one byte over the byte budget', async () => {
    const filePath = await fixture('12345');
    await expect(readBoundedSessionImportFile(filePath, 4)).rejects.toThrow('exceeds');
  });

  it('rejects invalid UTF-8', async () => {
    const filePath = await fixture(Buffer.from([0xff]));
    await expect(readBoundedSessionImportFile(filePath, 4)).rejects.toThrow();
  });
});
