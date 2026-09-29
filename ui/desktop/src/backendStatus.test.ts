// @vitest-environment node
import { describe, expect, it, vi } from 'vitest';
import { checkBackendStatus, getBackendStatus } from './backendStatus';

type FetchInput = Parameters<typeof globalThis.fetch>[0];
type FetchInit = NonNullable<Parameters<typeof globalThis.fetch>[1]>;

const fetchInputUrl = (input: FetchInput): string => {
  if (typeof input === 'string') {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
};

type FetchSignal = NonNullable<FetchInit['signal']>;

const expectAbortSignal = (init?: FetchInit): FetchSignal => {
  expect(init?.signal).toBeInstanceOf(globalThis.AbortSignal);
  return init!.signal!;
};

describe('checkBackendStatus', () => {
  it.each([401, 403])(
    'distinguishes HTTP %s authentication rejection without retrying',
    async (status) => {
      const fetch = vi.fn().mockResolvedValue(new Response(null, { status }));
      const result = await getBackendStatus({
        baseUrl: 'https://example.com',
        serverSecret: 'test',
        fetch,
      });
      expect(result).toEqual({
        ready: false,
        detail: expect.stringContaining('Authentication was rejected'),
      });
      expect(fetch).toHaveBeenCalledTimes(1);
    }
  );

  it('distinguishes TLS failures without exposing raw error details', async () => {
    const fetch = vi
      .fn()
      .mockRejectedValue(new Error('net::ERR_CERT_AUTHORITY_INVALID private-secret'));
    const result = await getBackendStatus({
      baseUrl: 'https://example.com',
      serverSecret: 'private-secret',
      fetch,
    });
    expect(result).toEqual({
      ready: false,
      detail: expect.stringContaining('TLS negotiation failed'),
    });
    expect(JSON.stringify(result)).not.toContain('private-secret');
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it.each(['network', 'status', 'acp'])('distinguishes a bounded %s failure', async (failure) => {
    vi.useFakeTimers();
    try {
      const fetch = vi.fn(async (input: FetchInput) => {
        if (failure === 'network') throw new Error('ECONNREFUSED');
        return new Response(null, {
          status: failure === 'status' || fetchInputUrl(input).endsWith('/acp') ? 404 : 200,
        });
      });
      const pending = getBackendStatus({
        baseUrl: 'http://127.0.0.1:4000',
        serverSecret: 'test',
        fetch,
      });
      await vi.advanceTimersByTimeAsync(31000);
      expect(await pending).toEqual({
        ready: false,
        detail: expect.stringContaining(
          failure === 'network'
            ? 'did not respond'
            : `${failure === 'status' ? 'status' : 'ACP'} endpoint returned HTTP 404`
        ),
      });
    } finally {
      vi.useRealTimers();
    }
  });

  it('checks /status and validates the secret against /acp', async () => {
    const fetch = vi.fn(async (input: FetchInput, init?: FetchInit) => {
      const url = fetchInputUrl(input);
      if (url === 'https://example.com/gosling/status') {
        expect(init?.headers).toEqual({ 'X-Secret-Key': 'test-secret' });
        expectAbortSignal(init);
        return new Response(null, { status: 200 });
      }
      if (url === 'https://example.com/gosling/acp') {
        expect(init?.headers).toEqual({ 'X-Secret-Key': 'test-secret' });
        expectAbortSignal(init);
        return new Response(null, { status: 406 });
      }

      throw new Error(`Unexpected URL: ${url}`);
    });

    await expect(
      checkBackendStatus({
        baseUrl: 'https://example.com/gosling',
        serverSecret: 'test-secret',
        fetch,
      })
    ).resolves.toBe(true);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(fetch.mock.calls.map(([input]) => fetchInputUrl(input))).toEqual([
      'https://example.com/gosling/status',
      'https://example.com/gosling/acp',
    ]);
    expect(fetch.mock.calls.some(([input]) => fetchInputUrl(input).includes('token='))).toBe(false);
  });

  it('fails immediately when the ACP auth probe rejects the secret', async () => {
    const onEvent = vi.fn();
    const fetch = vi.fn(async (input: FetchInput, init?: FetchInit) => {
      const url = fetchInputUrl(input);
      if (url === 'https://example.com/status') {
        return new Response(null, { status: 200 });
      }
      if (url === 'https://example.com/acp') {
        expect(init?.headers).toEqual({ 'X-Secret-Key': 'wrong-secret' });
        return new Response(null, { status: 401 });
      }

      throw new Error(`Unexpected URL: ${url}`);
    });

    await expect(
      checkBackendStatus({
        baseUrl: 'https://example.com',
        serverSecret: 'wrong-secret',
        fetch,
        options: { onEvent },
      })
    ).resolves.toBe(false);

    expect(fetch).toHaveBeenCalledTimes(2);
    expect(onEvent).toHaveBeenCalledWith('healthcheck_auth_failed', { attempt: 1 });
  });

  it('aborts hanging ACP auth probes and reports the healthcheck timeout', async () => {
    vi.useFakeTimers();

    try {
      const onEvent = vi.fn();
      const acpSignals: FetchSignal[] = [];
      const fetch = vi.fn((input: FetchInput, init?: FetchInit): Promise<Response> => {
        const url = fetchInputUrl(input);
        if (url === 'https://example.com/status') {
          expectAbortSignal(init);
          return Promise.resolve(new Response(null, { status: 200 }));
        }
        if (url === 'https://example.com/acp') {
          const signal = expectAbortSignal(init);
          acpSignals.push(signal);
          return new Promise<Response>((_, reject) => {
            signal.addEventListener('abort', () => reject(new Error('aborted')), { once: true });
          });
        }

        throw new Error(`Unexpected URL: ${url}`);
      });

      const result = checkBackendStatus({
        baseUrl: 'https://example.com',
        serverSecret: 'test-secret',
        fetch,
        options: { onEvent },
      });

      await vi.advanceTimersByTimeAsync(31000);

      await expect(result).resolves.toBe(false);
      expect(acpSignals.length).toBeGreaterThan(0);
      expect(acpSignals.every((signal) => signal.aborted)).toBe(true);
      expect(onEvent).toHaveBeenCalledWith('healthcheck_timeout', { timeoutMs: 30000 });
    } finally {
      vi.useRealTimers();
    }
  });
});
