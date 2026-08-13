import { describe, expect, it, vi } from 'vitest';
import { resolveContextLimit } from './contextLimit';

describe('resolveContextLimit', () => {
  it('prefers the active provider route over a public canonical limit', async () => {
    const loadCanonicalLimit = vi.fn().mockResolvedValue(1_000_000);

    const limit = await resolveContextLimit(
      'gpt-5.4',
      [],
      async () => [{ name: 'gpt-5.4', context_limit: 258_400 }],
      loadCanonicalLimit,
      128_000
    );

    expect(limit).toBe(258_400);
    expect(loadCanonicalLimit).not.toHaveBeenCalled();
  });

  it('keeps an explicit predefined-model override first', async () => {
    const limit = await resolveContextLimit(
      'custom-model',
      [{ name: 'custom-model', context_limit: 64_000 }],
      async () => [{ name: 'custom-model', context_limit: 32_000 }],
      async () => 16_000,
      8_000
    );

    expect(limit).toBe(64_000);
  });
});
