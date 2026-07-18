import { describe, expect, it } from 'vitest';
import type { ArtifactRoutingConfig } from '../types/artifactRouter';
import { ArtifactRoutingRegistry } from './artifactRoutingRegistry';

function createConfig(workspaceId: string): ArtifactRoutingConfig {
  return {
    workspaceId,
    workspaceName: workspaceId,
    outputs: [
      {
        id: `${workspaceId}-output`,
        path: `/tmp/${workspaceId}`,
        productTypes: ['export'],
        isDefault: true,
      },
    ],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('ArtifactRoutingRegistry', () => {
  it('keeps the newest config when validation finishes out of order', async () => {
    const registry = new ArtifactRoutingRegistry();
    const firstValidation = deferred<ArtifactRoutingConfig | null>();
    const secondValidation = deferred<ArtifactRoutingConfig | null>();
    const firstConfig = createConfig('first');
    const secondConfig = createConfig('second');

    const firstUpdate = registry.update(7, firstConfig, () => firstValidation.promise);
    const secondUpdate = registry.update(7, secondConfig, () => secondValidation.promise);

    secondValidation.resolve(secondConfig);
    expect(await secondUpdate).toBe(true);
    expect(registry.get(7)).toEqual(secondConfig);

    firstValidation.resolve(firstConfig);
    expect(await firstUpdate).toBe(false);
    expect(registry.get(7)).toEqual(secondConfig);
  });

  it('does not restore a config after the window is cleared during validation', async () => {
    const registry = new ArtifactRoutingRegistry();
    const validation = deferred<ArtifactRoutingConfig | null>();
    const config = createConfig('closed-window');

    const update = registry.update(9, config, () => validation.promise);
    registry.clear(9);
    validation.resolve(config);

    expect(await update).toBe(false);
    expect(registry.get(9)).toBeUndefined();
  });
});
