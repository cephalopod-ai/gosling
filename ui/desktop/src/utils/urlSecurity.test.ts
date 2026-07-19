import { describe, expect, it } from 'vitest';
import { normalizeWebUrl } from './urlSecurity';

describe('normalizeWebUrl', () => {
  it('accepts HTTP URLs containing command-shell metacharacters as inert URL data', () => {
    expect(normalizeWebUrl('https://example.com/?next=report&mode=full')).toBe(
      'https://example.com/?next=report&mode=full'
    );
  });

  it('rejects non-web protocols, malformed URLs, and non-string values', () => {
    expect(normalizeWebUrl('file:///tmp/report')).toBeNull();
    expect(normalizeWebUrl('not a URL')).toBeNull();
    expect(normalizeWebUrl({ url: 'https://example.com' })).toBeNull();
  });
});
