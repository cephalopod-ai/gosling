import { describe, expect, it, vi } from 'vitest';
import type { ShellDirectoryValidateResponse_unstable } from '@repo-makeover/gosling-sdk';
import { createShellDirectoryController } from './directoryController';
import {
  defaultShellLocalSettings,
  type ShellLocalSettings,
  type ShellSettingsStore,
} from './localSettings';

function copySettings(settings: ShellLocalSettings): ShellLocalSettings {
  return {
    schemaVersion: settings.schemaVersion,
    appearance: { ...settings.appearance },
    workspace: { ...settings.workspace },
  };
}

function memorySettingsStore(lastWorkingDirectory: string | null): ShellSettingsStore {
  let settings = {
    ...defaultShellLocalSettings(),
    workspace: { lastWorkingDirectory, preferredCredentialProfileId: null as string | null },
  };
  return {
    read: () => copySettings(settings),
    recovery: () => ({ status: 'loaded' as const, schemaVersion: 1 }),
    setAppearance: () => copySettings(settings),
    setLastWorkingDirectory(directory) {
      settings = {
        ...settings,
        workspace: { ...settings.workspace, lastWorkingDirectory: directory },
      };
      return copySettings(settings);
    },
    setPreferredCredentialProfileId(profileId) {
      settings = {
        ...settings,
        workspace: { ...settings.workspace, preferredCredentialProfileId: profileId },
      };
      return copySettings(settings);
    },
    reset: () => copySettings(settings),
  };
}

function harness(options?: {
  remembered?: string | null;
  chosen?: string[];
  canceled?: boolean;
  validation?: ShellDirectoryValidateResponse_unstable;
  selectable?: { allowed: boolean; reasonCode?: string };
  generation?: () => number;
}) {
  const settings = memorySettingsStore(options?.remembered ?? null);
  const showOpenDialog = vi.fn(async () => ({
    canceled: options?.canceled ?? false,
    filePaths: options?.chosen ?? ['/chosen/alias'],
  }));
  const validate = vi.fn(
    async (candidate: string): Promise<ShellDirectoryValidateResponse_unstable> =>
      options?.validation ?? { status: 'valid', canonicalPath: `${candidate}-canonical` }
  );
  const controller = createShellDirectoryController({
    settings,
    showOpenDialog,
    validate,
    generation: options?.generation ?? (() => 1),
    isSelectable: () => options?.selectable ?? { allowed: true },
  });
  return { controller, settings, showOpenDialog, validate };
}

