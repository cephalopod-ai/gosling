import { describe, expect, it } from 'vitest';
import { isAcpConnectionClosedError, parseAcpCreditsExhaustedError } from '../errors';

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
