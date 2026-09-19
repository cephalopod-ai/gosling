// @vitest-environment node
import { spawn, type ChildProcess } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanupRecordedBackendProcesses, registerBackendProcess } from './backendProcessRegistry';

const temporaryDirectories: string[] = [];
const spawnedProcesses: ChildProcess[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  for (const child of spawnedProcesses.splice(0)) {
    if (child.pid && !child.killed) child.kill('SIGKILL');
  }
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { recursive: true, force: true }))
  );
});

// A real, controllable process rather than a mocked one: cleanupRecordedBackendProcesses
// decides liveness with process.kill(pid, 0) against actual OS state, which a fake pid
// number cannot exercise honestly.
function spawnSleeper(): ChildProcess {
  const child = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)']);
  spawnedProcesses.push(child);
  return child;
}

function waitForExit(child: ChildProcess): Promise<void> {
  return new Promise((resolve) => child.once('exit', () => resolve()));
}

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

function isAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

async function pollUntil(predicate: () => boolean, timeoutMs = 3000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return true;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  return predicate();
}

const silentLogger = { info: () => {}, error: () => {} };

// The command line has to look like a real `gosling serve --platform desktop`
// invocation, since isRecordedGoslingServeProcess shells out to the real `ps`
// rather than trusting the registry's own recorded args.
const GOSLING_SERVE_SHAPE_ARGS = [
  '-e',
  'setInterval(() => {}, 1000)',
  '--',
  'serve',
  '--platform',
  'desktop',
];

describe.skipIf(process.platform === 'win32')('cleanupRecordedBackendProcesses', () => {
  it('keeps a record whose spawning process is still alive, live or in the registry', async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-process-registry-'));
    temporaryDirectories.push(directory);
    const registryPath = path.join(directory, 'backend-processes.json');

    const owner = spawnSleeper();
    const backend = spawn(process.execPath, GOSLING_SERVE_SHAPE_ARGS);
    spawnedProcesses.push(backend);
    await pollUntil(() => isAlive(owner.pid!) && isAlive(backend.pid!));

    const record = {
      pid: backend.pid!,
      parentPid: owner.pid!,
      binaryPath: process.execPath,
      args: ['serve', '--platform', 'desktop'],
      workingDir: directory,
      startedAt: new Date().toISOString(),
    };
    await fs.writeFile(registryPath, JSON.stringify({ version: 1, processes: [record] }));

    // This is the regression the incident produced: a record matching the
    // gosling-serve command-line shape whose owning process (a different,
    // still-running Gosling instance) had not exited was killed anyway.
    await cleanupRecordedBackendProcesses(registryPath, silentLogger);

    expect(isAlive(backend.pid!)).toBe(true);
    expect(JSON.parse(await fs.readFile(registryPath, 'utf8'))).toEqual({
      version: 1,
      processes: [record],
    });
  });

  it('terminates and drops a record whose spawning process has actually exited', async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-process-registry-'));
    temporaryDirectories.push(directory);
    const registryPath = path.join(directory, 'backend-processes.json');

    const owner = spawn(process.execPath, ['-e', 'process.exit(0)']);
    spawnedProcesses.push(owner);
    await waitForExit(owner);
    expect(isAlive(owner.pid!)).toBe(false);

    const backend = spawn(process.execPath, GOSLING_SERVE_SHAPE_ARGS);
    spawnedProcesses.push(backend);
    await pollUntil(() => isAlive(backend.pid!));

    const record = {
      pid: backend.pid!,
      parentPid: owner.pid!,
      binaryPath: process.execPath,
      args: ['serve', '--platform', 'desktop'],
      workingDir: directory,
      startedAt: new Date().toISOString(),
    };
    await fs.writeFile(registryPath, JSON.stringify({ version: 1, processes: [record] }));

    await cleanupRecordedBackendProcesses(registryPath, silentLogger);

    expect(await pollUntil(() => !isAlive(backend.pid!))).toBe(true);
    expect(JSON.parse(await fs.readFile(registryPath, 'utf8'))).toEqual({
      version: 1,
      processes: [],
    });
  });
});
