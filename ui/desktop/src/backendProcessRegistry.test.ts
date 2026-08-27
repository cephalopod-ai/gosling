import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { registerBackendProcess } from './backendProcessRegistry';

const temporaryDirectories: string[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { recursive: true, force: true }))
  );
});

const processRecord = {
  pid: 42,
  parentPid: 1,
  binaryPath: '/tmp/gosling',
  args: ['serve', '--platform', 'desktop'],
  workingDir: '/tmp',
  startedAt: '2026-08-27T00:00:00.000Z',
};

describe('backend process registry persistence', () => {
  it('publishes a complete registry through a same-directory rename', async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-process-registry-'));
    temporaryDirectories.push(directory);
    const registryPath = path.join(directory, 'backend-processes.json');
    const rename = vi.spyOn(fs, 'rename');

    await registerBackendProcess(registryPath, processRecord);

    expect(rename).toHaveBeenCalledOnce();
    const [temporaryPath, destinationPath] = rename.mock.calls[0];
    expect(path.dirname(temporaryPath.toString())).toBe(directory);
    expect(destinationPath).toBe(registryPath);
    expect(JSON.parse(await fs.readFile(registryPath, 'utf8'))).toEqual({
      version: 1,
      processes: [processRecord],
    });
    expect((await fs.readdir(directory)).some((name) => name.endsWith('.tmp'))).toBe(false);
  });

  it('removes the temporary registry when publication fails', async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-process-registry-'));
    temporaryDirectories.push(directory);
    const registryPath = path.join(directory, 'backend-processes.json');
    vi.spyOn(fs, 'rename').mockRejectedValueOnce(new Error('simulated rename failure'));

    await expect(registerBackendProcess(registryPath, processRecord)).rejects.toThrow(
      'simulated rename failure'
    );

    expect((await fs.readdir(directory)).some((name) => name.endsWith('.tmp'))).toBe(false);
  });
});
