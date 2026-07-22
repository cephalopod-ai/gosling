import { describe, expect, it } from 'vitest';
import {
  describeAcpError,
  isAcpConnectionClosedError,
  parseAcpCreditsExhaustedError,
} from '../errors';

describe('parseAcpCreditsExhaustedError', () => {
  it('parses structured ACP credits exhausted errors', () => {
    expect(
      parseAcpCreditsExhaustedError({
        code: -32603,
        message: 'Please add credits to your account, then resend your message to continue.',
        data: {
          reason: 'credits_exhausted',
          url: 'https://router.tetrate.ai/billing',
        },
      })
    ).toEqual({
      message: 'Please add credits to your account, then resend your message to continue.',
      url: 'https://router.tetrate.ai/billing',
    });
  });

  it('parses wrapped JSON-RPC errors', () => {
    expect(
      parseAcpCreditsExhaustedError({
        error: {
          code: -32603,
          message: 'Add credits to continue.',
          data: {
            reason: 'credits_exhausted',
          },
        },
      })
    ).toEqual({
      message: 'Add credits to continue.',
    });
  });

  it('ignores non-credits-exhausted errors', () => {
    expect(
      parseAcpCreditsExhaustedError({
        code: -32603,
        message: 'Something failed.',
        data: {
          reason: 'provider_error',
        },
      })
    ).toBeNull();
  });
});

describe('describeAcpError', () => {
  it('appends string data from Rust-side internal errors', () => {
    expect(
      describeAcpError({
        code: -32603,
        message: 'Internal error',
        data: 'provider config missing an api key',
      })
    ).toBe('Internal error: provider config missing an api key');
  });

  it('appends structured data as JSON', () => {
    expect(
      describeAcpError({
        code: -32603,
        message: 'Internal error',
        data: { reason: 'provider_error' },
      })
    ).toBe('Internal error: {"reason":"provider_error"}');
  });

  it('returns the bare message when there is no data', () => {
    expect(describeAcpError({ code: -32601, message: 'Method not found' })).toBe(
      'Method not found'
    );
  });

  it('falls back to a plain Error message', () => {
    expect(describeAcpError(new Error('ACP connection closed'))).toBe('ACP connection closed');
  });
});

describe('isAcpConnectionClosedError', () => {
  it('recognizes SDK and WebSocket connection failures', () => {
    expect(isAcpConnectionClosedError(new Error('ACP connection closed'))).toBe(true);
    expect(isAcpConnectionClosedError(new Error('ACP WebSocket connection failed'))).toBe(true);
    expect(isAcpConnectionClosedError({ message: 'WebSocket connection reset' })).toBe(true);
  });

  it('does not classify provider failures as connection loss', () => {
    expect(isAcpConnectionClosedError(new Error('Provider rate limit exceeded'))).toBe(false);
  });
});
