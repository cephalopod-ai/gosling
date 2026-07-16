import { describe, expect, it } from 'vitest';
import {
  addSandboxCsp,
  artifactKindFromMimeType,
  artifactKindFromPath,
  parseCsv,
} from './artifactUtils';

describe('artifactUtils', () => {
  it('maps common deliverable paths and MIME types', () => {
    expect(artifactKindFromPath('/tmp/report.md')).toBe('markdown');
    expect(artifactKindFromPath('C:\\work\\brief.PDF')).toBe('pdf');
    expect(artifactKindFromPath('/tmp/archive.bin')).toBe('unknown');
    expect(artifactKindFromMimeType('image/svg+xml')).toBe('svg');
  });

  it('parses quoted CSV cells and embedded newlines', () => {
    expect(parseCsv('name,note\n"Ada, A.","line 1\nline 2"\n')).toEqual([
      ['name', 'note'],
      ['Ada, A.', 'line 1\nline 2'],
    ]);
  });

  it('injects a restrictive CSP into HTML previews', () => {
    const result = addSandboxCsp('<html><head><title>Output</title></head><body></body></html>');
    expect(result).toContain('Content-Security-Policy');
    expect(result).toContain("connect-src 'none'");
    expect(result.indexOf('Content-Security-Policy')).toBeLessThan(result.indexOf('<title>'));
  });
});
