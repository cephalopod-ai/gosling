import { spawn } from 'child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { Logger } from './goslingServe';

export interface BackendProcessRecord {
  pid: number;
  parentPid: number;
  binaryPath: string;
  args: string[];
  workingDir: string;
  startedAt: string;
}

interface BackendProcessRegistryFile {
  version: 1;
  processes: BackendProcessRecord[];
}

const TERMINATE_GRACE_MS = 3000;
const POLL_INTERVAL_MS = 100;
const registryUpdateQueues = new Map<string, Promise<void>>();

const isBackendProcessRecord = (value: unknown): value is BackendProcessRecord => {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const record = value as Partial<BackendProcessRecord>;
  return (
    typeof record.pid === 'number' &&
    Number.isInteger(record.pid) &&
    record.pid > 0 &&
    typeof record.parentPid === 'number' &&
    typeof record.binaryPath === 'string' &&
    Array.isArray(record.args) &&
    record.args.every((arg) => typeof arg === 'string') &&
    typeof record.workingDir === 'string' &&
    typeof record.startedAt === 'string'
  );
};

const delay = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

const readRecords = async (registryPath: string): Promise<BackendProcessRecord[]> => {
  let contents: string;
  try {
    contents = await fs.readFile(registryPath, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return [];
    }
    throw error;
  }

  const parsed = JSON.parse(contents) as BackendProcessRegistryFile | BackendProcessRecord[];
  const records = Array.isArray(parsed) ? parsed : parsed.processes;
  if (!Array.isArray(records)) {
    return [];
  }
  return records.filter(isBackendProcessRecord);
};

const writeRecords = async (
  registryPath: string,
  records: BackendProcessRecord[]
): Promise<void> => {
  await fs.mkdir(path.dirname(registryPath), { recursive: true });
  const file: BackendProcessRegistryFile = {
    version: 1,
    processes: records,
  };
  await fs.writeFile(registryPath, `${JSON.stringify(file, null, 2)}\n`, 'utf8');
};

const updateRecords = async (
  registryPath: string,
  update: (records: BackendProcessRecord[]) => BackendProcessRecord[]
): Promise<void> => {
  const previousUpdate = registryUpdateQueues.get(registryPath) ?? Promise.resolve();
  const nextUpdate = previousUpdate
    .catch(() => undefined)
    .then(async () => {
      const records = await readRecords(registryPath);
      await writeRecords(registryPath, update(records));
    });

  registryUpdateQueues.set(registryPath, nextUpdate);
  try {
    await nextUpdate;
  } finally {
    if (registryUpdateQueues.get(registryPath) === nextUpdate) {
      registryUpdateQueues.delete(registryPath);
    }
  }
};

export const registerBackendProcess = async (
  registryPath: string,
  record: BackendProcessRecord
): Promise<void> => {
  await updateRecords(registryPath, (records) => {
    const remaining = records.filter((existing) => existing.pid !== record.pid);
    return [...remaining, record];
  });
};

export const unregisterBackendProcess = async (
  registryPath: string,
  pid: number
): Promise<void> => {
  await updateRecords(registryPath, (records) => records.filter((record) => record.pid !== pid));
};

const isProcessRunning = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === 'EPERM';
  }
};

const commandLineForPid = (pid: number): Promise<string | null> => {
  if (process.platform === 'win32') {
    return Promise.resolve(null);
  }

  return new Promise((resolve) => {
    const child = spawn('ps', ['-p', String(pid), '-o', 'command='], {
      stdio: ['ignore', 'pipe', 'ignore'],
    });
    let stdout = '';
    child.stdout?.on('data', (data) => {
      stdout += data.toString();
    });
    child.on('error', () => resolve(null));
    child.on('close', (code) => {
      resolve(code === 0 ? stdout.trim() : null);
    });
  });
};

const isRecordedGoslingServeProcess = async (record: BackendProcessRecord): Promise<boolean> => {
  if (!isProcessRunning(record.pid)) {
    return false;
  }

  if (process.platform === 'win32') {
    return record.args[0] === 'serve' && record.args.includes('desktop');
  }

  const commandLine = await commandLineForPid(record.pid);
  if (!commandLine) {
    return false;
  }

  return (
    commandLine.includes(path.basename(record.binaryPath)) &&
    commandLine.includes(' serve') &&
    commandLine.includes('--platform') &&
    commandLine.includes('desktop')
  );
};

const waitForProcessExit = async (pid: number, timeoutMs: number): Promise<boolean> => {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (!isProcessRunning(pid)) {
      return true;
    }
    await delay(POLL_INTERVAL_MS);
  }
  return !isProcessRunning(pid);
};

const terminateProcess = async (pid: number): Promise<void> => {
  if (!isProcessRunning(pid)) {
    return;
  }

  if (process.platform === 'win32') {
    await new Promise<void>((resolve) => {
      const taskkill = spawn('taskkill', ['/pid', String(pid), '/f', '/t'], {
        stdio: 'ignore',
      });
      taskkill.on('error', () => resolve());
      taskkill.on('close', () => resolve());
    });
    return;
  }

  try {
    process.kill(pid, 'SIGTERM');
  } catch {
    return;
  }

  if (await waitForProcessExit(pid, TERMINATE_GRACE_MS)) {
    return;
  }

  try {
    process.kill(pid, 'SIGKILL');
  } catch {
    return;
  }
  await waitForProcessExit(pid, TERMINATE_GRACE_MS);
};

export const cleanupRecordedBackendProcesses = async (
  registryPath: string,
  logger: Logger
): Promise<void> => {
  const records = await readRecords(registryPath);
  const stillRunning: BackendProcessRecord[] = [];

  for (const record of records) {
    if (!(await isRecordedGoslingServeProcess(record))) {
      continue;
    }

    logger.info(`Cleaning up stale gosling serve process ${record.pid}`);
    await terminateProcess(record.pid);
    if (isProcessRunning(record.pid)) {
      logger.error(`Stale gosling serve process ${record.pid} is still running after cleanup`);
      stillRunning.push(record);
    }
  }

  await writeRecords(registryPath, stillRunning);
};
