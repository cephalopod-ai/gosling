import fs from 'node:fs';
import path from 'node:path';

const MAX_SETTINGS_BYTES = 16 * 1024;
const MAX_PATH_LENGTH = 4096;
const MAX_REFERENCE_LENGTH = 128;

export type ShellTheme = 'system' | 'light' | 'dark';

export interface ShellLocalSettings {
  schemaVersion: 1;
  appearance: {
    theme: ShellTheme;
    textScale: number;
  };
  workspace: {
    lastWorkingDirectory: string | null;
    preferredCredentialProfileId: string | null;
  };
}

export const defaultShellLocalSettings = (): ShellLocalSettings => ({
  schemaVersion: 1,
  appearance: {
    theme: 'system',
    textScale: 1,
  },
  workspace: {
    lastWorkingDirectory: null,
    preferredCredentialProfileId: null,
  },
});

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function isNullableBoundedString(value: unknown, maximum: number): value is string | null {
  return (
    value === null || (typeof value === 'string' && value.length > 0 && value.length <= maximum)
  );
}

export function parseShellLocalSettings(value: unknown): ShellLocalSettings {
  if (!isRecord(value) || !hasExactKeys(value, ['schemaVersion', 'appearance', 'workspace'])) {
    throw new Error('shell settings must contain only schemaVersion, appearance, and workspace');
  }
  if (value.schemaVersion !== 1 || !isRecord(value.appearance) || !isRecord(value.workspace)) {
    throw new Error('unsupported shell settings schema');
  }
  if (!hasExactKeys(value.appearance, ['theme', 'textScale'])) {
    throw new Error('invalid shell appearance settings');
  }
  if (
    !['system', 'light', 'dark'].includes(value.appearance.theme as string) ||
    typeof value.appearance.textScale !== 'number' ||
    !Number.isFinite(value.appearance.textScale) ||
    value.appearance.textScale < 0.8 ||
    value.appearance.textScale > 2
  ) {
    throw new Error('invalid shell appearance settings');
  }
  if (!hasExactKeys(value.workspace, ['lastWorkingDirectory', 'preferredCredentialProfileId'])) {
    throw new Error('invalid shell workspace settings');
  }
  if (
    !isNullableBoundedString(value.workspace.lastWorkingDirectory, MAX_PATH_LENGTH) ||
    (typeof value.workspace.lastWorkingDirectory === 'string' &&
      !path.isAbsolute(value.workspace.lastWorkingDirectory)) ||
    !isNullableBoundedString(value.workspace.preferredCredentialProfileId, MAX_REFERENCE_LENGTH)
  ) {
    throw new Error('invalid shell workspace settings');
  }
  return value as unknown as ShellLocalSettings;
}

export function readShellLocalSettings(file: string): ShellLocalSettings {
  let serialized: string;
  try {
    const stats = fs.lstatSync(file);
    if (!stats.isFile() || stats.isSymbolicLink()) {
      throw new Error('shell settings path must be a regular file');
    }
    if (stats.size > MAX_SETTINGS_BYTES) {
      throw new Error('shell settings exceed the 16 KiB limit');
    }
    serialized = fs.readFileSync(file, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return defaultShellLocalSettings();
    }
    throw error;
  }
  if (Buffer.byteLength(serialized, 'utf8') > MAX_SETTINGS_BYTES) {
    throw new Error('shell settings exceed the 16 KiB limit');
  }
  try {
    return parseShellLocalSettings(JSON.parse(serialized) as unknown);
  } catch (error) {
    if (error instanceof SyntaxError) {
      throw new Error('shell settings are not valid JSON');
    }
    throw error;
  }
}

export function writeShellLocalSettings(file: string, settings: ShellLocalSettings): void {
  const validated = parseShellLocalSettings(settings);
  const serialized = `${JSON.stringify(validated, null, 2)}\n`;
  if (Buffer.byteLength(serialized, 'utf8') > MAX_SETTINGS_BYTES) {
    throw new Error('shell settings exceed the 16 KiB limit');
  }

  const directory = path.dirname(file);
  fs.mkdirSync(directory, { recursive: true, mode: 0o700 });
  const temporary = path.join(directory, `.${path.basename(file)}.${process.pid}.tmp`);
  let descriptor: number | null = null;
  try {
    descriptor = fs.openSync(temporary, 'wx', 0o600);
    fs.writeFileSync(descriptor, serialized, 'utf8');
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = null;
    fs.renameSync(temporary, file);
    fs.chmodSync(file, 0o600);
  } catch (error) {
    if (descriptor !== null) {
      fs.closeSync(descriptor);
    }
    fs.rmSync(temporary, { force: true });
    throw error;
  }
}
