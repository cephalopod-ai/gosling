// @vitest-environment node
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { execFile } from 'child_process';
import { GIT_IPC_CHANNELS, gitArgs, isValidGitBranch, registerGitIpcHandlers } from './gitIpc';
import { desktopCommandChannels } from '../ipc/channels';

vi.mock('child_process', () => ({ execFile: vi.fn() }));

describe('Git IPC', () => {
  beforeEach(() => vi.clearAllMocks());

  it('treats an ungranted indicator directory as unavailable without running Git', async () => {
    const handle = vi.fn();
    const authorize = vi.fn().mockRejectedValue(new Error('Path is outside allowed roots'));
    registerGitIpcHandlers({ handle }, authorize);
    const handler = handle.mock.calls.find(
      ([channel]) => channel === desktopCommandChannels.getGitBranchInfo
    )![1];

    await expect(handler({ sender: { id: 7 } }, '/unapproved')).resolves.toBeNull();
    expect(authorize).toHaveBeenCalledWith(7, '/unapproved');
    expect(execFile).not.toHaveBeenCalled();
  });

  it('looks up a branch only at the authorized canonical directory', async () => {
    const handle = vi.fn();
    registerGitIpcHandlers({ handle }, vi.fn().mockResolvedValue('/approved/canonical'));
    vi.mocked(execFile).mockImplementation((_file, _args, _options, callback) => {
      callback!(null, 'main\n', '');
      return {} as ReturnType<typeof execFile>;
    });
    const handler = handle.mock.calls.find(
      ([channel]) => channel === desktopCommandChannels.getGitBranchInfo
    )![1];

    await expect(handler({ sender: { id: 7 } }, '/approved')).resolves.toEqual({ branch: 'main' });
    expect(execFile).toHaveBeenCalledWith(
      'git',
      gitArgs('/approved/canonical', ['symbolic-ref', '--quiet', '--short', 'HEAD']),
      { timeout: 3000 },
      expect.any(Function)
    );
  });
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

    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(GIT_IPC_CHANNELS);
  });
});
