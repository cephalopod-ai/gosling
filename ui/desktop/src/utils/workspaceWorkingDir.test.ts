import { describe, expect, it } from 'vitest';
import { reconcileWorkspaceWorkingDir } from './workspaceWorkingDir';

describe('reconcileWorkspaceWorkingDir', () => {
  it('adopts the primary folder when the active workspace changes', () => {
    expect(
      reconcileWorkspaceWorkingDir(
        '/projects/annual',
        { id: 'annual', workingFolder: '/projects/annual' },
        { id: 'personal', workingFolder: '/projects/personal' }
      )
    ).toBe('/projects/personal');
  });

  it('updates a followed primary folder after workspace editing', () => {
    expect(
      reconcileWorkspaceWorkingDir(
        '/projects/annual',
        { id: 'annual', workingFolder: '/projects/annual' },
        { id: 'annual', workingFolder: '/projects/annual-moved' }
      )
    ).toBe('/projects/annual-moved');
  });

  it('preserves an intentional temporary override during unrelated refreshes', () => {
    expect(
      reconcileWorkspaceWorkingDir(
        '/tmp/one-off',
        { id: 'annual', workingFolder: '/projects/annual' },
        { id: 'annual', workingFolder: '/projects/annual' }
      )
    ).toBe('/tmp/one-off');
  });
});
