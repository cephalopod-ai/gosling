import { describe, expect, it, vi } from 'vitest';
import type { ShellCredentialListResponse_unstable } from '@repo-makeover/gosling-sdk';
import { createShellCredentialController } from './credentialController';
import { defaultShellLocalSettings, type ShellSettingsStore } from './localSettings';

function memorySettingsStore(preferredCredentialProfileId: string | null): ShellSettingsStore {
  let settings = {
    ...defaultShellLocalSettings(),
    workspace: { lastWorkingDirectory: null as string | null, preferredCredentialProfileId },
  };
  return {
    read: () => structuredClone(settings),
    recovery: () => ({ status: 'loaded' as const, schemaVersion: 1 }),
    setAppearance: () => structuredClone(settings),
    setLastWorkingDirectory: () => structuredClone(settings),
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

const catalog: ShellCredentialListResponse_unstable = {
  status: 'available',
  profiles: [
    { id: 'work', name: 'Work account', providerOrServiceId: 'anthropic', status: 'configured' },
    {
      id: 'lapsed',
      name: 'Lapsed account',
      providerOrServiceId: 'anthropic',
      status: 'relink_required',
    },
  ],
};

function harness(options?: {
  preferred?: string | null;
  response?: ShellCredentialListResponse_unstable;
}) {
  const settings = memorySettingsStore(options?.preferred ?? null);
  const list = vi.fn(async () => options?.response ?? catalog);
  const controller = createShellCredentialController({
    settings,
    list,
    generation: () => 1,
  });
  return { controller, list, settings };
}

describe('shell credential controller', () => {
  it('reports a denied catalog with no profiles for fixed provisioning', async () => {
    const { controller } = harness({ response: { status: 'denied', profiles: [] } });

    await expect(controller.refresh()).resolves.toEqual({
      catalogStatus: 'denied',
      profiles: [],
      selectedProfileId: null,
      selectionStatus: 'none',
    });
    await expect(controller.select(1, 'work')).rejects.toThrow('not permitted');
  });

  it('persists only the opaque profile ID after an in-catalog selection', async () => {
    const { controller, settings } = harness();
    await controller.refresh();

    const snapshot = await controller.select(1, 'work');

    expect(snapshot.selectedProfileId).toBe('work');
    expect(snapshot.selectionStatus).toBe('configured');
    expect(settings.read().workspace).toEqual({
      lastWorkingDirectory: null,
      preferredCredentialProfileId: 'work',
    });
  });

  it('never accepts a profile outside the current safe catalog or a stale generation', async () => {
    const { controller, settings } = harness();
    await controller.refresh();

    await expect(controller.select(1, 'unknown')).rejects.toThrow('not in the current safe catalog');
    await expect(controller.select(2, 'work')).rejects.toThrow('generation is stale');
    expect(settings.read().workspace.preferredCredentialProfileId).toBeNull();
  });

  it('keeps a revoked preference selected-but-invalid instead of substituting another profile', async () => {
    const { controller } = harness({
      preferred: 'work',
      response: {
        status: 'available',
        profiles: [
          {
            id: 'other',
            name: 'Other account',
            providerOrServiceId: 'anthropic',
            status: 'configured',
          },
        ],
      },
    });

    const snapshot = await controller.refresh();

    expect(snapshot.selectedProfileId).toBe('work');
    expect(snapshot.selectionStatus).toBe('missing');
  });

  it('reports a relink-required preference honestly', async () => {
    const { controller } = harness({ preferred: 'lapsed' });

    await expect(controller.refresh()).resolves.toMatchObject({
      selectedProfileId: 'lapsed',
      selectionStatus: 'relink_required',
    });
  });

  it('drops any catalog entry carrying a field outside the safe projection', async () => {
    const { controller } = harness({
      response: {
        status: 'available',
        profiles: [
          {
            id: 'work',
            name: 'Work account',
            providerOrServiceId: 'anthropic',
            status: 'configured',
            configuredSecretFields: ['SENTINEL_SECRET_FIELD'],
          },
        ],
      } as unknown as ShellCredentialListResponse_unstable,
    });

    const snapshot = await controller.refresh();

    expect(snapshot.profiles).toEqual([]);
    expect(JSON.stringify(snapshot)).not.toContain('SENTINEL_SECRET_FIELD');
  });
});
