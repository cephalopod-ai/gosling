import { describe, expect, it, vi } from 'vitest';
import { createNavigationHandler } from './navigationUtils';

describe('createNavigationHandler research navigation', () => {
  it('opens the research landing route', () => {
    const navigate = vi.fn();

    createNavigationHandler(navigate)('research');

    expect(navigate).toHaveBeenCalledWith('/research', { state: undefined });
  });

  it('preserves the research experience when entering a session', () => {
    const navigate = vi.fn();

    createNavigationHandler(navigate)('pair', {
      resumeSessionId: 'research-session',
      sessionExperience: 'research',
    });

    expect(navigate).toHaveBeenCalledWith(
      '/pair?resumeSessionId=research-session&sessionExperience=research',
      {
        state: {
          resumeSessionId: 'research-session',
          sessionExperience: 'research',
        },
      }
    );
  });

  it.each([
    ['skills', '/settings?section=skills'],
    ['extensions', '/settings?section=extensions'],
  ] as const)('opens %s inside settings', (view, route) => {
    const navigate = vi.fn();

    createNavigationHandler(navigate)(view);

    expect(navigate).toHaveBeenCalledWith(route, { state: undefined });
  });
});
