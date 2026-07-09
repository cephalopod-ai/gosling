import { describe, expect, it } from 'vitest';
import { getOverrideOriginForRequest } from './requestOrigin';

describe('getOverrideOriginForRequest', () => {
  it('does not override requests when no dev server is configured', () => {
    expect(getOverrideOriginForRequest('http://127.0.0.1:50436/acp')).toBeNull();
  });

  it('only overrides requests that target the dev server origin', () => {
    expect(
      getOverrideOriginForRequest(
        'http://localhost:5173/src/main.tsx',
        'http://localhost:5173'
      )
    ).toBe('http://localhost:5173');
  });

  it('does not override ACP loopback requests in packaged mode', () => {
    expect(
      getOverrideOriginForRequest('ws://127.0.0.1:50436/acp?token=test', 'http://localhost:5173')
    ).toBeNull();
  });

  it('returns null for malformed URLs', () => {
    expect(getOverrideOriginForRequest('not a url', 'http://localhost:5173')).toBeNull();
    expect(getOverrideOriginForRequest('http://localhost:5173', 'not a url')).toBeNull();
  });
});