describe('shell working-directory controller', () => {
  it('starts unselected and never invents a directory', () => {
    const { controller, validate } = harness();
    expect(controller.read()).toEqual({
      state: 'unselected',
      path: null,
      label: null,
      reasonCode: null,
      remembered: false,
    });
    expect(controller.accepted()).toBeNull();
    expect(validate).not.toHaveBeenCalled();
  });

  it('persists only the backend-canonicalized path the operator confirmed', async () => {
    const { controller, settings, validate } = harness();

    const result = await controller.select(1);

    expect(validate).toHaveBeenCalledWith('/chosen/alias');
    expect(result).toEqual({
      status: 'selected',
      directory: {
        state: 'selected',
        path: '/chosen/alias-canonical',
        label: 'alias-canonical',
        reasonCode: null,
        remembered: true,
      },
    });
    expect(controller.accepted()).toBe('/chosen/alias-canonical');
    expect(settings.read().workspace.lastWorkingDirectory).toBe('/chosen/alias-canonical');
  });

  it('treats cancel as a successful result that changes nothing', async () => {
    const { controller, settings, validate } = harness({ canceled: true, chosen: [] });

    await expect(controller.select(1)).resolves.toEqual({
      status: 'cancelled',
      directory: {
        state: 'unselected',
        path: null,
        label: null,
        reasonCode: null,
        remembered: false,
      },
    });
    expect(validate).not.toHaveBeenCalled();
    expect(settings.read().workspace.lastWorkingDirectory).toBeNull();
  });

  it('rejects a rejected backend validation without persisting or selecting it', async () => {
    const { controller, settings } = harness({
      validation: { status: 'invalid', reason: 'not_found' },
    });

    await expect(controller.select(1)).resolves.toEqual({
      status: 'rejected',
      directory: {
        state: 'unselected',
        path: null,
        label: null,
        reasonCode: null,
        remembered: false,
      },
      reasonCode: 'not_found',
    });
    expect(settings.read().workspace.lastWorkingDirectory).toBeNull();
  });

  it('surfaces a stale remembered directory as missing and keeps the setting for recovery', async () => {
    const { controller, settings } = harness({
      remembered: '/removed/project',
      validation: { status: 'invalid', reason: 'not_found' },
    });

    await expect(controller.restore()).resolves.toEqual({
      state: 'missing',
      path: null,
      label: null,
      reasonCode: 'not_found',
      remembered: false,
    });
    expect(controller.accepted()).toBeNull();
    expect(settings.read().workspace.lastWorkingDirectory).toBe('/removed/project');
  });

  it('ignores a remembered relative path without calling the backend', async () => {
    const { controller, validate } = harness({ remembered: 'relative/project' });

    await expect(controller.restore()).resolves.toMatchObject({ state: 'unselected' });
    expect(validate).not.toHaveBeenCalled();
  });

  it('refuses a stale generation, a busy runtime, and a concurrent request', async () => {
    const stale = harness({ generation: () => 4 });
    await expect(stale.controller.select(3)).rejects.toThrow('generation is stale');
    expect(stale.showOpenDialog).not.toHaveBeenCalled();

    const busy = harness({
      selectable: { allowed: false, reasonCode: 'an interaction is in progress' },
    });
    await expect(busy.controller.select(1)).rejects.toThrow('an interaction is in progress');
    expect(busy.showOpenDialog).not.toHaveBeenCalled();

    const concurrent = harness();
    const first = concurrent.controller.select(1);
    await expect(concurrent.controller.select(1)).rejects.toThrow('already outstanding');
    await first;
    expect(concurrent.showOpenDialog).toHaveBeenCalledOnce();
  });

  it('still selects a directory when the settings document cannot be written', async () => {
    const { controller } = harness();
    const failing = {
      ...memorySettingsStore(null),
      setLastWorkingDirectory: () => {
        throw new Error('shell settings are in a recovery state and were not overwritten');
      },
    };
    const unwritable = createShellDirectoryController({
      settings: failing,
      showOpenDialog: async () => ({ canceled: false, filePaths: ['/chosen/alias'] }),
      validate: async (candidate) => ({ status: 'valid', canonicalPath: candidate }),
      generation: () => 1,
      isSelectable: () => ({ allowed: true }),
    });

    const result = await unwritable.select(1);

    expect(result.status).toBe('selected');
    expect(unwritable.accepted()).toBe('/chosen/alias');
    expect(unwritable.read().remembered).toBe(false);
    expect(controller.read().state).toBe('unselected');
  });

  it('discards a selection whose generation changed while validation was pending', async () => {
    const settings = memorySettingsStore(null);
    let generation = 1;
    let release!: (value: ShellDirectoryValidateResponse_unstable) => void;
    const controller = createShellDirectoryController({
      settings,
      showOpenDialog: async () => ({ canceled: false, filePaths: ['/chosen/alias'] }),
      validate: () =>
        new Promise<ShellDirectoryValidateResponse_unstable>((resolve) => (release = resolve)),
      generation: () => generation,
      isSelectable: () => ({ allowed: true }),
    });

    const selecting = controller.select(1);
    await Promise.resolve();
    generation = 2;
    release({ status: 'valid', canonicalPath: '/chosen/alias' });

    await expect(selecting).rejects.toThrow('generation is stale');
    expect(controller.read().state).toBe('unselected');
    expect(controller.accepted()).toBeNull();
    expect(settings.read().workspace.lastWorkingDirectory).toBeNull();
  });

  it('clears the live selection on teardown without deleting the remembered setting', async () => {
    const { controller, settings } = harness();
    await controller.select(1);

    controller.clear();

    expect(controller.read().state).toBe('unselected');
    expect(controller.accepted()).toBeNull();
    expect(settings.read().workspace.lastWorkingDirectory).toBe('/chosen/alias-canonical');
  });
});
