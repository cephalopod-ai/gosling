import { describe, expect, it } from 'vitest';
import { NAV_DIMENSIONS } from './constants';
import {
  MIN_DOCKED_PANE_WIDTH,
  MIN_MAIN_CONTENT_WIDTH,
  resolveArtifactPaneLayout,
} from './artifactPaneLayout';

const NAV = NAV_DIMENSIONS.NAV_WIDTH;

describe('resolveArtifactPaneLayout', () => {
  it('floats the pane over the hub at the default 940px window with navigation open', () => {
    expect(resolveArtifactPaneLayout(940, NAV, 480)).toEqual({ mode: 'overlay', width: 480 });
  });

  it('docks the pane at 940px when navigation is collapsed', () => {
    const layout = resolveArtifactPaneLayout(940, 0, 480);
    expect(layout).toEqual({ mode: 'docked', width: 480 });
    expect(940 - layout.width).toBeGreaterThanOrEqual(MIN_MAIN_CONTENT_WIDTH);
  });

  it('narrows a docked pane so the main panel keeps its minimum width', () => {
    const layout = resolveArtifactPaneLayout(1000, NAV, 480);
    expect(layout).toEqual({ mode: 'docked', width: 1000 - NAV - MIN_MAIN_CONTENT_WIDTH });
  });

  it('switches mode exactly where a minimum-width docked pane stops fitting', () => {
    const breakpoint = NAV + MIN_MAIN_CONTENT_WIDTH + MIN_DOCKED_PANE_WIDTH;
    expect(resolveArtifactPaneLayout(breakpoint, NAV, 480)).toEqual({
      mode: 'docked',
      width: MIN_DOCKED_PANE_WIDTH,
    });
    expect(resolveArtifactPaneLayout(breakpoint - 1, NAV, 480).mode).toBe('overlay');
  });

  it('keeps the preferred width when the window is wide enough', () => {
    expect(resolveArtifactPaneLayout(1600, NAV, 480)).toEqual({ mode: 'docked', width: 480 });
  });

  it('never makes an overlay wider than the window', () => {
    expect(resolveArtifactPaneLayout(480, 0, 720)).toEqual({ mode: 'overlay', width: 480 });
  });
});
