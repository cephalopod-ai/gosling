import { describe, expect, it, vi } from 'vitest';
import type { ShellDirectoryValidateResponse_unstable } from '@repo-makeover/gosling-sdk';
import { createShellDirectoryController } from './directoryController';
import { defaultShellLocalSettings, type ShellSettingsStore } from './localSettings';

function memorySettingsStore(lastWorkingDirectory: string | null): ShellSettingsStore {
  let settings = {
    ...defaultShellLocalSettings(),
    workspace: { lastWorkingDirectory, preferredCredentialProfileId: null as string | null },
  };
  return {
    read: () => structuredClone(settings),
    recovery: () => ({ status: 'loaded' as const, schemaVersion: 1 }),
    setAppearance: () => structuredClone(settings),
    setLastWorkingDirectory(directory) {
      settings = {
        ...settings,
        workspace: { ...settings.workspace, lastWorkingDirectory: directory },
      };
      return structuredClone(settings);
    },
    setPreferredCredentialProfileId(profileId) {
      settings = {
        ...settings,
        workspace: { ...settings.workspace, preferredCredentialProfileId: profileId },
      };
      return structuredClone(settings);
    },
    reset: () => structuredClone(settings),
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
      },
    });
    expect(controller.accepted()).toBe('/chosen/alias-canonical');
    expect(settings.read().workspace.lastWorkingDirectory).toBe('/chosen/alias-canonical');
  });

  it('treats cancel as a successful result that changes nothing', async () => {
    const { controller, settings, validate } = harness({ canceled: true, chosen: [] });

    await expect(controller.select(1)).resolves.toEqual({
      status: 'cancelled',
      directory: { state: 'unselected', path: null, label: null, reasonCode: null },
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
      directory: { state: 'unselected', path: null, label: null, reasonCode: null },
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

  it('clears the live selection on teardown without deleting the remembered setting', async () => {
    const { controller, settings } = harness();
    await controller.select(1);

    controller.clear();

    expect(controller.read().state).toBe('unselected');
    expect(controller.accepted()).toBeNull();
    expect(settings.read().workspace.lastWorkingDirectory).toBe('/chosen/alias-canonical');
  });
});
