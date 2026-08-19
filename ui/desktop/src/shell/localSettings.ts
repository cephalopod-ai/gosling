import fs from 'node:fs';
import path from 'node:path';

const MAX_SETTINGS_BYTES = 16 * 1024;
const MAX_PATH_LENGTH = 4096;
const MAX_REFERENCE_LENGTH = 128;

export {
  SHELL_SETTINGS_SCHEMA_VERSION,
  SHELL_THEME_VALUES,
  MIN_SHELL_TEXT_SCALE,
  MAX_SHELL_TEXT_SCALE,
  isValidShellTheme,
  isValidShellTextScale,
} from './settingsSchema';
export type { ShellTheme } from './settingsSchema';

import {
  SHELL_SETTINGS_SCHEMA_VERSION,
  isValidShellTextScale,
  isValidShellTheme,
} from './settingsSchema';
import type { ShellTheme } from './settingsSchema';

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
    !isValidShellTheme(value.appearance.theme) ||
    !isValidShellTextScale(value.appearance.textScale)
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

export type ShellSettingsRecoveryStatus =
  | 'loaded'
  | 'absent'
  | 'unsupported_schema'
  | 'malformed'
  | 'unreadable';

export interface ShellSettingsRecovery {
  status: ShellSettingsRecoveryStatus;
  schemaVersion: number | null;
}

export interface ShellLocalSettingsLoad {
  settings: ShellLocalSettings;
  recovery: ShellSettingsRecovery;
}

function declaredSchemaVersion(serialized: string): number | null {
  try {
    const parsed: unknown = JSON.parse(serialized);
    if (isRecord(parsed) && typeof parsed.schemaVersion === 'number') {
      return parsed.schemaVersion;
    }
  } catch {
    return null;
  }
  return null;
}

/// Loads the product-local document without ever repairing it in place.
///
/// A document this build cannot understand keeps its bytes and surfaces a recovery status, so an
/// operator can downgrade or export before anything overwrites it.
export function loadShellLocalSettings(file: string): ShellLocalSettingsLoad {
  let serialized: string;
  try {
    const stats = fs.lstatSync(file);
    if (!stats.isFile() || stats.isSymbolicLink() || stats.size > MAX_SETTINGS_BYTES) {
      return { settings: defaultShellLocalSettings(), recovery: recovery('malformed', null) };
    }
    serialized = fs.readFileSync(file, 'utf8');
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code;
    return {
      settings: defaultShellLocalSettings(),
      recovery: recovery(code === 'ENOENT' ? 'absent' : 'unreadable', null),
    };
  }
  if (Buffer.byteLength(serialized, 'utf8') > MAX_SETTINGS_BYTES) {
    return { settings: defaultShellLocalSettings(), recovery: recovery('malformed', null) };
  }
  const schemaVersion = declaredSchemaVersion(serialized);
  try {
    return {
      settings: parseShellLocalSettings(JSON.parse(serialized) as unknown),
      recovery: recovery('loaded', SHELL_SETTINGS_SCHEMA_VERSION),
    };
  } catch {
    return {
      settings: defaultShellLocalSettings(),
      recovery: recovery(
        schemaVersion !== null && schemaVersion !== SHELL_SETTINGS_SCHEMA_VERSION
          ? 'unsupported_schema'
          : 'malformed',
        schemaVersion
      ),
    };
  }
}

function recovery(status: ShellSettingsRecoveryStatus, schemaVersion: number | null) {
  return { status, schemaVersion };
}

export interface ShellSettingsStore {
  read(): ShellLocalSettings;
  recovery(): ShellSettingsRecovery;
  setAppearance(input: { theme?: ShellTheme; textScale?: number }): ShellLocalSettings;
  setLastWorkingDirectory(directory: string | null): ShellLocalSettings;
  setPreferredCredentialProfileId(profileId: string | null): ShellLocalSettings;
  reset(): ShellLocalSettings;
}

function copySettings(settings: ShellLocalSettings): ShellLocalSettings {
  return {
    schemaVersion: settings.schemaVersion,
    appearance: { ...settings.appearance },
    workspace: { ...settings.workspace },
  };
}

/// The only writer of the shell-local document.
///
/// It exposes one operation per allowlisted field rather than an arbitrary key/path setter, and
/// refuses to write over a document it could not parse until `reset` is called explicitly.
export function createShellSettingsStore(file: string): ShellSettingsStore {
  const loaded = loadShellLocalSettings(file);
  let settings = loaded.settings;
  let status = loaded.recovery;

  const commit = (next: ShellLocalSettings): ShellLocalSettings => {
    if (status.status !== 'loaded' && status.status !== 'absent') {
      throw new Error('shell settings are in a recovery state and were not overwritten');
    }
    writeShellLocalSettings(file, next);
    settings = next;
    status = recovery('loaded', SHELL_SETTINGS_SCHEMA_VERSION);
    return copySettings(settings);
  };

  return {
    read: () => copySettings(settings),
    recovery: () => ({ ...status }),
    setAppearance({ theme, textScale }) {
      const next = copySettings(settings);
      if (theme !== undefined) next.appearance.theme = theme;
      if (textScale !== undefined) next.appearance.textScale = textScale;
      return commit(next);
    },
    setLastWorkingDirectory(directory) {
      const next = copySettings(settings);
      next.workspace.lastWorkingDirectory = directory;
      return commit(next);
    },
    setPreferredCredentialProfileId(profileId) {
      const next = copySettings(settings);
      next.workspace.preferredCredentialProfileId = profileId;
      return commit(next);
    },
    reset() {
      status = recovery('absent', null);
      return commit(defaultShellLocalSettings());
    },
  };
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
