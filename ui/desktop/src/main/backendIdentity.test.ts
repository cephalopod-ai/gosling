// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { backendStorageId } from './backendIdentity';

describe('backendStorageId', () => {
  it('normalizes equivalent external URLs without including credentials', () => {
    const first = backendStorageId({ kind: 'external', baseUrl: 'https://example.com:443/team/' });
    expect(first).toBe(backendStorageId({ kind: 'external', baseUrl: 'https://example.com/team' }));
    expect(first).toBe(
      backendStorageId({ kind: 'external', baseUrl: 'https://user:secret@example.com/team/' })
    );
    expect(first).not.toContain('secret');
    expect(first).not.toBe(
      backendStorageId({ kind: 'external', baseUrl: 'https://example.com/other' })
    );
    expect(first).not.toBe(
      backendStorageId({ kind: 'external', baseUrl: 'http://example.com/team' })
    );
  });

  it('uses the local store root independently of per-window ports and working directories', () => {
    const first = backendStorageId({ kind: 'local', dataRoot: '/tmp/profile-a' });
    expect(first).toBe(backendStorageId({ kind: 'local', dataRoot: '/tmp/profile-a/.' }));
    expect(first).not.toBe(backendStorageId({ kind: 'local', dataRoot: '/tmp/profile-b' }));
  });
});
