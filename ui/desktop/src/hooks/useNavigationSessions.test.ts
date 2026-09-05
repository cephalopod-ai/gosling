import { describe, expect, it } from 'vitest';
import {
  createWorkspaceSessionFilter,
  matchesWorkspaceSessionFilter,
  pairSessionPath,
} from './useNavigationSessions';

describe('pairSessionPath', () => {
  it('passes an ordinary session id through unchanged', () => {
    expect(pairSessionPath('20260904_12')).toBe('/pair?resumeSessionId=20260904_12');
  });

  it('encodes characters that would otherwise alter the query string', () => {
    expect(pairSessionPath('a&b=c')).toBe('/pair?resumeSessionId=a%26b%3Dc');
    expect(pairSessionPath('a b#frag')).toBe('/pair?resumeSessionId=a%20b%23frag');
    expect(pairSessionPath('../../etc/passwd')).toBe(
      '/pair?resumeSessionId=..%2F..%2Fetc%2Fpasswd'
    );
  });
});

describe('createWorkspaceSessionFilter', () => {
  it('returns only the explicitly selected workspace', () => {
    expect(createWorkspaceSessionFilter('workspace-1')).toEqual({ workspaceId: 'workspace-1' });
  });

  it('does not filter when all workspaces is selected', () => {
    expect(createWorkspaceSessionFilter(null)).toBeUndefined();
  });
});

describe('matchesWorkspaceSessionFilter', () => {
  it('shows every session only in the all-workspaces view', () => {
    expect(matchesWorkspaceSessionFilter({ workspaceId: 'workspace-1' }, null)).toBe(true);
    expect(matchesWorkspaceSessionFilter({ workspaceId: undefined }, null)).toBe(true);
  });

  it('requires an exact assignment for a named workspace', () => {
    expect(matchesWorkspaceSessionFilter({ workspaceId: 'workspace-1' }, 'workspace-1')).toBe(true);
    expect(matchesWorkspaceSessionFilter({ workspaceId: 'workspace-2' }, 'workspace-1')).toBe(
      false
    );
    expect(matchesWorkspaceSessionFilter({ workspaceId: undefined }, 'workspace-1')).toBe(false);
  });
});
