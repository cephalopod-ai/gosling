import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getDefaultWorkspaceWorkingDir } from './workingDir';

describe('getDefaultWorkspaceWorkingDir', () => {
  beforeEach(() => {
    Object.assign(window, {
      appConfig: {
        get: vi.fn((key: string) =>
          key === 'GOSLING_HOME_DIR' ? '/Users/tester/' : '/ignored/project'
        ),
        getAll: vi.fn(() => ({})),
      },
    });
  });

  it('starts new workspace drafts in the user Work directory', () => {
    expect(getDefaultWorkspaceWorkingDir()).toBe('/Users/tester/Work');
  });

  it('uses the platform separator exposed by the home path', () => {
    vi.mocked(window.appConfig.get).mockReturnValue('C:\\Users\\tester\\');

    expect(getDefaultWorkspaceWorkingDir()).toBe('C:\\Users\\tester\\Work');
  });
});
