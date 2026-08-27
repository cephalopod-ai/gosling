import { describe, expect, it } from 'vitest';
import { normalizeWebUrl, openExternalUrlIfSafe } from './urlSecurity';

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

describe('openExternalUrlIfSafe', () => {
  it('opens allowed protocols through the supplied host callback', async () => {
    const opened: string[] = [];

    await expect(
      openExternalUrlIfSafe('https://example.com/report', async (url) => {
        opened.push(url);
      })
    ).resolves.toBe(true);
    expect(opened).toEqual(['https://example.com/report']);
  });

  it.each(['file:///etc/passwd', 'javascript:alert(1)', 'data:text/plain,secret']) (
    'blocks %s before calling the host',
    async (url) => {
      const opened: string[] = [];

      await expect(
        openExternalUrlIfSafe(url, async (safeUrl) => {
          opened.push(safeUrl);
        })
      ).resolves.toBe(false);
      expect(opened).toEqual([]);
    }
  );
});
