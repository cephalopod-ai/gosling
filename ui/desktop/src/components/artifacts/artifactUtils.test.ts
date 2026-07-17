import { describe, expect, it } from 'vitest';
import {
  addSandboxCsp,
  artifactKindFromMimeType,
  artifactKindFromPath,
  localFilePathFromUri,
  parseCsv,
  viewableFilePathsFromToolArguments,
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

  it('finds viewable files without depending on a tool name or project layout', () => {
    expect(
      viewableFilePathsFromToolArguments({
        sourcePath: 'analysis/results.json',
        nested: { outputFiles: ['/tmp/chart.svg', '/tmp/archive.bin'] },
        deliverables: [{ path: 'nested/deliverable.md' }],
        uri: 'https://example.com/report.pdf',
      })
    ).toEqual(['analysis/results.json', '/tmp/chart.svg', 'nested/deliverable.md']);
  });

  it('normalizes local resource links and rejects remote resources', () => {
    expect(localFilePathFromUri('file:///tmp/output%20brief.md')).toBe('/tmp/output brief.md');
    expect(localFilePathFromUri('relative/output.csv')).toBe('relative/output.csv');
    expect(localFilePathFromUri('https://example.com/output.csv')).toBeNull();
  });
});
