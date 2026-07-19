import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

export interface AtomicJsonWriteOptions {
  preservePrevious?: boolean;
}

export interface AtomicJsonReadResult<T> {
  value: T;
  recoveredFromPrevious: boolean;
  corruptFilePath?: string;
}

function syncDirectory(directoryPath: string): void {
  let descriptor: number | undefined;
  try {
    descriptor = fs.openSync(directoryPath, 'r');
    fs.fsyncSync(descriptor);
  } catch {
    // Some platforms do not support fsync on directory handles.
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }
}

function writeSerializedJsonAtomicSync(filePath: string, serialized: string): void {
  const directoryPath = path.dirname(filePath);
  fs.mkdirSync(directoryPath, { recursive: true, mode: 0o700 });
  fs.chmodSync(directoryPath, 0o700);
  const temporaryPath = path.join(directoryPath, `.${path.basename(filePath)}.${randomUUID()}.tmp`);
  let descriptor: number | undefined;

  try {
    descriptor = fs.openSync(temporaryPath, 'wx', 0o600);
    fs.writeFileSync(descriptor, serialized, 'utf8');
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = undefined;
    fs.renameSync(temporaryPath, filePath);
    fs.chmodSync(filePath, 0o600);
    syncDirectory(directoryPath);
  } catch (error) {
    if (descriptor !== undefined) fs.closeSync(descriptor);
    try {
      fs.unlinkSync(temporaryPath);
    } catch {
      // The temp file may already have been renamed or never created.
    }
    throw error;
  }
}

function quarantineCorruptFile(filePath: string): string {
  const corruptFilePath = `${filePath}.corrupt-${Date.now()}-${randomUUID()}`;
  fs.renameSync(filePath, corruptFilePath);
  fs.chmodSync(corruptFilePath, 0o600);
  return corruptFilePath;
}

export function writeJsonFileAtomicSync(
  filePath: string,
  value: unknown,
  options: AtomicJsonWriteOptions = {}
): void {
  const serialized = `${JSON.stringify(value, null, 2)}\n`;
  const preservePrevious = options.preservePrevious ?? true;

  if (preservePrevious && fs.existsSync(filePath)) {
    try {
      const existing = fs.readFileSync(filePath, 'utf8');
      JSON.parse(existing);
      writeSerializedJsonAtomicSync(`${filePath}.previous`, existing);
    } catch {
      // Never replace a known-good previous snapshot with malformed data.
    }
  }

  writeSerializedJsonAtomicSync(filePath, serialized);
}

export function readJsonFileWithRecoverySync<T>(
  filePath: string,
  validate: (value: unknown) => value is T
): AtomicJsonReadResult<T> | null {
  if (!fs.existsSync(filePath)) return null;

  let currentError: unknown;
  try {
    const value: unknown = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    if (!validate(value)) throw new Error('JSON data failed schema validation');
    fs.chmodSync(filePath, 0o600);
    return { value, recoveredFromPrevious: false };
  } catch (error) {
    currentError = error;
  }

  const previousPath = `${filePath}.previous`;
  if (!fs.existsSync(previousPath)) {
    quarantineCorruptFile(filePath);
    throw currentError;
  }

  let previousValue: unknown;
  try {
    previousValue = JSON.parse(fs.readFileSync(previousPath, 'utf8'));
    if (!validate(previousValue)) throw new Error('Previous JSON data failed schema validation');
  } catch {
    quarantineCorruptFile(filePath);
    throw currentError;
  }

  const corruptFilePath = quarantineCorruptFile(filePath);
  writeJsonFileAtomicSync(filePath, previousValue, { preservePrevious: false });
  return {
    value: previousValue,
    recoveredFromPrevious: true,
    corruptFilePath,
  };
}
