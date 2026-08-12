export type MotionAwareScrollBehavior = 'auto' | 'smooth';

export function getMotionAwareScrollBehavior(): MotionAwareScrollBehavior {
  if (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  ) {
    return 'auto';
  }

  return 'smooth';
}
