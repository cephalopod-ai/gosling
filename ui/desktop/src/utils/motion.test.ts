import { afterEach, describe, expect, it, vi } from 'vitest';
import { getMotionAwareScrollBehavior } from './motion';

describe('getMotionAwareScrollBehavior', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses smooth scrolling by default', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: false }))
    );

    expect(getMotionAwareScrollBehavior()).toBe('smooth');
  });

  it('uses immediate scrolling when reduced motion is requested', () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }))
    );

    expect(getMotionAwareScrollBehavior()).toBe('auto');
  });
});
