import { afterEach, describe, expect, it, vi } from 'vitest';
import { getAllowList } from './allowlist';

describe('extension allowlist', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
  });

  it('returns an empty list when no URL is configured', async () => {
    vi.stubEnv('GOSLING_ALLOWLIST', '');
    await expect(getAllowList()).resolves.toEqual([]);
  });

  it('preserves HTTPS YAML command extraction', async () => {
    vi.stubEnv('GOSLING_ALLOWLIST', 'https://example.test/allowlist.yaml');
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(
          new Response('extensions:\n  - id: example\n    command: npx example\n', { status: 200 })
        )
    );

    await expect(getAllowList()).resolves.toEqual(['npx example']);
  });

  it('rejects a non-HTTPS source before fetching', async () => {
    vi.stubEnv('GOSLING_ALLOWLIST', 'http://example.test/allowlist.yaml');
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);

    await expect(getAllowList()).rejects.toThrow('must use https');
    expect(fetch).not.toHaveBeenCalled();
  });
});
