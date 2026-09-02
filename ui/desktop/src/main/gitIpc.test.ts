import { describe, expect, it, vi } from 'vitest';
import { gitArgs, isValidGitBranch, registerGitIpcHandlers } from './gitIpc';

describe('Git IPC', () => {
  it('keeps hardening options ahead of the repository and caller arguments', () => {
    expect(gitArgs('/repo', ['status', '--short'])).toEqual([
      '-c',
      'safe.bareRepository=explicit',
      '-c',
      'core.fsmonitor=false',
      '-C',
      '/repo',
      'status',
      '--short',
    ]);
  });

  it('rejects unsafe or malformed branch values', () => {
    expect(isValidGitBranch('feature/example')).toBe(true);
    expect(isValidGitBranch('')).toBe(false);
    expect(isValidGitBranch('-detach')).toBe(false);
    expect(isValidGitBranch('bad\0branch')).toBe(false);
    expect(isValidGitBranch(123)).toBe(false);
  });

  it('registers the original four channel names', () => {
    const handle = vi.fn();
    registerGitIpcHandlers({ handle }, vi.fn());

    expect(handle.mock.calls.map(([channel]) => channel)).toEqual([
      'list-git-worktree-dirs',
      'get-git-branch-info',
      'list-git-branches',
      'switch-git-branch',
    ]);
  });
});
