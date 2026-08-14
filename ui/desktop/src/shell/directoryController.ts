import path from 'node:path';
import type { ShellDirectoryValidateResponse_unstable } from '@repo-makeover/gosling-sdk';
import type { ShellSettingsStore } from './localSettings';

const MAX_DIRECTORY_PATH_LENGTH = 4096;
const MAX_DIRECTORY_LABEL_LENGTH = 128;

export type ShellDirectoryState = 'unselected' | 'selected' | 'missing' | 'invalid';

export interface ShellDirectorySnapshot {
  state: ShellDirectoryState;
  path: string | null;
  label: string | null;
  reasonCode: string | null;
  remembered: boolean;
}

export type ShellDirectorySelectResult =
  | { status: 'cancelled'; directory: ShellDirectorySnapshot }
  | { status: 'selected'; directory: ShellDirectorySnapshot }
  | { status: 'rejected'; directory: ShellDirectorySnapshot; reasonCode: string };

export interface ShellDirectoryController {
  read(): ShellDirectorySnapshot;
  accepted(): string | null;
  restore(): Promise<ShellDirectorySnapshot>;
  select(generation: number): Promise<ShellDirectorySelectResult>;
  clear(): void;
  onChanged(listener: (directory: ShellDirectorySnapshot) => void): () => void;
}

export interface ShellDirectoryDependencies {
  settings: ShellSettingsStore;
  showOpenDialog(options: {
    title: string;
    buttonLabel: string;
    message: string;
  }): Promise<{ canceled: boolean; filePaths: string[] }>;
  validate(directory: string): Promise<ShellDirectoryValidateResponse_unstable>;
  generation(): number;
  isSelectable(): { allowed: boolean; reasonCode?: string };
}

function unselected(): ShellDirectorySnapshot {
  return { state: 'unselected', path: null, label: null, reasonCode: null, remembered: false };
}

function label(directory: string): string {
  const basename = path.basename(directory) || directory;
  return basename.slice(0, MAX_DIRECTORY_LABEL_LENGTH);
}

function selected(directory: string): ShellDirectorySnapshot {
  return {
    state: 'selected',
    path: directory,
    label: label(directory),
    reasonCode: null,
    remembered: true,
  };
}

function unusable(state: 'missing' | 'invalid', reasonCode: string): ShellDirectorySnapshot {
  return { state, path: null, label: null, reasonCode, remembered: false };
}

function isAcceptablePath(value: unknown): value is string {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    value.length <= MAX_DIRECTORY_PATH_LENGTH &&
    !value.includes('\0') &&
    path.isAbsolute(value)
  );
}

function projectValidation(
  response: ShellDirectoryValidateResponse_unstable
): ShellDirectorySnapshot {
  if (response.status === 'valid' && isAcceptablePath(response.canonicalPath)) {
    return selected(response.canonicalPath);
  }
  const reasonCode = typeof response.reason === 'string' ? response.reason : 'invalid_path';
  return unusable(reasonCode === 'not_found' ? 'missing' : 'invalid', reasonCode);
}

/// Owns the working directory for one shell process.
///
/// The renderer never supplies a path: it asks for a native chooser, main sends the operator's
/// confirmed path to the backend for canonicalization, and only the accepted canonical path is kept
/// or persisted.
export function createShellDirectoryController(
  dependencies: ShellDirectoryDependencies
): ShellDirectoryController {
  let directory = unselected();
  let outstanding: Promise<ShellDirectorySelectResult> | null = null;
  const listeners = new Set<(directory: ShellDirectorySnapshot) => void>();

  const publish = (next: ShellDirectorySnapshot) => {
    directory = next;
    for (const listener of listeners) listener({ ...directory });
    return { ...directory };
  };

  const accept = async (
    candidate: string,
    generation: number
  ): Promise<ShellDirectorySnapshot> => {
    const validated = projectValidation(await dependencies.validate(candidate));
    // Validation is a round trip: the runtime may have torn down or retried while it was pending,
    // and a stale acceptance must not persist or reappear in a newer generation.
    if (generation !== dependencies.generation()) {
      throw new Error('directory selection generation is stale');
    }
    if (validated.state === 'selected' && validated.path) {
      // A settings document this build refuses to overwrite must not make the shell unusable:
      // the selection still applies to this run, it just is not remembered. The recovery status
      // travels in the runtime snapshot so the operator is told why.
      try {
        dependencies.settings.setLastWorkingDirectory(validated.path);
      } catch {
        return { ...validated, remembered: false };
      }
    }
    return validated;
  };

  return {
    read: () => ({ ...directory }),
    accepted: () => (directory.state === 'selected' ? directory.path : null),
    async restore() {
      const generation = dependencies.generation();
      const remembered = dependencies.settings.read().workspace.lastWorkingDirectory;
      if (!isAcceptablePath(remembered)) {
        return publish(unselected());
      }
      const validated = projectValidation(await dependencies.validate(remembered));
      if (generation !== dependencies.generation()) {
        return { ...directory };
      }
      return publish(validated);
    },
    select(generation) {
      if (outstanding) {
        return Promise.reject(new Error('a directory selection is already outstanding'));
      }
      if (generation !== dependencies.generation()) {
        return Promise.reject(new Error('directory selection generation is stale'));
      }
      const selectable = dependencies.isSelectable();
      if (!selectable.allowed) {
        return Promise.reject(
          new Error(selectable.reasonCode ?? 'directory selection is unavailable')
        );
      }
      outstanding = (async (): Promise<ShellDirectorySelectResult> => {
        const chosen = await dependencies.showOpenDialog({
          title: 'Choose a working directory',
          buttonLabel: 'Use this folder',
          message: 'This shell reads and writes only inside the folder you choose.',
        });
        if (generation !== dependencies.generation()) {
          throw new Error('directory selection generation is stale');
        }
        if (chosen.canceled || chosen.filePaths.length !== 1) {
          return { status: 'cancelled', directory: { ...directory } };
        }
        const [candidate] = chosen.filePaths;
        if (!isAcceptablePath(candidate)) {
          return {
            status: 'rejected',
            directory: { ...directory },
            reasonCode: 'invalid_path',
          };
        }
        const validated = await accept(candidate, generation);
        if (validated.state !== 'selected') {
          return {
            status: 'rejected',
            directory: { ...directory },
            reasonCode: validated.reasonCode ?? 'invalid_path',
          };
        }
        return { status: 'selected', directory: publish(validated) };
      })().finally(() => {
        outstanding = null;
      });
      return outstanding;
    },
    clear() {
      publish(unselected());
    },
    onChanged(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
