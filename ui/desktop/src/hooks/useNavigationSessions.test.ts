import { describe, expect, it } from 'vitest';
import { createWorkspaceSessionFilter } from './useNavigationSessions';

describe('createWorkspaceSessionFilter', () => {
  it('returns only the explicitly selected workspace', () => {
    expect(createWorkspaceSessionFilter('workspace-1')).toEqual({ workspaceId: 'workspace-1' });
  });

  it('does not filter when all workspaces is selected', () => {
    expect(createWorkspaceSessionFilter(null)).toBeUndefined();
  });
});
