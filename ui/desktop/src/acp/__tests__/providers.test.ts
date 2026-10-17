// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAcpClient } from '../acpConnection';
import {
  acpListProviderModels,
  acpPreviewSessionHandoff,
  acpSaveDefaults,
  acpSetSessionProviderModel,
  parseProviderType,
} from '../providers';

vi.mock('../acpConnection', () => ({
  getAcpClient: vi.fn(),
}));

describe('ACP providers', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('sets provider, model, and thinking effort through one atomic transition', async () => {
    const snapshot = { snapshotId: 'handoff-1' };
    const client = {
      gosling: {
        sessionProviderTransition_unstable: vi.fn().mockResolvedValue({
          snapshot,
          previousProvider: 'openai',
          previousModel: 'gpt-5',
          activeProvider: 'anthropic',
          activeModel: 'claude-sonnet-4-5',
        }),
      },
    };
    vi.mocked(getAcpClient).mockResolvedValue(
      client as unknown as Awaited<ReturnType<typeof getAcpClient>>
    );

    const applied = await acpSetSessionProviderModel(
      'session-1',
      'anthropic',
      'claude-sonnet-4-5',
      'high'
    );

    expect(client.gosling.sessionProviderTransition_unstable).toHaveBeenCalledOnce();
    expect(client.gosling.sessionProviderTransition_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      targetProvider: 'anthropic',
      targetModel: 'claude-sonnet-4-5',
      targetThinkingEffort: 'high',
      targetContextLimit: null,
      requestParams: null,
      expectedCurrentGeneration: null,
      expectedSourceHash: null,
      expectedActiveRunId: null,
      expectedToolStateHash: null,
      confirmNewContext: false,
    });
    expect(applied).toEqual({
      providerId: 'anthropic',
      modelId: 'claude-sonnet-4-5',
      thinkingEffort: 'high',
      snapshot,
    });
  });

  it('previews checkpoint coverage without activating the target', async () => {
    const preview = { snapshot: { snapshotId: 'preview-1' }, expectedCurrentGeneration: 3 };
    const client = {
      gosling: {
        sessionHandoffCheckpointPreview_unstable: vi.fn().mockResolvedValue(preview),
      },
    };
    vi.mocked(getAcpClient).mockResolvedValue(
      client as unknown as Awaited<ReturnType<typeof getAcpClient>>
    );

    await expect(
      acpPreviewSessionHandoff('session-1', 'anthropic', 'claude-sonnet-4-5', 200_000)
    ).resolves.toEqual(preview);
    expect(client.gosling.sessionHandoffCheckpointPreview_unstable).toHaveBeenCalledWith({
      sessionId: 'session-1',
      targetProvider: 'anthropic',
      targetModel: 'claude-sonnet-4-5',
      targetContextLimit: 200_000,
    });
  });

  it('prefers live context metadata over cached inventory metadata', async () => {
    const client = {
      gosling: {
        providersSupportedModelsList_unstable: vi.fn().mockResolvedValue({
          models: ['spark', 'unknown'],
          contextLimits: { spark: 1_048_576 },
        }),
        providersList_unstable: vi.fn().mockResolvedValue({
          entries: [{ providerId: 'together', models: [{ id: 'spark', contextLimit: 128_000 }] }],
        }),
      },
    };
    vi.mocked(getAcpClient).mockResolvedValue(
      client as unknown as Awaited<ReturnType<typeof getAcpClient>>
    );
    expect(await acpListProviderModels('together')).toEqual([
      expect.objectContaining({ id: 'spark', contextLimit: 1_048_576 }),
      { id: 'unknown', contextLimit: undefined },
    ]);
  });

  it('persists the selected default model context limit', async () => {
    const save = vi.fn().mockResolvedValue({});
    vi.mocked(getAcpClient).mockResolvedValue({
      gosling: { defaultsSave_unstable: save },
    } as unknown as Awaited<ReturnType<typeof getAcpClient>>);
    await acpSaveDefaults('together', 'spark', 1_048_576);
    expect(save).toHaveBeenCalledWith({
      providerId: 'together',
      modelId: 'spark',
      contextLimit: 1_048_576,
    });
  });
});

describe('parseProviderType', () => {
  it.each(['Preferred', 'Builtin', 'Declarative', 'Custom'] as const)(
    'accepts the known provider type %s',
    (value) => {
      expect(parseProviderType(value)).toBe(value);
    }
  );

  it('throws instead of silently casting an unrecognized provider type', () => {
    expect(() => parseProviderType('SomeFutureProviderType')).toThrow();
  });

  it('throws on a case mismatch rather than accepting it', () => {
    expect(() => parseProviderType('preferred')).toThrow();
  });
});
